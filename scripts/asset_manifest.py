"""发布资产清单（scripts/release-assets.json）严格 schema 的 Python 读取与校验。

本模块与 scripts/AssetManifest.psm1 实现同一协议（schemaVersion 1）。任何一侧修改都必须同步
另一侧，以及两种语言共享的用例 scripts/tests/fixtures/asset-manifest/。

协议要点（全部 fail closed）：
- 顶层只允许 schemaVersion / assets / synthetic；未知字段、缺字段、类型不符一律拒绝。
- kind 只允许 ocr-model / ort-runtime / dictionary-input，每种 kind 的 identity 字段固定。
- id、repo、文件集合、下载 URL、归档内路径都由 identity 推导，清单值必须逐字相等；
  因而 URL 必然包含不可变 revision（40 位 commit）或版本号，不可能指向 main/master。
- 每个文件必须给出正整数 size 与 64 位小写十六进制 sha256。
- approval.status 只能是 approved，且必须有审批人、日期、方法和非空证据列表。
- 带 "synthetic": true 的清单只用于测试，默认（allow_synthetic=False）拒绝加载。
- JSON 层：同一对象内大小写不敏感的重复键、NaN/Infinity、非 UTF-8 均拒绝。

本模块只依赖标准库，不联网、不下载、不写文件。
"""

from __future__ import annotations

import datetime
import hashlib
import json
import re
from pathlib import Path
from typing import Any, Mapping

SCHEMA_VERSION = 1
# 正式清单的约定位置。本模块不创建该文件，正式内容由工单04人工审批后产出。
DEFAULT_MANIFEST_PATH = Path(__file__).resolve().parent / "release-assets.json"

KINDS = ("ocr-model", "ort-runtime", "dictionary-input")
MODEL_PROFILES = ("tiny", "small", "medium")
MODEL_ROLES = ("det", "rec")
MODEL_FILES = ("inference.onnx", "inference.json", "inference.yml")
ORT_PLATFORMS = ("win",)
ORT_ARCHES = ("x64",)
DICTIONARY_SOURCES = ("ecdict", "wordnet")
APPROVAL_STATUSES = ("approved",)
ECDICT_REPO = "skywind3000/ECDICT"
# 与 PowerShell 侧 Int64/双精度解析保持同一结论的上限（2^53-1）。
MAX_SIZE = 9007199254740991

# 每种 kind 用于查找条目的选择键（不含 revision 等由清单钉住的值）。
SELECTOR_KEYS = {
    "ocr-model": ("profile", "role"),
    "ort-runtime": ("version", "platform", "arch"),
    "dictionary-input": ("source",),
}

# 统一使用 fullmatch 与显式 ASCII 字符类：避免 `$` 匹配末尾换行、`\d` 匹配非 ASCII 数字。
_SHA256 = re.compile(r"[0-9a-f]{64}")
_REVISION = re.compile(r"[0-9a-f]{40}")
_SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
_MAJOR_MINOR = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
_DATE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")


class AssetManifestError(ValueError):
    """清单不合法、条目未批准，或本地文件与清单不符时抛出。"""


def _fail(where: str, message: str) -> None:
    raise AssetManifestError(f"[asset-manifest] {where}: {message}")


# ---------------------------------------------------------------------------
# 基础断言（错误文本与 PowerShell 实现保持一致，共享用例按子串比对）
# ---------------------------------------------------------------------------


def _object(value: Any, where: str, required: tuple[str, ...], optional: tuple[str, ...] = ()) -> None:
    if not isinstance(value, dict):
        _fail(where, "must be an object")
    for key in value:
        if key not in required and key not in optional:
            _fail(where, f"unknown field '{key}'")
    for key in required:
        if key not in value:
            _fail(where, f"missing field '{key}'")


def _string(value: Any, where: str) -> None:
    if not isinstance(value, str):
        _fail(where, "must be a string")
    if not value.strip():
        _fail(where, "must be a non-empty string")


def _pattern(value: Any, where: str, pattern: re.Pattern[str], description: str) -> None:
    _string(value, where)
    if pattern.fullmatch(value) is None:
        _fail(where, f"must match {description}")


