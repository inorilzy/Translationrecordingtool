# 04 — Add staged selected-text shortcut workflow

**What to build:** Route selected-text shortcut translation through the shared workflow while retaining the fast popup experience: show a local result immediately, enrich it asynchronously when possible, fall back to the configured provider, and prevent stale requests from replacing current results.

**Blocked by:** 03 — Unify manual and clipboard translation workflow.

**Status:** ready-for-agent

- [ ] The selected-text handler is limited to shortcut handling, text acquisition, request identity, and popup presentation.
- [ ] Windows UI Automation remains the preferred selected-text source and clipboard copy/restore remains the fallback.
- [ ] The shared workflow reports stages for local result, enrichment, remote progress, completion, cancellation, and failure.
- [ ] A local dictionary hit can be displayed before remote enrichment completes.
- [ ] Enrichment updates the active popup without creating a second result or incrementing access count twice.
- [ ] Local misses and provider fallback use the same policy as manual and clipboard translation.
- [ ] Older requests cannot display or update after a newer request becomes active.
- [ ] The selected-text handler no longer calls dictionary, enrichment, merge, or translation-provider implementations directly.
- [ ] Tests cover UI Automation success, clipboard fallback, immediate local results, enrichment, remote fallback, failure, and rapid consecutive requests.
