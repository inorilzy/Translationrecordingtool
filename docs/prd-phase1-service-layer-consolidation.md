# PRD: Phase 1 Service-Layer Consolidation

> **Document type:** Product Requirements Document
> **Audience:** Solo developer (Liu Zhiyu)
> **Time horizon:** 3-6 month technical refactor direction
> **Status:** Draft

---

## 1. Problem Statement

The translation tool currently has **two parallel persistence layers** that operate independently and create data consistency risks:

| Layer | Technology | What it owns |
|-------|-----------|--------------|
| Frontend (Vue/Pinia) | `@tauri-apps/plugin-sql` + `localStorage` | History, favorites, detail queries, API keys, shortcuts, tray behavior, theme |
| Backend (Rust/Tauri) | In-memory `RwLock` state + stubbed commands | Runtime config copies (api_key, api_secret, shortcut, tray), popup window lifecycle |

### Concrete symptoms

1. **Settings split across three stores.** `apiKey`, `apiSecret`, `globalShortcut` live in `localStorage` AND get synced to Rust `RwLock<AppConfig>`. `enable_tray` lives in `localStorage` AND gets synced to `RwLock<TrayBehaviorConfig>`. Theme lives only in `localStorage`. If the sync step fails or runs out of order, Rust and frontend disagree on configuration.

2. **Frontend opens SQLite directly.** `src/lib/database.ts` calls `Database.load('sqlite:translations.db')` from the browser context. The popup window opens its own independent connection. There is no single authority over the database lifecycle.

3. **Rust persistence commands are stubs.** `save_translation`, `toggle_favorite`, `load_favorites`, `load_history`, `get_translation_by_id` all return dummy values. The frontend bypasses them entirely and runs raw SQL through the Tauri SQL plugin.

4. **No migration path for settings.** When a user changes a setting in the Settings page, the frontend writes to `localStorage`, then fires an `invoke` to sync Rust. If the app restarts, Rust starts from `Default` and waits for the frontend to re-sync. During that window, the shortcut handler uses stale config.

5. **Popup window duplicates persistence logic.** `PopupWindow.vue` opens its own database connection and runs `upsertTranslation` independently of the main window's store. Two connections writing to the same SQLite file without coordination.

### Why this matters now

The app works today because the solo developer controls both sides and the data volume is small. But every new feature that touches persistence (sync, backup, multi-window, search indexing) will compound the coupling. Moving persistence behind Rust commands now keeps the surface area small while the codebase is still manageable.

---

## 2. Goals

### Primary goal

Consolidate all user-data persistence (settings, history, favorites, translation details) behind Rust Tauri commands. The frontend becomes a thin client that calls commands and renders results.

### Secondary goals

- Eliminate `localStorage` as a source of truth for runtime settings.
- Remove direct `@tauri-apps/plugin-sql` usage from the frontend.
- Establish a single SQLite connection managed by Rust, shared across all commands.
- Keep the app fully functional throughout the migration. Each command replacement ships as an independent, testable unit.

### Non-goals (for this PRD)

- Cloud sync or multi-device support.
- User accounts or authentication.
- Database schema changes beyond what the existing code already requires.
- Performance optimization beyond removing redundant connections.

---

## 3. Scope

### In scope

| Area | Current state | Target state |
|------|--------------|--------------|
| Settings persistence | `localStorage` + Rust `RwLock` sync | Rust-managed SQLite config table + commands |
| History CRUD | Frontend raw SQL via `plugin-sql` | Rust commands: `load_history`, `save_translation` |
| Favorites CRUD | Frontend raw SQL via `plugin-sql` | Rust commands: `load_favorites`, `toggle_favorite` |
| Detail lookup | Frontend raw SQL via `plugin-sql` | Rust command: `get_translation_by_id` |
| Database lifecycle | Frontend opens connection per window | Rust opens one connection at startup |
| Popup persistence | Independent frontend connection | Rust command path (same as main window) |

### Out of scope

- Theme persistence (stays in `localStorage` for now, purely cosmetic).
- Autostart behavior (managed by Tauri plugin, no change needed).
- Offline dictionary database (`dictionary.db`, already read-only and Rust-owned).
- Youdao API key encryption (settings will be in SQLite plaintext, same as current `localStorage` behavior).

---

## 4. Architecture After Phase 1

