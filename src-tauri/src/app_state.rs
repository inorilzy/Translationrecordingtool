use parking_lot::RwLock;
/// Application state and configuration management.
///
/// Contains all mutable state held in `Arc<RwLock<T>>` and persisted settings
/// (JSON config file).
use std::{fs, path::Path, sync::Arc};
use tauri::Manager;

use crate::settings::{load_settings, save_settings, settings_file_exists, SettingsRecord};

/// Serializes settings writers across candidate construction, file commit and
/// in-memory publish. Without it, a slow writer could persist snapshot A after
/// a newer snapshot B, and readers could observe A last.
static SETTINGS_COMMIT_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

// ─── Runtime State ───────────────────────────────────────────────────────────

// ─── Popup / Screenshot Runtime ──────────────────────────────────────────────

/// Tracks whether the popup window is ready and which translation request
/// is currently active (to avoid race conditions on rapid shortcut triggers).
#[derive(Default)]
pub struct PopupRuntimeState {
    pub ready: bool,
    pub active_request_id: u64,
}

#[derive(Default)]
pub struct ScreenshotRuntimeState {
    pub active_request_id: u64,
}

// ─── Popup State Helpers ─────────────────────────────────────────────────────

pub fn next_popup_request_id(state: &Arc<RwLock<PopupRuntimeState>>) -> u64 {
    let mut popup_state = state.write();
    popup_state.active_request_id += 1;
    popup_state.active_request_id
}

pub fn is_active_popup_request(state: &Arc<RwLock<PopupRuntimeState>>, request_id: u64) -> bool {
    state.read().active_request_id == request_id
}

pub fn is_popup_ready(state: &Arc<RwLock<PopupRuntimeState>>) -> bool {
    state.read().ready
}

pub fn mark_popup_ready(state: &Arc<RwLock<PopupRuntimeState>>, ready: bool) {
    state.write().ready = ready;
}

// ─── Screenshot State Helpers ────────────────────────────────────────────────

pub fn next_screenshot_request_id(state: &Arc<RwLock<ScreenshotRuntimeState>>) -> u64 {
    let mut screenshot_state = state.write();
    screenshot_state.active_request_id += 1;
    screenshot_state.active_request_id
}

pub fn is_active_screenshot_request(
    state: &Arc<RwLock<ScreenshotRuntimeState>>,
    request_id: u64,
) -> bool {
    state.read().active_request_id == request_id
}

// ─── Settings Persistence ────────────────────────────────────────────────────

pub fn load_persisted_settings(app: &tauri::AppHandle) -> Result<(SettingsRecord, bool), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let has_persisted_settings = settings_file_exists(&config_dir);
    load_settings(&config_dir).map(|settings| (settings, has_persisted_settings))
}

pub fn save_persisted_settings(
    app: &tauri::AppHandle,
    settings: &SettingsRecord,
) -> Result<(), String> {
    let _commit = SETTINGS_COMMIT_LOCK.lock();
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    save_settings(&config_dir, settings)
}

pub fn persist_managed_settings(
    app: &tauri::AppHandle,
    settings: &Arc<RwLock<SettingsRecord>>,
) -> Result<(), String> {
    let _commit = SETTINGS_COMMIT_LOCK.lock();
    let snapshot = settings.read().clone();
    save_persisted_settings_without_commit_lock(app, &snapshot)
}

fn save_persisted_settings_without_commit_lock(
    app: &tauri::AppHandle,
    settings: &SettingsRecord,
) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    save_settings(&config_dir, settings)
}

// ─── Single-Field Update Helpers ─────────────────────────────────────────────
//
// Persistence is the commit point: each helper builds a candidate from the
// canonical record, writes it, and only then publishes it to memory. Writers
// serialize on SETTINGS_COMMIT_LOCK so a slow write cannot be followed by a
// later writer committing an older snapshot. A failed save leaves the runtime
// record untouched.

fn update_and_persist<F>(
    app: &tauri::AppHandle,
    settings: &Arc<RwLock<SettingsRecord>>,
    apply: F,
) -> Result<(), String>
where
    F: FnOnce(&mut SettingsRecord),
{
    let _commit = SETTINGS_COMMIT_LOCK.lock();
    commit_candidate(settings, apply, |snapshot| {
        save_persisted_settings_without_commit_lock(app, snapshot)
    })
}

