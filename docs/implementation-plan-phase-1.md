# Phase 1 Service-Layer Consolidation Implementation Plan

## Goal

Make Rust the persisted runtime source of truth for application settings while keeping translation data access unchanged for this phase.

## Files Expected To Change

### Backend

- `src-tauri/src/settings.rs`
  - new settings persistence module
- `src-tauri/src/lib.rs`
  - load persisted settings on startup
  - expose `get_settings` and `update_theme`
  - persist settings in existing update commands

### Frontend

- `src/lib/settings.ts`
  - shared settings shape, defaults, legacy migration helpers
- `src/stores/translation.ts`
  - load settings from Rust and migrate legacy localStorage
- `src/views/SettingsPage.vue`
  - route all settings updates through store methods backed by Rust commands
- `src/views/PopupWindow.vue`
  - load initial theme from Rust settings
- `src/main.ts`
  - bootstrap theme from Rust settings before mounting app

### Documentation

- `docs/prd-phase-1-service-consolidation.md`
- `docs/implementation-plan-phase-1.md`

## Sequence

### Step 1 — Add Rust settings persistence module

- Add a serializable settings struct with defaults.
- Persist to `app_config_dir()/settings.json`.
- Add unit tests for:
  - default load when file is missing
  - save and load round-trip
  - malformed file error path

### Step 2 — Load settings during Tauri startup

- Read settings in `setup()`.
- Seed runtime state before tray and shortcut initialization.
- Register startup shortcut from persisted config.
- Fall back to the default shortcut if persisted data is invalid.

### Step 3 — Expose Rust commands for frontend settings bootstrap

- Add `get_settings`.
- Add `update_theme`.
- Update existing commands so they persist settings after mutation.

### Step 4 — Update frontend settings flow

- Add a shared settings helper module.
- Load settings from Rust in the store.
- Migrate legacy `localStorage` values only when Rust reports no persisted settings.
- Mirror settings back to localStorage during the transition phase to reduce rollback risk.

### Step 5 — Update theme bootstrap and popup theme loading

- Load initial theme in `main.ts` before app mount.
- Load initial popup theme from Rust on popup startup.
- Keep the existing theme-changed event path for live updates.

### Step 6 — Verification

- Rust unit tests for settings module
- frontend production build
- Rust compilation checks when environment permits
- manual checks:
  - change API key / secret and reload
  - change shortcut and restart
  - change tray behavior and restart
  - change theme and verify both main window and popup

## Guardrails

1. Do not move history/favorites/detail persistence in this phase.
2. Do not remove `@tauri-apps/plugin-sql` yet.
3. Do not refactor unrelated translation code.
4. Keep changes surgical and reversible.

## Risks To Watch During Implementation

- Global shortcut re-registration logic can regress if runtime state and persisted state diverge.
- Theme may flicker if frontend bootstrap applies defaults before backend settings are read.
- Legacy migration can overwrite backend settings if not gated by a persisted flag.

## Done Definition

- Docs are saved.
- Rust owns settings persistence.
- Frontend reads settings from Rust.
- Legacy migration exists.
- Verification results are recorded in the implementation summary.
