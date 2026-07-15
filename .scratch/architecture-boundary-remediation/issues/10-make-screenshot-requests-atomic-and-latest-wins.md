# 10 — Make screenshot requests atomic and latest-wins

**What to build:** Make each screenshot translation a coherent request that uses one settings snapshot, backfills recognized text as soon as OCR succeeds, and prevents cancelled or older requests from overwriting the newest user-visible state.

**Blocked by:** None — can start immediately.

**Status:** completed

- [x] OCR and translation configuration are captured together when the screenshot request starts and remain stable for that request.
- [x] A settings change during OCR affects the next request, not the in-flight request.
- [x] Valid trimmed OCR text is backfilled before provider translation completes, including when provider translation later fails.
- [x] Empty OCR text fails explicitly without calling dictionary or provider collaborators.
- [x] Every asynchronous input, result, history, loading, and error update is scoped to the active screenshot request.
- [x] Cancelling a request invalidates its later OCR, translation, popup, and main-input updates.
- [x] Tests cover settings changes during OCR, provider failure after successful OCR, cancellation, and two requests completing out of order.
