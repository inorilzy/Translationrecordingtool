#!/usr/bin/env python3
import argparse
import base64
import binascii
import json
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


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


class PaddleOcrServer(BaseHTTPRequestHandler):
    ocr: Any = None

    def do_GET(self) -> None:
        if self.path == "/health":
            self.send_json({
                "ok": True,
                "engine": "paddleocr",
                "lang": getattr(self.server, "ocr_lang", None),
                "device": getattr(self.server, "ocr_device", None),
            })
            return
        self.send_error(404, "Not found")

    def do_POST(self) -> None:
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
                prediction = self.ocr.predict(str(image_path))
            finally:
                image_path.unlink(missing_ok=True)

            raw_results = [result_to_jsonable(item) for item in prediction]
            lines: list[str] = []
            for item in raw_results:
                lines.extend(collect_text(item))

            lines = clean_text_lines(lines)
            self.send_json({
                "text": "\n".join(lines),
                "result": [{"recText": line} for line in lines],
            })
        except Exception as exc:
            self.send_json({"error": str(exc)}, status=500)

    def log_message(self, format: str, *args: Any) -> None:
        print("[paddle-ocr] " + format % args)

    def send_json(self, payload: dict[str, Any], status: int = 200) -> None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def build_ocr(args: argparse.Namespace) -> Any:
    try:
        from paddleocr import PaddleOCR
    except ImportError as exc:
        raise SystemExit(
            "未安装 paddleocr。请先运行: pip install paddleocr paddlepaddle"
        ) from exc

    return PaddleOCR(
        lang=args.lang,
        device=args.device,
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=False,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Local PaddleOCR v6 HTTP server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8866)
    parser.add_argument("--lang", default="ch")
    parser.add_argument("--device", default="cpu")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    PaddleOcrServer.ocr = build_ocr(args)
    server = ThreadingHTTPServer((args.host, args.port), PaddleOcrServer)
    server.ocr_lang = args.lang
    server.ocr_device = args.device
    print(f"PaddleOCR HTTP server listening on http://{args.host}:{args.port}/ocr")
    server.serve_forever()


if __name__ == "__main__":
    main()
