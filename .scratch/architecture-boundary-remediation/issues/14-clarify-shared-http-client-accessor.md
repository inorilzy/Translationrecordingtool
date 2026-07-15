# 14 — Clarify the shared HTTP client accessor

**What to build:** Give the shared HTTP client accessor a name that clearly communicates client retrieval rather than an HTTP GET request, while preserving one pooled client and all current request behavior.

**Blocked by:** None — can start immediately.

**Status:** completed

- [x] The accessor name clearly identifies a shared or reusable HTTP client.
- [x] Every OCR and translation-provider caller uses the renamed accessor.
- [x] Client singleton ownership, connection pooling, timeout policy, headers, request bodies, and error behavior remain unchanged.
- [x] No compatibility alias or duplicate accessor remains after the rename.
- [x] Backend compilation and focused OCR/provider tests pass with the final name.
