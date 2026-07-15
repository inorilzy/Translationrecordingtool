# 11 — Restore explicit OCR compatibility profiles

**What to build:** Preserve native ONNX, PaddleOCR, and RapidOCR as explicit runtime choices whose engine and model sources are resolved predictably, without a discovered sidecar silently changing the configured engine.

**Blocked by:** None — can start immediately.

**Status:** completed

- [x] Native ONNX remains the default when packaged native resources are available.
- [x] An explicit RapidOCR configuration remains RapidOCR even when another sidecar binary is discoverable.
- [x] PaddleOCR and RapidOCR startup, health, restart, recognition, and logging use the engine actually selected for the profile.
- [x] Each supported profile declares where its required model resources come from.
- [x] Missing required local resources fail before recognition starts with an actionable error.
- [x] Any profile that intentionally permits official model download makes that behavior explicit and verifies it rather than relying on an implicit library fallback.
- [x] Deterministic tests cover engine selection for native, lite, full, PaddleOCR, and RapidOCR configurations.
