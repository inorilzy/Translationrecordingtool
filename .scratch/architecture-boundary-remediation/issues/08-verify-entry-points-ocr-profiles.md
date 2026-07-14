# 08 — Verify every entry point and OCR build profile

**What to build:** Demonstrate that the completed architecture cutover preserves the full product across every translation entry point, stored-data workflow, rapid-request case, and documented OCR package while eliminating the audited critical dependency problems.

**Blocked by:** 02 — Break the OCR dependency cycle; 06 — Contract legacy translation paths; 07 — Consolidate popup event ownership.

**Status:** ready-for-agent

- [ ] Automated frontend and backend suites pass with the final command, workflow, domain, persistence, OCR, and popup contracts.
- [ ] Manual input and clipboard translation use current backend settings and produce persisted history records.
- [ ] Selected-text translation works through UI Automation and clipboard fallback, including immediate local results and later enrichment.
- [ ] Screenshot translation covers selection, native OCR, recognized-text backfill, translation, persistence, and anchored popup display.
- [ ] History loading, detail lookup, access counts, deduplication, and favorite toggling remain compatible with existing data.
- [ ] Rapid consecutive selected-text and screenshot requests cannot show stale results.
- [ ] Native, lite, and full OCR profiles resolve their resources and complete the appropriate startup or smoke path.
- [ ] The final dependency graph contains no OCR cycle and no translation-domain dependency on the database module.
- [ ] The final shortcut orchestration no longer owns dictionary or provider policy and has materially reduced fan-out.
- [ ] Architecture documentation describes the final single-workflow, backend-settings, domain/persistence, and OCR dependency direction.
- [ ] A follow-up Architecture Audit records the new health score and confirms the original two Critical findings are resolved.
