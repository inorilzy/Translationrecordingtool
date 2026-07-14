# 07 — Consolidate popup event ownership

**What to build:** Give popup translation and theme events one frontend owner while keeping Escape, close, drag, cleanup, ready signaling, staged results, and main-window navigation unchanged for the user.

**Blocked by:** 04 — Add staged selected-text shortcut workflow; 05 — Route screenshot OCR through the unified workflow.

**Status:** ready-for-agent

- [ ] The popup component is the sole owner of theme and staged translation event handling.
- [ ] The popup controls boundary owns only Escape handling, close, drag, cleanup, and ready signaling.
- [ ] Duplicate no-op subscriptions to theme and translation events are removed.
- [ ] Each emitted workflow stage produces at most one popup state transition.
- [ ] Listener registration and cleanup occur exactly once per popup lifecycle.
- [ ] Loading, enriched-result, error, favorite, audio, and open-main-window behavior remains compatible.
- [ ] Existing theme changes still update an open popup.
- [ ] Existing popup runtime and component tests cover the final event ownership without asserting private listener implementation details.
