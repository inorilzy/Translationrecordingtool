# Translation Tool Phase 1 PRD

## 1. Objective

This document defines the first shippable refactor phase for the desktop translation tool. The goal of this phase is to move **settings persistence and runtime settings truth** behind the Rust/Tauri layer while preserving the current translation and history behavior.

This phase is intentionally conservative. It establishes a safer architecture boundary without attempting a broad rewrite.

## 2. Current Problem

The project currently has a split-brain settings model:

- API key / secret, shortcut, tray behavior, and theme are primarily stored in `localStorage`
- Rust holds only partial in-memory state for API config and tray behavior
- Global shortcut registration happens in Rust, but its source of truth is still frontend-managed
- On restart, runtime behavior depends on frontend sync rather than Rust-owned persistence

This creates three problems:

1. **State truth is fragmented** — frontend storage, Rust runtime state, and actual app behavior can drift.
2. **Startup behavior is fragile** — shortcut and tray behavior are not restored from a backend-owned source.
3. **Future refactors are blocked** — it is hard to move more business logic behind Rust while settings still originate in the frontend.

## 3. Phase 1 Scope

### In scope

- Persist application settings in Rust using a file-based settings store.
- Load persisted settings during Tauri startup.
- Make Rust the runtime source of truth for:
  - Youdao API key
  - Youdao API secret
  - Global shortcut
  - Tray behavior
  - Theme
- Add a `get_settings` command for frontend bootstrap.
- Add or update commands so frontend changes flow through Rust persistence.
- Keep `localStorage` only as a temporary migration fallback for one transition phase.
- Update frontend bootstrap, store, popup, and settings page to read from Rust-backed settings.

### Out of scope

- Moving translation/history/favorites/detail persistence behind Rust.
- Removing `@tauri-apps/plugin-sql` from translation data flows.
- Redesigning UI.
- Introducing cloud sync, accounts, or new product features.

## 4. Why This Is the Right Cut

This phase gives immediate value with low regression risk.

- Settings are small, structured, and low-frequency writes.
- Rust already owns part of the runtime behavior for shortcut and tray logic.
- Translation persistence is currently working in the frontend and is materially riskier to move in the same pass.

This creates a clean first architecture step:

> **Phase 1:** Rust owns settings truth.
>
> **Phase 2:** Rust owns translation persistence and query APIs.

## 5. User Value

After this phase:

- settings survive app restarts from a backend-owned source
- startup behavior is predictable
- shortcut registration reflects persisted settings immediately
- theme and tray behavior no longer rely on `localStorage` as the primary source

## 6. Functional Requirements

1. The app must persist settings to a Rust-managed file in the app config directory.
2. The app must load persisted settings during startup before registering the initial global shortcut.
3. The frontend must be able to fetch the current settings snapshot from Rust.
4. The existing update flows for API config, shortcut, and tray behavior must continue to work.
5. Theme selection must be persisted through Rust and restored on startup.
6. Existing user `localStorage` settings must be migratable without manual re-entry.

## 7. Non-Functional Requirements

- No new runtime dependencies beyond what is already in the repository.
- The change must preserve current user-facing behavior.
- Failure to read settings should fall back to safe defaults, not crash startup.
- Settings file writes must be atomic enough for normal desktop use and easy to debug.

## 8. Proposed Architecture

### New backend boundary

- Rust owns persisted settings through a dedicated `settings.rs` module.
- Frontend uses Tauri commands to read and update settings.
- `localStorage` remains only as a compatibility bridge during migration.

### Persistence format

- Use a JSON file in the app config directory.
- Rationale:
  - lower risk than introducing another SQLite workflow for settings
  - easy to inspect manually
  - appropriate for a small, stable settings object

## 9. Risks

1. **Migration drift**
   - Existing `localStorage` values could conflict with persisted backend values.
   - Mitigation: only migrate from legacy storage when Rust has no persisted settings yet.

2. **Shortcut startup regression**
   - If persisted shortcut format is invalid, startup registration could fail.
   - Mitigation: fall back to default shortcut and log the failure.

3. **Theme inconsistency across windows**
   - Main window and popup may render different themes if bootstrap order is wrong.
   - Mitigation: load settings in frontend bootstrap and keep the existing popup theme event path.

## 10. Acceptance Criteria

- A Rust settings module exists and is covered by unit tests.
- The Tauri app loads persisted settings during startup.
- `get_settings` returns a complete settings snapshot to the frontend.
- API config, shortcut, tray behavior, and theme changes are persisted through Rust.
- Frontend startup and settings UI no longer depend on `localStorage` as the primary source of truth.
- Legacy `localStorage` values can be migrated on first run after the refactor.

## 11. Follow-up After Phase 1

The next refactor phase should move translation persistence and query APIs behind Rust:

- history list
- favorites list
- detail lookup
- favorite toggling
- translation upsert / deduplication

That work should happen only after Phase 1 is stable and verified.