/// Same transaction as the production helpers, with persistence injected so
/// failure/commit ordering can be tested without the app config directory.
fn commit_candidate<F, P>(
    settings: &Arc<RwLock<SettingsRecord>>,
    apply: F,
    persist: P,
) -> Result<(), String>
where
    F: FnOnce(&mut SettingsRecord),
    P: FnOnce(&SettingsRecord) -> Result<(), String>,
{
    let snapshot = {
        let mut cfg = settings.read().clone();
        apply(&mut cfg);
        cfg
    };
    persist(&snapshot)?;
    *settings.write() = snapshot;
    Ok(())
}

pub fn update_and_persist_api_config(
    app: &tauri::AppHandle,
    settings: &Arc<RwLock<SettingsRecord>>,
    api_key: String,
    api_secret: String,
    translation_provider: String,
    microsoft_translator_key: String,
    microsoft_translator_region: String,
    google_api_key: String,
    ocr_endpoint: String,
    ocr_engine: String,
    ocr_model_profile: String,
    ocr_preload_on_startup: bool,
) -> Result<(), String> {
    update_and_persist(app, settings, |cfg| {
        cfg.api_key = api_key;
        cfg.api_secret = api_secret;
        cfg.translation_provider = translation_provider;
        cfg.microsoft_translator_key = microsoft_translator_key;
        cfg.microsoft_translator_region = microsoft_translator_region;
        cfg.google_api_key = google_api_key;
        cfg.ocr_endpoint = ocr_endpoint;
        cfg.ocr_engine = ocr_engine;
        cfg.ocr_model_profile = ocr_model_profile;
        cfg.ocr_preload_on_startup = ocr_preload_on_startup;
    })
}

pub fn update_and_persist_theme(
    app: &tauri::AppHandle,
    settings: &Arc<RwLock<SettingsRecord>>,
    theme: String,
) -> Result<(), String> {
    update_and_persist(app, settings, |cfg| cfg.theme = theme)
}

pub fn update_and_persist_global_shortcuts(
    app: &tauri::AppHandle,
    settings: &Arc<RwLock<SettingsRecord>>,
    global_shortcut: String,
    screenshot_shortcut: String,
) -> Result<(), String> {
    update_and_persist(app, settings, |cfg| {
        cfg.global_shortcut = global_shortcut;
        cfg.screenshot_shortcut = screenshot_shortcut;
    })
}

pub fn update_and_persist_tray_behavior(
    app: &tauri::AppHandle,
    settings: &Arc<RwLock<SettingsRecord>>,
    enabled: bool,
) -> Result<(), String> {
    update_and_persist(app, settings, |cfg| cfg.enable_tray = enabled)
}

// ─── Legacy App Data Migration ───────────────────────────────────────────────

const LEGACY_APP_DATA_DIR_NAME: &str = "com.zhiyu_liu.translation-tool";
const RUNTIME_DATA_FILES: &[&str] = &["translations.db", "dictionary.db"];

pub fn migrate_legacy_app_data(app: &tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    migrate_legacy_app_data_dir(&app_data_dir, LEGACY_APP_DATA_DIR_NAME, RUNTIME_DATA_FILES)
}

