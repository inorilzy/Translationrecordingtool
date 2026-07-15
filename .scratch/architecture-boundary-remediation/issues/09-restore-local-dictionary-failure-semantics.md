# 09 — Restore local dictionary failure semantics

**What to build:** Keep local dictionary failures distinct from genuine misses across manual, clipboard, selected-text, and screenshot translation, so corrupted or unavailable local data fails clearly instead of silently sending the request to online services.

**Blocked by:** None — can start immediately.

**Status:** completed

- [x] Every production translation entry point receives the same explicit local dictionary failure through the shared workflow.
- [x] Only a successful lookup with no result proceeds to Free Dictionary and configured-provider fallback.
- [x] A local dictionary failure does not call enrichment or translation-provider collaborators.
- [x] Existing local-hit, genuine-miss, provider fallback, and user-visible error behavior remains compatible.
- [x] Deterministic workflow tests distinguish local hits, local misses, and local lookup failures.
