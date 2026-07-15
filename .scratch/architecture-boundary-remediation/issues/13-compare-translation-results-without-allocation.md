# 13 — Compare translation results without allocation

**What to build:** Compare persisted records with workflow results through borrowed data so completing or enriching a translation does not clone the full record merely to decide whether content changed.

**Blocked by:** None — can start immediately.

**Status:** completed

- [x] Result comparison performs no avoidable cloning or allocation of translated text, examples, phonetics, or synonyms.
- [x] Comparison uses translation-content meaning and does not accidentally include persistence identity, access count, or favorite state.
- [x] Equal local and completed results continue to avoid duplicate persistence or access-count updates.
- [x] Changed enrichment or provider content is still detected and persisted exactly once.
- [x] Focused tests cover equal content, changed content, and differing persistence metadata.