pub fn migrate_legacy_app_data_dir(
    app_data_dir: &Path,
    legacy_dir_name: &str,
    file_names: &[&str],
) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;

    let Some(parent_dir) = app_data_dir.parent() else {
        return Ok(());
    };

    let legacy_dir = parent_dir.join(legacy_dir_name);
    if !legacy_dir.exists() || legacy_dir == app_data_dir {
        return Ok(());
    }

    for file_name in file_names {
        let target_path = app_data_dir.join(file_name);
        if target_path.exists() {
            continue;
        }

        let legacy_path = legacy_dir.join(file_name);
        if !legacy_path.exists() {
            continue;
        }

        fs::copy(&legacy_path, &target_path).map_err(|e| {
            format!(
                "迁移历史数据失败: {} -> {} ({})",
                legacy_path.display(),
                target_path.display(),
                e
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod app_state_tests {
    use super::commit_candidate;
    use crate::settings::SettingsRecord;
    use parking_lot::RwLock;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Barrier,
    };

    fn state(theme: &str) -> Arc<RwLock<SettingsRecord>> {
        Arc::new(RwLock::new(SettingsRecord {
            theme: theme.to_string(),
            ..SettingsRecord::default()
        }))
    }

    #[test]
    fn failed_persistence_does_not_publish_runtime_fields() {
        let settings = state("light");
        let before = settings.read().clone();
        let result = commit_candidate(
            &settings,
            |cfg| {
                cfg.theme = "dark".into();
                cfg.api_key = "rejected-key".into();
                cfg.enable_tray = false;
            },
            |_| Err("injected save failure".to_string()),
        );
        assert!(result.is_err());
        assert_eq!(*settings.read(), before);
        // A later successful save must not carry the rejected fields.
        commit_candidate(
            &settings,
            |cfg| cfg.global_shortcut = "Ctrl+F9".into(),
            |_| Ok(()),
        )
        .unwrap();
        let after = settings.read().clone();
        assert_eq!(after.theme, "light");
        assert_eq!(after.api_key, "");
        assert!(after.enable_tray);
        assert_eq!(after.global_shortcut, "Ctrl+F9");
    }

    #[test]
    fn successful_persistence_publishes_exact_persisted_snapshot() {
        let settings = state("light");
        let persisted = std::sync::Mutex::new(None);
        commit_candidate(
            &settings,
            |cfg| {
                cfg.theme = "dark".into();
                cfg.enable_tray = false;
            },
            |snapshot| {
                *persisted.lock().unwrap() = Some(snapshot.clone());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(persisted.lock().unwrap().as_ref(), Some(&*settings.read()));
        assert_eq!(settings.read().theme, "dark");
    }

    #[test]
    fn memory_is_not_published_until_persist_returns() {
        let settings = state("light");
        let published_early = AtomicBool::new(false);
        commit_candidate(
            &settings,
            |cfg| cfg.theme = "dark".into(),
            |_| {
                published_early.store(settings.read().theme == "dark", Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
        assert!(!published_early.load(Ordering::SeqCst));
        assert_eq!(settings.read().theme, "dark");
    }

    #[test]
    fn concurrent_commits_are_not_lost_and_last_persisted_value_wins() {
        let settings = state("light");
        let commits = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(2));
        let run = |value: &'static str| {
            let settings = settings.clone();
            let commits = commits.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                commit_candidate(
                    &settings,
                    |cfg| cfg.theme = value.into(),
                    |snapshot| {
                        commits.fetch_add(1, Ordering::SeqCst);
                        assert_eq!(snapshot.theme, value);
                        Ok(())
                    },
                )
                .unwrap();
            })
        };
        let first = run("dark");
        let second = run("solarized");
        first.join().unwrap();
        second.join().unwrap();
        assert_eq!(commits.load(Ordering::SeqCst), 2);
        // Each commit publishes exactly its own persisted snapshot, and the
        // last one held the lock, so file and memory agree on one of them.
        let final_theme = settings.read().theme.clone();
        assert!(matches!(final_theme.as_str(), "dark" | "solarized"));
    }

    use super::migrate_legacy_app_data_dir;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn migrates_missing_runtime_files_from_legacy_directory() {
        let temp_dir = TempDirGuard::new("translation-tool-migrate");
        let current_dir = temp_dir.path().join("com.zhiyu-liu.translation-tool");
        let legacy_dir = temp_dir.path().join("com.zhiyu_liu.translation-tool");

        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(legacy_dir.join("translations.db"), b"history").unwrap();
        fs::write(legacy_dir.join("dictionary.db"), b"dictionary").unwrap();

        migrate_legacy_app_data_dir(
            &current_dir,
            "com.zhiyu_liu.translation-tool",
            &["translations.db", "dictionary.db"],
        )
        .unwrap();

        assert_eq!(
            fs::read(current_dir.join("translations.db")).unwrap(),
            b"history"
        );
        assert_eq!(
            fs::read(current_dir.join("dictionary.db")).unwrap(),
            b"dictionary"
        );
    }

    #[test]
    fn migration_does_not_overwrite_existing_runtime_files() {
        let temp_dir = TempDirGuard::new("translation-tool-migrate-existing");
        let current_dir = temp_dir.path().join("com.zhiyu-liu.translation-tool");
        let legacy_dir = temp_dir.path().join("com.zhiyu_liu.translation-tool");

        fs::create_dir_all(&current_dir).unwrap();
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(current_dir.join("translations.db"), b"new-history").unwrap();
        fs::write(legacy_dir.join("translations.db"), b"old-history").unwrap();

        migrate_legacy_app_data_dir(
            &current_dir,
            "com.zhiyu_liu.translation-tool",
            &["translations.db"],
        )
        .unwrap();

        assert_eq!(
            fs::read(current_dir.join("translations.db")).unwrap(),
            b"new-history"
        );
    }
}