```
Frontend (Vue 3 / Pinia)
  |
  |  invoke("save_translation", { ... })
  |  invoke("load_history")
  |  invoke("toggle_favorite", { id, is_favorite })
  |  invoke("load_favorites")
  |  invoke("get_translation_by_id", { id })
  |  invoke("load_settings")
  |  invoke("save_settings", { ... })
  |
  v
Rust (Tauri commands)
  |
  |  Single rusqlite Connection (opened at app startup)
  |  - translations table (existing schema)
  |  - settings table (new)
  |
  v
SQLite file (translations.db)
```

### Key design decisions

1. **Rust owns the connection.** `rusqlite` is already a dependency. The connection opens once in `setup()` and gets stored via `app.manage()`. All commands borrow it through `tauri::State`.

2. **Settings get their own table.** A simple key-value table avoids adding more `RwLock` structs. Rows like `('api_key', '...')`, `('global_shortcut', 'Ctrl+Q')`, `('enable_tray', 'true')`.

3. **Frontend store becomes a cache.** Pinia store holds in-memory copies for reactive UI, but the source of truth is always Rust. On mount, the store calls `load_settings` and `load_history`. On change, it calls the relevant save command.

4. **Popup window uses the same commands.** No independent database connection. The popup emits events to trigger translation, and the Rust side persists via the shared connection.

---

## 5. Success Criteria

| Criterion | How to verify |
|-----------|--------------|
| No `Database.load()` calls in frontend | Grep `src/` for `@tauri-apps/plugin-sql` imports. Zero results except type references. |
| No `localStorage` reads for settings | Grep `src/` for `localStorage.getItem` with keys `youdao_app_key`, `youdao_app_secret`, `global_shortcut`, `enable_tray`. Zero results. |
| All five stub commands return real data | Manual test: save a translation, reload app, verify it appears in history. |
| Settings survive app restart | Change shortcut, close app, reopen, verify shortcut persists. |
| `cargo test` passes | All existing tests pass, new tests for persistence commands pass. |
| `npm run tauri dev` runs without errors | No console errors, no Rust panics on startup. |

---

## 6. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| SQLite WAL mode conflicts with multiple readers | Data corruption or locked database | Use a single connection with `rusqlite`. No frontend-side connections. |
| Migration breaks existing user data | Lost history/favorites | The `translations.db` schema does not change. Existing data files remain compatible. Settings migration reads from `localStorage` on first run and writes to SQLite, then clears `localStorage`. |
| `rusqlite` serialization of `Vec<String>` differs from frontend JSON | Data format mismatch | Use the same JSON serialization approach the frontend already uses (`serde_json::to_string`). The `Translation` struct already derives `Serialize`/`Deserialize`. |
| Popup window timing issues during transition | Popup saves before Rust connection ready | The popup already waits for `popup-ready` event. Add a similar guard for database readiness, or ensure Rust connection opens before any window. |
| Scope creep into settings UI redesign | Delays core persistence work | Keep settings UI unchanged. Only the storage backend changes. |

---

## 7. Phased Rollout Plan

| Phase | Scope | Estimated effort | Dependency |
|-------|-------|-----------------|------------|
| **Phase 1.1** | Rust-side SQLite connection + `save_translation` command | 1-2 days | None |
| **Phase 1.2** | `load_history` + `load_favorites` commands | 1-2 days | Phase 1.1 |
| **Phase 1.3** | `toggle_favorite` + `get_translation_by_id` commands | 1 day | Phase 1.1 |
| **Phase 1.4** | Settings table + `load_settings` / `save_settings` commands | 2-3 days | Phase 1.1 |
| **Phase 1.5** | Frontend migration: replace all direct SQL with command calls | 2-3 days | Phases 1.1-1.4 |
| **Phase 1.6** | Remove `@tauri-apps/plugin-sql` from frontend, clean up `localStorage` | 1 day | Phase 1.5 |

Total estimated effort: **8-12 working days** for a solo developer. Spread across 3-6 months alongside feature work.

---

## 8. Future Considerations (Not in Phase 1)

These items inform design decisions but are not part of the current work:

- **Settings encryption.** If API keys need protection, a simple XOR or OS keychain integration can layer on top of the settings table.
- **Database migrations.** A versioned migration system becomes useful once the schema evolves beyond the current single table.
- **Full-text search.** SQLite FTS5 on the `translations` table would enable fast search without changing the command interface.
- **Backup/export.** A single SQLite file makes backup trivial. An export command could dump to JSON.
