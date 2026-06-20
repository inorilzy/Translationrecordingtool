# OCR Models

Put bundled PP-OCRv6 ONNX model profiles here when building a fully offline OCR package.

Expected layout:

```text
ocr-models/
  tiny/
    det/
    rec/
  small/
    det/
    rec/
  medium/
    det/
    rec/
```

The app uses `det` and `rec` when both directories contain `inference.onnx`. Use
`npm run ocr:models:win -- -Profile small` to download the recommended profile
from the official PaddlePaddle Hugging Face PP-OCRv6 collection.
