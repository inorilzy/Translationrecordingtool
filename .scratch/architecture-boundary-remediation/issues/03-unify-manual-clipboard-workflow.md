# 03 — Unify manual and clipboard translation workflow

**What to build:** Make manual input and clipboard translation use one backend-owned workflow and one runtime settings source. Both entry points must preserve local dictionary priority, enrichment, provider fallback, persistence, and frontend state while no longer sending provider credentials with each request.

**Blocked by:** 01 — Expand translation domain and persistence records.

**Status:** ready-for-agent

- [ ] A backend application workflow owns dictionary lookup, enrichment, configured-provider selection, fallback, and translation result construction.
- [ ] The workflow exposes one high-level testing seam with narrow dictionary and translation-provider collaborators.
- [ ] Manual input and clipboard translation both delegate to the workflow.
- [ ] Provider selection and credentials are read from backend-managed runtime settings rather than frontend command payloads.
- [ ] Manual and clipboard command requests contain only interaction-specific user input.
- [ ] Single-word local lookup, Free Dictionary enrichment, sentence translation, remote fallback, and error precedence remain compatible.
- [ ] Successful results are mapped and persisted through the record boundary before frontend history state is updated.
- [ ] Existing frontend loading, error, current-result, and history behavior remains visible to the user.
- [ ] Deterministic workflow tests cover local hits, enrichment, misses, provider selection, provider failure, empty input, and configuration changes.
