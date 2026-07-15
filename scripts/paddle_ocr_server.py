#!/usr/bin/env python3
import argparse
import base64
import binascii
import inspect
import json
import sys
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


PPOCRV6_MODEL_NAMES = {
    "tiny": ("PP-OCRv6_tiny_det", "PP-OCRv6_tiny_rec"),
    "small": ("PP-OCRv6_small_det", "PP-OCRv6_small_rec"),
    "medium": ("PP-OCRv6_medium_det", "PP-OCRv6_medium_rec"),
}

LEGACY_PROFILE_MAP = {
    "lite": "tiny",
    "standard": "small",
    "accurate": "medium",
}


def configure_frozen_import_paths() -> None:
    bundle_dir = getattr(sys, "_MEIPASS", None)
    if not bundle_dir:
        return

    paddleocr_dir = Path(bundle_dir) / "paddleocr"
    if paddleocr_dir.exists():
        sys.path.insert(0, str(paddleocr_dir))


def clean_text_lines(lines: list[str]) -> list[str]:
    return [line.strip() for line in lines if line and line.strip()]


def collect_text(value: Any) -> list[str]:
    if value is None:
        return []

    if isinstance(value, str):
        return [value]

    if isinstance(value, dict):
        for key in ("rec_texts", "texts", "text", "recText", "transcription", "words"):
            if key in value:
                return collect_text(value[key])
        lines: list[str] = []
        for item in value.values():
            lines.extend(collect_text(item))
        return lines

    if isinstance(value, (list, tuple)):
        lines: list[str] = []
        for item in value:
            if isinstance(item, (list, tuple)) and item:
                # Old PaddleOCR often returns [box, (text, score)].
                candidate = item[1] if len(item) > 1 else item[0]
                if isinstance(candidate, (list, tuple)) and candidate:
                    lines.extend(collect_text(candidate[0]))
                    continue
            lines.extend(collect_text(item))
        return lines

    return []


def result_to_jsonable(result: Any) -> Any:
    if hasattr(result, "json"):
        value = result.json
        return value() if callable(value) else value
    if hasattr(result, "res"):
        value = result.res
        return value() if callable(value) else value
    return result


def run_ocr(ocr: Any, image_path: Path) -> list[Any]:
    if hasattr(ocr, "ocr"):
        return ocr.ocr(str(image_path))
    if hasattr(ocr, "predict"):
        return ocr.predict(str(image_path))
    raise RuntimeError("PaddleOCR instance has neither ocr() nor predict()")


def run_rapid_ocr(ocr: Any, image_path: Path) -> Any:
    if callable(ocr):
        return ocr(str(image_path))
    if hasattr(ocr, "detect"):
        return ocr.detect(str(image_path))
    raise RuntimeError("RapidOCR instance has neither __call__() nor detect()")


def recognize_text(ocr: Any, image_path: Path, engine: str) -> tuple[list[str], list[Any]]:
    prediction = run_rapid_ocr(ocr, image_path) if engine == "rapidocr" else run_ocr(ocr, image_path)
    raw_results = [result_to_jsonable(item) for item in prediction] if isinstance(prediction, (list, tuple)) else [result_to_jsonable(prediction)]
    lines: list[str] = []
    for item in raw_results:
        lines.extend(collect_text(item))
    return clean_text_lines(lines), raw_results


class OcrServer(BaseHTTPRequestHandler):
    ocr: Any = None

    def do_GET(self) -> None:
        if self.path == "/health":
            self.send_json({
                "ok": True,
                "engine": getattr(self.server, "ocr_engine", None),
                "lang": getattr(self.server, "ocr_lang", None),
                "device": getattr(self.server, "ocr_device", None),
                "modelProfile": getattr(self.server, "ocr_model_profile", None),
                "modelDir": getattr(self.server, "ocr_model_dir", None),
                "modelSource": getattr(self.server, "ocr_model_source", None),
            })
            return
        self.send_error(404, "Not found")

    def do_POST(self) -> None:
        if self.path == "/warmup":
            self.send_json({"ok": True, "warmed": True})
            return

        if self.path != "/ocr":
            self.send_error(404, "Not found")
            return

        try:
            body_length = int(self.headers.get("Content-Length", "0"))
            payload = json.loads(self.rfile.read(body_length))
            image = payload.get("image", "")
            if not image:
                raise ValueError("missing image")

            if "," in image:
                image = image.split(",", 1)[1]

            try:
                image_bytes = base64.b64decode(image, validate=True)
            except binascii.Error as exc:
                raise ValueError("invalid base64 image") from exc

            if not image_bytes:
                raise ValueError("empty image")

            with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tmp:
                tmp.write(image_bytes)
                image_path = Path(tmp.name)

            try:
                lines, _raw_results = recognize_text(
                    self.ocr,
                    image_path,
                    getattr(self.server, "ocr_engine", "paddleocr"),
                )
            finally:
                image_path.unlink(missing_ok=True)

            self.send_json({
                "text": "\n".join(lines),
                "result": [{"recText": line} for line in lines],
            })
        except Exception as exc:
            self.send_json({"error": str(exc)}, status=500)

    def log_message(self, format: str, *args: Any) -> None:
        engine = getattr(self.server, "ocr_engine", "ocr")
        print(f"[{engine}] " + format % args)

    def send_json(self, payload: dict[str, Any], status: int = 200) -> None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def build_ocr(args: argparse.Namespace) -> Any:
    if args.engine == "rapidocr":
        return build_rapid_ocr()

    configure_frozen_import_paths()
    return build_paddle_ocr(args)


