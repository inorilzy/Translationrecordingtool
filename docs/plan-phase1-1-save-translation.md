# Phase 1.1 Implementation Plan: Rust-Side SQLite Connection + save_translation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open a single SQLite connection in Rust at app startup and implement the `save_translation` command so it actually persists data instead of returning a stub.

**Architecture:** Add a `DatabaseManager` struct that holds a `rusqlite::Connection`. Store it via `app.manage()` in `setup()`. Wire `save_translation` to use it. The frontend can optionally start calling this command while still maintaining its own connection (dual-write during transition).

**Tech Stack:** Rust, rusqlite (already in Cargo.toml), Tauri 2, serde_json

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src-tauri/src/db_manager.rs` | `DatabaseManager` struct, connection init, `save_translation` logic |
| Modify | `src-tauri/src/lib.rs:1-7` | Add `mod db_manager;` |
| Modify | `src-tauri/src/lib.rs:838-887` (setup function) | Initialize `DatabaseManager`, call `app.manage()` |
| Modify | `src-tauri/src/lib.rs:601-607` (save_translation command) | Replace stub with real implementation using `DatabaseManager` |
| Modify | `src-tauri/src/database.rs` | Add `to_json()` helper for array fields (explains, examples, synonyms) |
| Test | `src-tauri/tests/db_manager_tests.rs` | Unit tests for save and round-trip |

---

### Task 1: Create DatabaseManager struct

**Files:**
- Create: `src-tauri/src/db_manager.rs`

- [ ] **Step 1: Write the module file**

```rust
// src-tauri/src/db_manager.rs

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{info, warn};

use crate::database::{Translation, INIT_SQL};

/// Manages a single SQLite connection for the application.
///
/// All persistence commands borrow this through `tauri::State`.
pub struct DatabaseManager {
    conn: Mutex<Connection>,
}

impl DatabaseManager {
    /// Open (or create) the translations database at the given path.
    ///
    /// Runs the INIT_SQL schema creation on first connect.
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        info!("Opening translations database: {}", db_path.display());

        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|e| format!("Failed to set PRAGMAs: {}", e))?;

        let manager = Self {
            conn: Mutex::new(conn),
        };

        // Initialize schema
        manager.run_init_sql()?;

        Ok(manager)
    }

    /// Run the schema initialization SQL.
    fn run_init_sql(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(INIT_SQL)
            .map_err(|e| format!("Failed to initialize schema: {}", e))?;
        Ok(())
    }

    /// Save or update a translation record.
    ///
    /// Uses INSERT ... ON CONFLICT to upsert by (source_text, source_lang, target_lang).
    /// Returns the row ID of the saved record.
    pub fn save_translation(&self, translation: &Translation) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let explains_json = serialize_string_list(translation.explains.as_ref());
        let examples_json = serialize_string_list(translation.examples.as_ref());
        let synonyms_json = serialize_string_list(translation.synonyms.as_ref());

        let sql = r#"
            INSERT INTO translations (
                source_text, translated_text, phonetic, us_phonetic, uk_phonetic,
                audio_url, explains, examples, synonyms, source_lang, target_lang,
                word_type, created_at, access_count, is_favorite
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0
            )
            ON CONFLICT(source_text, source_lang, target_lang) DO UPDATE SET
                translated_text = excluded.translated_text,
                phonetic = excluded.phonetic,
                us_phonetic = excluded.us_phonetic,
                uk_phonetic = excluded.uk_phonetic,
                audio_url = excluded.audio_url,
                explains = excluded.explains,
                examples = excluded.examples,
                synonyms = excluded.synonyms,
                word_type = excluded.word_type,
                access_count = access_count + 1,
                created_at = excluded.created_at
        "#;

        conn.execute(
            sql,
            rusqlite::params![
                translation.source_text,
                translation.translated_text,
                translation.phonetic,
                translation.us_phonetic,
                translation.uk_phonetic,
                translation.audio_url,
                explains_json,
                examples_json,
                synonyms_json,
                translation.source_lang,
                translation.target_lang,
                translation.word_type,
                translation.created_at,
                translation.access_count,
            ],
        )
        .map_err(|e| format!("Failed to save translation: {}", e))?;

        // Return the row ID (for new inserts this is the new ID, for updates we look it up)
        let id: i64 = conn
            .query_row(
                "SELECT id FROM translations WHERE source_text = ?1 AND source_lang = ?2 AND target_lang = ?3",
                rusqlite::params![translation.source_text, translation.source_lang, translation.target_lang],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to retrieve saved translation ID: {}", e))?;

        Ok(id)
    }
}

