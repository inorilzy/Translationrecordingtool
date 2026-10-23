"""离线合成测试，不访问网络或用户缓存；运行 python -B -m unittest discover -s scripts/tests。"""
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import asset_manifest as am

FIXTURES = Path(__file__).parent / "fixtures" / "asset-manifest"


class ManifestTests(unittest.TestCase):
    def setUp(self):
        self.base = json.loads((FIXTURES / "synthetic-valid.json").read_text(encoding="utf-8"))
        self.cases = json.loads((FIXTURES / "cases.json").read_text(encoding="utf-8"))

    def test_fixture_requires_explicit_test_opt_in(self):
        path = FIXTURES / "synthetic-valid.json"
        with self.assertRaises(am.AssetManifestError):
            am.load_manifest(path)
        manifest = am.load_manifest(path, allow_synthetic=True)
        self.assertEqual(len(manifest["assets"]), 5)

    def test_shared_mutations(self):
        for case in self.cases["mutationCases"]:
            with self.subTest(case=case["name"]):
                data = copy.deepcopy(self.base)
                for op in case["ops"]:
                    parent = data
                    for key in op["path"][:-1]:
                        parent = parent[key]
                    key = op["path"][-1]
                    if op["op"] == "remove":
                        del parent[key]
                    elif op["op"] == "copy":
                        value = data
                        for source in op["from"]:
                            value = value[source]
                        parent[key] = copy.deepcopy(value)
                    else:
                        parent[key] = copy.deepcopy(op["value"])
                with self.assertRaises(am.AssetManifestError) as caught:
                    am.validate_manifest(data, allow_synthetic=True)
                self.assertIn(case["expectError"], str(caught.exception))

    def test_shared_raw_cases(self):
        for case in self.cases["rawCases"]:
            with self.subTest(case=case["name"]):
                with self.assertRaises(am.AssetManifestError) as caught:
                    am.validate_manifest(am.parse_manifest_text(case["text"]), allow_synthetic=True)
                self.assertIn(case["expectError"], str(caught.exception))

    def test_selection(self):
        manifest = am.validate_manifest(self.base, allow_synthetic=True)
        for kind, selector in [
            ("ocr-model", {"profile": "small", "role": "det"}),
            ("ort-runtime", {"version": "1.20.1", "platform": "win", "arch": "x64"}),
            ("dictionary-input", {"source": "ecdict"}),
            ("dictionary-input", {"source": "wordnet"}),
        ]:
            self.assertEqual(am.find_asset(manifest, kind, selector)["kind"], kind)
        for selector in ({"profile": "tiny", "role": "det"}, {"profile": "small"},
                         {"profile": "Small", "role": "det"}):
            with self.assertRaises(am.AssetManifestError):
                am.find_asset(manifest, "ocr-model", selector)

    def test_content_and_missing_manifest(self):
        # 摘要仅来自本测试创建的三个字节，不是生产资产摘要。
        with tempfile.TemporaryDirectory(prefix="asset-manifest-test-") as directory:
            path = Path(directory) / "synthetic.bin"
            entry = {"size": 3, "sha256": hashlib.sha256(b"abc").hexdigest()}
            self.assertFalse(am.file_matches(path, entry))
            with self.assertRaises(am.AssetManifestError):
                am.load_manifest(path)
            path.write_bytes(b"abc")
            am.verify_file(path, entry)
            path.write_bytes(b"abd")
            self.assertFalse(am.file_matches(path, entry))
            path.write_bytes(b"ab")
            self.assertFalse(am.file_matches(path, entry))
            path.write_bytes(b"\xff")
            with self.assertRaisesRegex(am.AssetManifestError, "invalid UTF-8"):
                am.load_manifest(path)


if __name__ == "__main__":
    unittest.main()
