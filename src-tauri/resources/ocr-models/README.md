# OCR Models

Bundled PP-OCRv6 ONNX model files live here. Only the `small` profile is
supported by the product; this directory is generated and mostly ignored by Git.

Expected layout:

```text
ocr-models/
  small/
    det/
      inference.onnx
      inference.json
      inference.yml
    rec/
      inference.onnx
      inference.json
      inference.yml
```

Prepare it on any platform (pinned upstream revisions and SHA-256 checks):

```bash
npm run ocr:models
```

Windows can also use the original PowerShell path:

```bash
npm run ocr:models:win -- -Profile small
```

The app locates `det/inference.onnx`, `rec/inference.onnx` and
`rec/inference.yml` at runtime. Models are bundled into the installed app;
developers must run the preparation command before a release build.
