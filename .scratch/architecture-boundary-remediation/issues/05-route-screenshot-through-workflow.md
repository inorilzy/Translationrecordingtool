# 05 — Route screenshot OCR through the unified workflow

**What to build:** Preserve the complete screenshot translation journey—region selection, OCR progress, recognized-text backfill, translation, persistence, and anchored popup display—while using the acyclic OCR facade, backend runtime settings, and shared translation policy.

**Blocked by:** 02 — Break the OCR dependency cycle; 03 — Unify manual and clipboard translation workflow.

**Status:** ready-for-agent

- [ ] Screenshot selection and result anchoring retain their current user-visible behavior.
- [ ] OCR engine, endpoint, model profile, and translation provider configuration are read from backend-managed runtime settings.
- [ ] The frontend no longer sends OCR or provider configuration with the screenshot translation request.
- [ ] OCR progress, translation progress, completion, cancellation, and failure are exposed as observable workflow stages.
- [ ] Recognized text is trimmed, rejected explicitly when empty, and backfilled into the main translation input when valid.
- [ ] Valid OCR text enters the same dictionary, enrichment, provider, and fallback policy used by other translation entry points.
- [ ] Native ONNX and compatibility-sidecar OCR routes both work through the new facade.
- [ ] Stale or cancelled screenshot requests cannot update the popup or main input.
- [ ] Tests cover selection cancellation, OCR failure, empty OCR output, successful translation, text backfill, popup anchoring, and provider failure.
