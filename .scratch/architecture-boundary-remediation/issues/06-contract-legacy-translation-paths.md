# 06 — Contract legacy translation paths

**What to build:** Complete the clean cutover so every production translation entry point uses one workflow, one backend settings source, and the separated domain/persistence model. Remove the expanded legacy forms only after all migrated paths are proven working.

**Blocked by:** 04 — Add staged selected-text shortcut workflow; 05 — Route screenshot OCR through the unified workflow.

**Status:** ready-for-agent

- [ ] Manual, clipboard, selected-text, and screenshot translation have no production caller outside the shared workflow.
- [ ] Legacy resolver variants and duplicated dictionary or provider policies are removed.
- [ ] Translation commands no longer accept frontend provider credentials, provider selection, or OCR runtime configuration.
- [ ] Core translation logic no longer depends on the persisted translation record or database-owned types.
- [ ] Temporary expanded types, adapters, wrappers, and compatibility command forms that are no longer used are deleted.
- [ ] No deprecated aliases, fallback implementations, or parallel settings paths remain.
- [ ] Existing database rows, serialized records, history, favorites, details, and settings remain backward compatible.
- [ ] Frontend and backend tests pass with only the final workflow and command contracts present.
- [ ] A dependency scan confirms the shortcut handler no longer directly depends on dictionary and provider implementations.
