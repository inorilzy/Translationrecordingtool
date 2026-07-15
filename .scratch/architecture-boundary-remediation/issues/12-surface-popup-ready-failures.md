# 12 — Surface popup-ready failures

**What to build:** Ensure a popup that cannot signal readiness fails visibly and terminates its loading state instead of swallowing the error and waiting forever for queued translation events.

**Blocked by:** None — can start immediately.

**Status:** completed

- [x] Ready signaling still occurs only after all popup theme and translation listeners are registered.
- [x] A ready-event failure is propagated to the popup component rather than converted into success.
- [x] The popup leaves loading state and presents a terminal, actionable error when readiness cannot be established.
- [x] The backend does not treat a failed ready signal as permission to flush deferred stages.
- [x] Successful ready signaling, staged results, cleanup, Escape, close, drag, and main-window navigation remain compatible.
- [x] Popup tests cover both successful readiness ordering and a rejected ready event without unhandled promise rejections.