def build_paddle_ocr(args: argparse.Namespace) -> Any:

    try:
        from paddleocr import PaddleOCR
    except ImportError as exc:
        raise SystemExit(
            f"未安装或无法加载 paddleocr。请先运行: pip install paddleocr onnxruntime。原始错误: {exc}"
        ) from exc

    init_params = inspect.signature(PaddleOCR.__init__).parameters
    profile = normalize_model_profile(args.model_profile)
    model_dir = Path(args.model_dir).resolve() if args.model_dir else None
    if model_dir and not model_dir.exists():
        raise SystemExit(f"指定的 PaddleOCR 本地模型目录不存在: {model_dir}")
    if profile == "official":
        if model_dir is not None:
            raise SystemExit("PaddleOCR official 配置不接受 --model-dir；请移除本地模型目录参数。")
        if not args.allow_official_model_download:
            raise SystemExit(
                "PaddleOCR official 配置必须显式传入 --allow-official-model-download。"
            )
    elif model_dir is None:
        raise SystemExit(
            f"PaddleOCR {profile} 配置缺少本地模型目录。请传入 --model-dir。"
        )
    use_local_models = model_dir is not None
    kwargs: dict[str, Any] = {} if use_local_models else {"lang": args.lang}
    has_var_kwargs = accepts_var_kwargs(init_params)

    if "device" in init_params or has_var_kwargs:
        kwargs["device"] = args.device
    else:
        kwargs["use_gpu"] = args.device.lower().startswith("gpu")
        kwargs["show_log"] = False

    version_params = [] if use_local_models else [("ocr_version", "PP-OCRv6")]
    for key, value in (
        *version_params,
        ("engine", "onnxruntime"),
        ("enable_hpi", False),
        ("use_tensorrt", False),
    ):
        if key in init_params or has_var_kwargs:
            kwargs[key] = value

    for key in (
        "use_doc_orientation_classify",
        "use_doc_unwarping",
        "use_textline_orientation",
    ):
        if key in init_params:
            kwargs[key] = False

    if profile != "official":
        det_model_name, rec_model_name = PPOCRV6_MODEL_NAMES[profile]
        if "text_detection_model_name" in init_params:
            kwargs["text_detection_model_name"] = det_model_name
        if "text_recognition_model_name" in init_params:
            kwargs["text_recognition_model_name"] = rec_model_name

    if model_dir and model_dir.exists():
        model_candidates = {
            "text_detection_model_dir": model_dir / "det",
            "text_recognition_model_dir": model_dir / "rec",
            "textline_orientation_model_dir": model_dir / "cls",
            "det_model_dir": model_dir / "det",
            "rec_model_dir": model_dir / "rec",
            "cls_model_dir": model_dir / "cls",
        }
        for key, path in model_candidates.items():
            if key in init_params and path.exists():
                kwargs[key] = str(path)

    return PaddleOCR(**kwargs)


def accepts_var_kwargs(params: dict[str, inspect.Parameter]) -> bool:
    return any(param.kind == inspect.Parameter.VAR_KEYWORD for param in params.values())


def normalize_model_profile(profile: str) -> str:
    value = (profile or "small").strip().lower()
    value = LEGACY_PROFILE_MAP.get(value, value)
    return value if value == "official" or value in PPOCRV6_MODEL_NAMES else "small"


def build_rapid_ocr() -> Any:
    try:
        from rapidocr_onnxruntime import RapidOCR
    except ImportError as exc:
        raise SystemExit(
            f"未安装或无法加载 rapidocr-onnxruntime。请先运行: pip install rapidocr-onnxruntime onnxruntime。原始错误: {exc}"
        ) from exc

    return RapidOCR()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Local OCR HTTP server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8866)
    parser.add_argument("--engine", choices=["paddleocr", "rapidocr"], default="paddleocr")
    parser.add_argument("--lang", default="ch")
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--model-profile", default="small")
    parser.add_argument("--model-dir", default="")
    parser.add_argument("--allow-official-model-download", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    print(f"initializing {args.engine} OCR runtime...", flush=True)
    OcrServer.ocr = build_ocr(args)
    server = ThreadingHTTPServer((args.host, args.port), OcrServer)
    server.ocr_engine = args.engine
    server.ocr_lang = args.lang
    server.ocr_device = args.device
    server.ocr_model_profile = "embedded" if args.engine == "rapidocr" else args.model_profile
    server.ocr_model_dir = args.model_dir
    server.ocr_model_source = (
        "embedded" if args.engine == "rapidocr"
        else "local" if args.model_dir
        else "official-download"
    )
    print(
        f"{args.engine} HTTP server listening on http://{args.host}:{args.port}/ocr "
        f"(model source: {server.ocr_model_source})",
        flush=True,
    )
    server.serve_forever()


if __name__ == "__main__":
    main()
