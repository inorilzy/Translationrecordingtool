# 01 — Expand translation domain and persistence records

**What to build:** Introduce a translation result that represents translated content independently from stored history metadata, while keeping all existing manual translation, history, favorite, and detail behavior working. Add explicit mapping to the persisted record so later entry points can migrate without a flag day or user-data change.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] A core translation result exists without persistence identity, access-count, or favorite-state fields.
- [ ] A persisted translation record remains responsible for identifiers, access counts, favorite state, and the existing serialized record contract.
- [ ] Explicit mapping connects translation results, persisted records, and database rows without changing the database schema.
- [ ] Existing saved rows remain readable and require no migration.
- [ ] Saving, deduplication, history ordering, access counts, favorites, and detail lookup retain their current observable behavior.
- [ ] Existing translation entry points continue to work while the old and new forms coexist during the expansion phase.
- [ ] Isolated persistence tests cover mapping and round trips using temporary or in-memory storage rather than user data.