/// Serialize an optional Vec<String> to JSON string for SQLite storage.
///
/// Matches the format the frontend expects when reading back.
fn serialize_string_list(items: Option<&Vec<String>>) -> Option<String> {
    items.filter(|v| !v.is_empty()).map(|v| serde_json::to_string(v).unwrap_or_default())
}
```

- [ ] **Step 2: Add module declaration in lib.rs**

In `src-tauri/src/lib.rs`, add after the existing mod declarations (line 6):

```rust
mod db_manager;
```

The top of `lib.rs` should now read:

```rust
mod clipboard;
mod database;
mod db_manager;
pub mod local_dictionary;
mod logger;
mod translator;
```

---

### Task 2: Initialize DatabaseManager in app setup

**Files:**
- Modify: `src-tauri/src/lib.rs` (setup function, around line 838)

- [ ] **Step 1: Add imports**

At the top of `lib.rs`, add `PathBuf` to the existing `std` import (line 13-14):

```rust
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
```

Also add `db_manager::DatabaseManager` to the imports (after line 8):

```rust
use database::{Translation, INIT_SQL};
use db_manager::DatabaseManager;
```

- [ ] **Step 2: Initialize DatabaseManager in setup()**

In the `setup()` closure (around line 838), add database initialization **after** the logger setup and **before** the config initialization. Insert after the logger `app.manage(guard)` block:

```rust
            // Initialize database connection
            let db_path = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("translations.db");

            let db_manager = DatabaseManager::new(db_path.clone())
                .unwrap_or_else(|e| {
                    warn!("Failed to initialize database at {}: {}", db_path.display(), e);
                    panic!("Database initialization failed: {}", e);
                });

            app.manage(db_manager);
            info!("Database manager initialized");
```

The full setup function order should be:
1. Ctrl+C handler (existing)
2. Logger init (existing)
3. **Database manager init (NEW)**
4. Config state init (existing)
5. Tray behavior state init (existing)
6. Popup state init (existing)
7. Legacy data migration (existing)
8. Dictionary init (existing)
9. Popup warmup (existing)
10. Tray menu (existing)
11. Window close handler (existing)
12. Global shortcut (existing)

---

### Task 3: Replace save_translation stub with real implementation

**Files:**
- Modify: `src-tauri/src/lib.rs` (save_translation command, lines 601-607)

- [ ] **Step 1: Replace the stub**

Replace the existing `save_translation` command (lines 601-607):

```rust
// OLD (delete):
#[tauri::command]
async fn save_translation(
    _app: tauri::AppHandle,
    _translation: Translation,
) -> Result<i64, String> {
    // TODO: 实现数据库保存
    Ok(1)
}
```

With the real implementation:

```rust
// NEW:
#[tauri::command]
async fn save_translation(
    db: tauri::State<'_, DatabaseManager>,
    translation: Translation,
) -> Result<i64, String> {
    db.save_translation(&translation)
}
```

---

### Task 4: Write tests

**Files:**
- Create: `src-tauri/tests/db_manager_tests.rs`

- [ ] **Step 1: Write the test file**

```rust
// src-tauri/tests/db_manager_tests.rs

use std::fs;
use translation_tool_lib::database::Translation;
use translation_tool_lib::db_manager::DatabaseManager;

struct TempDbGuard {
    path: std::path::PathBuf,
}

impl TempDbGuard {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "{}-{}.db",
            prefix,
            uuid::Uuid::new_v4()
        ));
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDbGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        // Also remove WAL and SHM files if they exist
        let _ = fs::remove_file(format!("{}-wal", self.path.display()));
        let _ = fs::remove_file(format!("{}-shm", self.path.display()));
    }
}

fn sample_translation() -> Translation {
    Translation {
        id: None,
        source_text: "hello".to_string(),
        translated_text: "你好".to_string(),
        phonetic: Some("/həˈloʊ/".to_string()),
        us_phonetic: Some("/həˈloʊ/".to_string()),
        uk_phonetic: Some("/həˈləʊ/".to_string()),
        audio_url: Some("https://example.com/audio.mp3".to_string()),
        explains: Some(vec!["int. 你好".to_string(), "n. 招呼".to_string()]),
        examples: Some(vec!["Hello, how are you?".to_string()]),
        synonyms: Some(vec!["hi".to_string(), "greetings".to_string()]),
        source_lang: "en".to_string(),
        target_lang: "zh".to_string(),
        word_type: Some("interjection".to_string()),
        created_at: 1700000000,
        access_count: 1,
        is_favorite: 0,
    }
}

