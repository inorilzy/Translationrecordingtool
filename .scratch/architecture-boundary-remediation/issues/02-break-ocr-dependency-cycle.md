# 02 — Break the OCR dependency cycle

**What to build:** Preserve native and compatibility OCR behavior while reorganizing OCR configuration, facade, lifecycle, and engine adapters into a one-way dependency structure. Users must retain recognition, status, warmup, restart, model discovery, and compatibility-sidecar behavior.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] OCR configuration, status, and engine contracts are owned independently from facade and adapter implementations.
- [ ] The OCR facade selects and coordinates native or compatibility-sidecar behavior through one-way dependencies.
- [ ] Native OCR and sidecar OCR adapters do not import their facade or lifecycle owner.
- [ ] The OCR module graph has no circular dependency and the Rust project compiles with the new direction.
- [ ] Native ONNX recognition, status, warmup, model discovery, and error behavior remain compatible.
- [ ] PaddleOCR and RapidOCR compatibility-sidecar health, startup, restart, and logging behavior remain compatible.
- [ ] Native, lite, and full packaging configurations still resolve their intended runtime resources.
- [ ] Tests verify observable adapter selection and lifecycle behavior without asserting private module calls.