def _enum(value: Any, where: str, allowed: tuple[str, ...]) -> None:
    _string(value, where)
    if value not in allowed:
        _fail(where, "must be one of: " + ", ".join(allowed))


def _equal(value: Any, where: str, expected: str) -> None:
    _string(value, where)
    if value != expected:
        _fail(where, f"must equal '{expected}'")


def _positive_int(value: Any, where: str) -> None:
    # bool 是 int 子类必须显式排除；浮点（含 1.0）一律拒绝。
    if type(value) is not int or value <= 0 or value > MAX_SIZE:
        _fail(where, "must be a positive integer")


# ---------------------------------------------------------------------------
# JSON 解析钩子
# ---------------------------------------------------------------------------


def _pairs_hook(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    # PowerShell 键查找默认大小写不敏感；两侧统一拒绝仅大小写不同的重复键，消除歧义。
    seen: set[str] = set()
    for key, _ in pairs:
        folded = key.lower()
        if folded in seen:
            _fail("$", f"duplicate key '{key}'")
        seen.add(folded)
    return dict(pairs)


def _reject_constant(name: str) -> None:
    _fail("$", f"non-standard JSON constant '{name}' is not allowed")


def parse_manifest_text(text: str) -> Any:
    """严格解析 JSON 文本（不做 schema 校验）。"""
    if text.startswith("\ufeff"):
        text = text[1:]
    try:
        return json.loads(text, object_pairs_hook=_pairs_hook, parse_constant=_reject_constant)
    except json.JSONDecodeError as error:
        _fail("$", f"invalid JSON ({error.msg})")


# ---------------------------------------------------------------------------
# 结构校验
# ---------------------------------------------------------------------------


def _expected_layout(kind: str, identity: Any, where: str) -> tuple[str, list[dict[str, str]]]:
    """按 kind 校验 identity，并返回 (规范 id, 期望文件列表)。

    期望文件只含 name 与定位字段：url（直接下载）或 fromArchive + archivePath（取自归档）。
    """
    if kind == "ocr-model":
        _object(identity, where, ("profile", "role", "repo", "revision"))
        _enum(identity["profile"], f"{where}.profile", MODEL_PROFILES)
        _enum(identity["role"], f"{where}.role", MODEL_ROLES)
        repo = f"PaddlePaddle/PP-OCRv6_{identity['profile']}_{identity['role']}_onnx"
        _equal(identity["repo"], f"{where}.repo", repo)
        _pattern(identity["revision"], f"{where}.revision", _REVISION, "40-character lowercase hex commit")
        revision = identity["revision"]
        files = [
            {"name": name, "url": f"https://huggingface.co/{repo}/resolve/{revision}/{name}"}
            for name in MODEL_FILES
        ]
        return f"ocr-model/{identity['profile']}/{identity['role']}", files

    if kind == "ort-runtime":
        _object(identity, where, ("version", "platform", "arch"))
        _pattern(identity["version"], f"{where}.version", _SEMVER, "MAJOR.MINOR.PATCH")
        _enum(identity["platform"], f"{where}.platform", ORT_PLATFORMS)
        _enum(identity["arch"], f"{where}.arch", ORT_ARCHES)
        version = identity["version"]
        package = f"onnxruntime-{identity['platform']}-{identity['arch']}-{version}"
        files = [
            {
                "name": f"{package}.zip",
                "url": f"https://github.com/microsoft/onnxruntime/releases/download/v{version}/{package}.zip",
            },
            {
                "name": "onnxruntime.dll",
                "fromArchive": f"{package}.zip",
                "archivePath": f"{package}/lib/onnxruntime.dll",
            },
        ]
        return f"ort-runtime/{version}/{identity['platform']}-{identity['arch']}", files

    # dictionary-input：ECDICT 与 WordNet 是两个独立输入，identity 形状按 source 区分。
    if not isinstance(identity, dict):
        _fail(where, "must be an object")
    if "source" not in identity:
        _fail(where, "missing field 'source'")
    _enum(identity["source"], f"{where}.source", DICTIONARY_SOURCES)
    if identity["source"] == "ecdict":
        _object(identity, where, ("source", "repo", "revision"))
        _equal(identity["repo"], f"{where}.repo", ECDICT_REPO)
        _pattern(identity["revision"], f"{where}.revision", _REVISION, "40-character lowercase hex commit")
        files = [
            {
                "name": "ecdict.csv",
                "url": f"https://raw.githubusercontent.com/{ECDICT_REPO}/{identity['revision']}/ecdict.csv",
            }
        ]
        return "dictionary-input/ecdict", files

    _object(identity, where, ("source", "version"))
    _pattern(identity["version"], f"{where}.version", _MAJOR_MINOR, "MAJOR.MINOR")
    version = identity["version"]
    files = [
        {
            "name": f"WNdb-{version}.tar.gz",
            "url": f"https://wordnetcode.princeton.edu/{version}/WNdb-{version}.tar.gz",
        }
    ]
    return "dictionary-input/wordnet", files


def _validate_files(files: Any, where: str, expected: list[dict[str, str]]) -> None:
    if not isinstance(files, list):
        _fail(where, "must be an array")
    by_name = {entry["name"]: entry for entry in expected}
    seen: set[str] = set()
    for index, entry in enumerate(files):
        fw = f"{where}[{index}]"
        if not isinstance(entry, dict):
            _fail(fw, "must be an object")
        if "name" not in entry:
            _fail(fw, "missing field 'name'")
        name = entry["name"]
        _string(name, f"{fw}.name")
        if name not in by_name:
            _fail(f"{fw}.name", f"unexpected file '{name}'")
        if name in seen:
            _fail(f"{fw}.name", f"duplicate file '{name}'")
        seen.add(name)
        exp = by_name[name]
        locator = tuple(key for key in exp if key != "name")
        _object(entry, fw, ("name",) + locator + ("size", "sha256"))
        for key in locator:
            _equal(entry[key], f"{fw}.{key}", exp[key])
        _positive_int(entry["size"], f"{fw}.size")
        _pattern(entry["sha256"], f"{fw}.sha256", _SHA256, "64-character lowercase hex")
    for exp in expected:
        if exp["name"] not in seen:
            _fail(where, f"missing required file '{exp['name']}'")


def _validate_approval(approval: Any, where: str) -> None:
    _object(approval, where, ("status", "approvedBy", "approvedAt", "method", "evidence"))
    _enum(approval["status"], f"{where}.status", APPROVAL_STATUSES)
    _string(approval["approvedBy"], f"{where}.approvedBy")
    approved_at = approval["approvedAt"]
    _pattern(approved_at, f"{where}.approvedAt", _DATE, "YYYY-MM-DD")
    try:
        datetime.date(int(approved_at[0:4]), int(approved_at[5:7]), int(approved_at[8:10]))
    except ValueError:
        _fail(f"{where}.approvedAt", "must be a valid calendar date")
    _string(approval["method"], f"{where}.method")
    evidence = approval["evidence"]
    if not isinstance(evidence, list):
        _fail(f"{where}.evidence", "must be an array")
    if not evidence:
        _fail(f"{where}.evidence", "must not be empty")
    for index, item in enumerate(evidence):
        _string(item, f"{where}.evidence[{index}]")


def _validate_asset(asset: Any, where: str) -> None:
    _object(asset, where, ("id", "kind", "identity", "files", "approval"))
    kind = asset["kind"]
    _string(kind, f"{where}.kind")
    if kind not in KINDS:
        _fail(f"{where}.kind", f"unknown kind '{kind}'")
    expected_id, expected_files = _expected_layout(kind, asset["identity"], f"{where}.identity")
    _equal(asset["id"], f"{where}.id", expected_id)
    _validate_files(asset["files"], f"{where}.files", expected_files)
    _validate_approval(asset["approval"], f"{where}.approval")


def validate_manifest(data: Any, *, allow_synthetic: bool = False) -> dict[str, Any]:
    """校验已解析的清单对象；合法时原样返回，否则抛出 AssetManifestError。"""
    _object(data, "$", ("schemaVersion", "assets"), ("synthetic",))
    schema_version = data["schemaVersion"]
    if type(schema_version) is not int or schema_version != SCHEMA_VERSION:
        _fail("$.schemaVersion", f"unsupported schemaVersion (expected {SCHEMA_VERSION})")
    if "synthetic" in data:
        if data["synthetic"] is not True:
            _fail("$.synthetic", "must be true when present")
        if not allow_synthetic:
            _fail("$.synthetic", "synthetic test manifest is not trusted production data")
    assets = data["assets"]
    if not isinstance(assets, list):
        _fail("$.assets", "must be an array")
    if not assets:
        _fail("$.assets", "must not be empty")
    seen_ids: set[str] = set()
    for index, asset in enumerate(assets):
        where = f"$.assets[{index}]"
        _validate_asset(asset, where)
        if asset["id"] in seen_ids:
            _fail(f"{where}.id", f"duplicate asset id '{asset['id']}'")
        seen_ids.add(asset["id"])
    return data


def load_manifest(path: str | Path = DEFAULT_MANIFEST_PATH, *, allow_synthetic: bool = False) -> dict[str, Any]:
    """读取并严格校验清单文件。生产调用方不得传 allow_synthetic=True。"""
    manifest_path = Path(path)
    if not manifest_path.is_file():
        _fail(str(manifest_path), "manifest file not found")
    try:
        text = manifest_path.read_bytes().decode("utf-8")
    except UnicodeDecodeError:
        _fail(str(manifest_path), "invalid UTF-8")
    return validate_manifest(parse_manifest_text(text), allow_synthetic=allow_synthetic)


# ---------------------------------------------------------------------------
# 查询与内容校验（供 25–27 接入）
# ---------------------------------------------------------------------------


def find_asset(manifest: Mapping[str, Any], kind: str, selector: Mapping[str, str]) -> dict[str, Any]:
    """按选择键精确查找已批准条目；找不到即拒绝（未批准的 profile / 版本 / 架构 / override）。

    manifest 必须来自 load_manifest / validate_manifest。selector 的键集合必须等于
    SELECTOR_KEYS[kind]（多键、少键都拒绝），值逐字比较；别名（如 lite）须由调用方先归一。
    """
    if kind not in SELECTOR_KEYS:
        _fail("selector", f"unknown kind '{kind}'")
    keys = SELECTOR_KEYS[kind]
    if set(selector) != set(keys):
        _fail("selector", f"kind '{kind}' requires exactly: " + ", ".join(keys))
    label = ", ".join(f"{key}={selector[key]}" for key in keys)
    for asset in manifest["assets"]:
        if asset["kind"] == kind and all(asset["identity"].get(key) == selector[key] for key in keys):
            return asset
    _fail("$.assets", f"no approved asset for kind '{kind}' with {label}")
    raise AssertionError("unreachable")


def get_file(asset: Mapping[str, Any], name: str) -> dict[str, Any]:
    """返回条目中名为 name 的文件记录（含 size/sha256 以及 url 或 fromArchive/archivePath）。"""
    for entry in asset["files"]:
        if entry["name"] == name:
            return entry
    _fail(f"asset '{asset['id']}'", f"no file '{name}'")
    raise AssertionError("unreachable")


def sha256_file(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_file(path: str | Path, file_entry: Mapping[str, Any]) -> None:
    """校验本地文件大小与 SHA-256，不符抛出 AssetManifestError。先比大小，避免对截断文件算摘要。"""
    file_path = Path(path)
    if not file_path.is_file():
        _fail(str(file_path), "file not found")
    expected_size = file_entry["size"]
    actual_size = file_path.stat().st_size
    if actual_size != expected_size:
        _fail(str(file_path), f"size mismatch (expected {expected_size}, got {actual_size})")
    expected_hash = file_entry["sha256"]
    actual_hash = sha256_file(file_path)
    if actual_hash != expected_hash:
        _fail(str(file_path), f"sha256 mismatch (expected {expected_hash}, got {actual_hash})")


def file_matches(path: str | Path, file_entry: Mapping[str, Any]) -> bool:
    """缓存 / 目标复验用：缺失、大小或摘要不符返回 False；其他 I/O 异常照常抛出。"""
    try:
        verify_file(path, file_entry)
    except AssetManifestError:
        return False
    return True