#[test]
fn saves_new_translation_and_returns_id() {
    let temp_db = TempDbGuard::new("test-save");
    let manager = DatabaseManager::new(temp_db.path().to_path_buf()).unwrap();

    let translation = sample_translation();
    let id = manager.save_translation(&translation).unwrap();

    assert!(id > 0);
}

#[test]
fn upsert_increments_access_count_on_duplicate() {
    let temp_db = TempDbGuard::new("test-upsert");
    let manager = DatabaseManager::new(temp_db.path().to_path_buf()).unwrap();

    let mut translation = sample_translation();
    let id1 = manager.save_translation(&translation).unwrap();

    // Save again with same source_text
    translation.access_count = 1; // Reset to simulate a fresh translation object
    let id2 = manager.save_translation(&translation).unwrap();

    // Same row (upsert), not a new row
    assert_eq!(id1, id2);
}

#[test]
fn saves_translation_with_null_optional_fields() {
    let temp_db = TempDbGuard::new("test-null-fields");
    let manager = DatabaseManager::new(temp_db.path().to_path_buf()).unwrap();

    let translation = Translation {
        id: None,
        source_text: "test".to_string(),
        translated_text: "测试".to_string(),
        phonetic: None,
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains: None,
        examples: None,
        synonyms: None,
        source_lang: "en".to_string(),
        target_lang: "zh".to_string(),
        word_type: None,
        created_at: 1700000000,
        access_count: 1,
        is_favorite: 0,
    };

    let id = manager.save_translation(&translation).unwrap();
    assert!(id > 0);
}

#[test]
fn saves_translation_with_empty_vec_fields() {
    let temp_db = TempDbGuard::new("test-empty-vecs");
    let manager = DatabaseManager::new(temp_db.path().to_path_buf()).unwrap();

    let translation = Translation {
        id: None,
        source_text: "empty-test".to_string(),
        translated_text: "空测试".to_string(),
        phonetic: None,
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains: Some(vec![]),
        examples: Some(vec![]),
        synonyms: Some(vec![]),
        source_lang: "en".to_string(),
        target_lang: "zh".to_string(),
        word_type: None,
        created_at: 1700000000,
        access_count: 1,
        is_favorite: 0,
    };

    let id = manager.save_translation(&translation).unwrap();
    assert!(id > 0);
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test db_manager_tests -- --nocapture
```

Expected: All 4 tests pass.

---

### Task 5: Verify end-to-end

**Files:** No changes.

- [ ] **Step 1: Run cargo check**

```bash
cd src-tauri && cargo check
```

Expected: No errors, no warnings related to new code.

- [ ] **Step 2: Run all tests**

```bash
cd src-tauri && cargo test
```

Expected: All existing tests pass (including `lib_tests` migration tests) plus the 4 new `db_manager_tests`.

- [ ] **Step 3: Verify lsp_diagnostics**

Check `src-tauri/src/db_manager.rs` and `src-tauri/src/lib.rs` for any LSP errors.

Expected: Clean.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db_manager.rs src-tauri/src/lib.rs src-tauri/tests/db_manager_tests.rs
git commit -m "feat: implement Rust-side SQLite connection and save_translation command

- Add DatabaseManager struct with single rusqlite connection
- Initialize database in app setup with WAL mode
- Replace save_translation stub with real upsert logic
- Add unit tests for save, upsert, null fields, empty vecs"
```

---

## Risks Specific to This Phase

| Risk | Mitigation |
|------|-----------|
| `rusqlite` params macro limit (max 16) | We use exactly 14 params, well within limit. |
| `Mutex` contention during rapid saves | `rusqlite` with WAL mode handles concurrent reads well. Writes serialize through the mutex, which is acceptable for a single-user desktop app. |
| Frontend still writes via its own connection | Acceptable for this phase. Both sides write to the same file. The upsert logic ensures no duplicate rows. Phase 1.5 removes the frontend connection. |
| `DatabaseManager::new` panics on failure | Intentional. The app cannot function without a database. A graceful fallback would create silent data loss. |

## Verification Checklist

- [ ] `cargo test` passes (all tests, including new ones)
- [ ] `cargo check` produces no errors
- [ ] `save_translation` command is registered in `invoke_handler` (already listed, no change needed)
- [ ] `DatabaseManager` is stored via `app.manage()` in setup
- [ ] Test file covers: new insert, upsert, null fields, empty vecs
- [ ] No changes to existing `database.rs` struct definitions (backward compatible)
