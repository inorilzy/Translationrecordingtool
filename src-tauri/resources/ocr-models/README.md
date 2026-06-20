# OCR Models

Put bundled PaddleOCR model profiles here when building a fully offline OCR package.

Expected layout:

```text
ocr-models/
  lite/
    det/
    rec/
    cls/
  standard/
    det/
    rec/
    cls/
  accurate/
    det/
    rec/
    cls/
```

The app uses `det` and `rec` when both directories contain model files. `cls` is optional.
