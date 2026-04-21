/// Application state and configuration management.
///
/// Contains all mutable state held in `Arc<RwLock<T>>` and persisted settings
/// (JSON config file).
use std::{
    fs,
    path::Path,
    sync::{Arc, RwLock},
};
use tauri::Manager;

use crate::settings::{load_settings, save_settings, PersistedSettings, DEFAULT_GLOBAL_SHORTCUT, DEFAULT_THEME};

// ─── Runtime State ───────────────────────────────────────────────────────────

/// Live application configuration (hot-reloadable via Tauri commands).
#[derive(Clone)]
pub struct AppConfig {
    pub api_key: String,
    pub api_secret: String,
    pub global_shortcut: String,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            global_shortcut: DEFAULT_GLOBAL_SHORTCUT.to_string(),
            theme: DEFAULT_THEME.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct TrayBehaviorConfig {
    pub enabled: bool,
}

impl Default for TrayBehaviorConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Tracks whether the popup window is ready and which translation request
/// is currently active (to avoid race conditions on rapid shortcut triggers).
#[derive(Default)]
pub struct PopupRuntimeState {
    pub ready: bool,
    pub active_request_id: u64,
}

// ─── Popup State Helpers ─────────────────────────────────────────────────────

pub fn next_popup_request_id(state: &Arc<RwLock<PopupRuntimeState>>) -> u64 {
    let mut popup_state = state.write().unwrap();
    popup_state.active_request_id += 1;
    popup_state.active_request_id
}

pub fn is_active_popup_request(state: &Arc<RwLock<PopupRuntimeState>>, request_id: u64) -> bool {
    state.read().unwrap().active_request_id == request_id
}

pub fn is_popup_ready(state: &Arc<RwLock<PopupRuntimeState>>) -> bool {
    state.read().unwrap().ready
}

pub fn mark_popup_ready(state: &Arc<RwLock<PopupRuntimeState>>, ready: bool) {
    state.write().unwrap().ready = ready;
}

// ─── Settings Persistence ────────────────────────────────────────────────────

pub fn to_persisted_settings(config: &AppConfig, tray_behavior: &TrayBehaviorConfig) -> PersistedSettings {
    PersistedSettings {
        api_key: config.api_key.clone(),
        api_secret: config.api_secret.clone(),
        global_shortcut: config.global_shortcut.clone(),
        enable_tray: tray_behavior.enabled,
        theme: config.theme.clone(),
    }
}

pub fn load_persisted_settings(app: &tauri::AppHandle) -> Result<PersistedSettings, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    load_settings(&config_dir)
}

pub fn save_persisted_settings(app: &tauri::AppHandle, settings: &PersistedSettings) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    save_settings(&config_dir, settings)
}

pub fn persist_managed_settings(
    app: &tauri::AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    tray_behavior: &Arc<RwLock<TrayBehaviorConfig>>,
) -> Result<(), String> {
    let config_snapshot = config.read().unwrap().clone();
    let tray_snapshot = tray_behavior.read().unwrap().clone();
    let settings = to_persisted_settings(&config_snapshot, &tray_snapshot);
    save_persisted_settings(app, &settings)
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
