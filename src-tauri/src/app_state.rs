/// Application state and configuration management.
///
/// Contains all mutable state held in `Arc<RwLock<T>>` and persisted settings
/// (JSON config file).
use std::{fs, path::Path, sync::Arc};
use parking_lot::RwLock;
use tauri::Manager;

use crate::settings::{
    load_settings, save_settings, PersistedSettings, DEFAULT_GLOBAL_SHORTCUT, DEFAULT_OCR_ENDPOINT,
    DEFAULT_OCR_ENGINE, DEFAULT_OCR_MODEL_PROFILE, DEFAULT_SCREENSHOT_SHORTCUT, DEFAULT_THEME,
};

// ─── Runtime State ───────────────────────────────────────────────────────────

/// Live application configuration (hot-reloadable via Tauri commands).
#[derive(Clone)]
pub struct AppConfig {
    pub api_key: String,
    pub api_secret: String,
    pub translation_provider: String,
    pub microsoft_translator_key: String,
    pub microsoft_translator_region: String,
    pub ocr_endpoint: String,
    pub ocr_engine: String,
    pub ocr_model_profile: String,
    pub ocr_preload_on_startup: bool,
    pub global_shortcut: String,
    pub screenshot_shortcut: String,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            translation_provider: "youdao".to_string(),
            microsoft_translator_key: String::new(),
            microsoft_translator_region: String::new(),
            ocr_endpoint: DEFAULT_OCR_ENDPOINT.to_string(),
            ocr_engine: DEFAULT_OCR_ENGINE.to_string(),
            ocr_model_profile: DEFAULT_OCR_MODEL_PROFILE.to_string(),
            ocr_preload_on_startup: true,
            global_shortcut: DEFAULT_GLOBAL_SHORTCUT.to_string(),
            screenshot_shortcut: DEFAULT_SCREENSHOT_SHORTCUT.to_string(),
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

// ─── Settings Persistence ────────────────────────────────────────────────────

pub fn to_persisted_settings(
    config: &AppConfig,
    tray_behavior: &TrayBehaviorConfig,
) -> PersistedSettings {
    PersistedSettings {
        api_key: config.api_key.clone(),
        api_secret: config.api_secret.clone(),
        translation_provider: config.translation_provider.clone(),
        microsoft_translator_key: config.microsoft_translator_key.clone(),
        microsoft_translator_region: config.microsoft_translator_region.clone(),
        ocr_endpoint: config.ocr_endpoint.clone(),
        ocr_engine: config.ocr_engine.clone(),
        ocr_model_profile: config.ocr_model_profile.clone(),
        ocr_preload_on_startup: config.ocr_preload_on_startup,
        global_shortcut: config.global_shortcut.clone(),
        screenshot_shortcut: config.screenshot_shortcut.clone(),
        enable_tray: tray_behavior.enabled,
        theme: config.theme.clone(),
    }
}

pub fn load_persisted_settings(app: &tauri::AppHandle) -> Result<PersistedSettings, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    load_settings(&config_dir)
}

pub fn save_persisted_settings(
    app: &tauri::AppHandle,
    settings: &PersistedSettings,
) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    save_settings(&config_dir, settings)
}

pub fn persist_managed_settings(
    app: &tauri::AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    tray_behavior: &Arc<RwLock<TrayBehaviorConfig>>,
) -> Result<(), String> {
    let config_snapshot = config.read().clone();
    let tray_snapshot = tray_behavior.read().clone();
    let settings = to_persisted_settings(&config_snapshot, &tray_snapshot);
    save_persisted_settings(app, &settings)
}

// ─── Single-Field Update Helpers ─────────────────────────────────────────────
//
// Each helper acquires a write lock on `AppConfig`, applies the update, then
// persists the combined (config + tray) snapshot to disk.  This removes the
// repetitive "lock → mutate → persist" boilerplate from individual commands.

pub fn update_and_persist_api_config(
    app: &tauri::AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    tray_behavior: &Arc<RwLock<TrayBehaviorConfig>>,
    api_key: String,
    api_secret: String,
    translation_provider: String,
    microsoft_translator_key: String,
    microsoft_translator_region: String,
    ocr_endpoint: String,
    _ocr_engine: String,
    _ocr_model_profile: String,
    ocr_preload_on_startup: bool,
) -> Result<(), String> {
    {
        let mut cfg = config.write();
        cfg.api_key = api_key;
        cfg.api_secret = api_secret;
        cfg.translation_provider = translation_provider;
        cfg.microsoft_translator_key = microsoft_translator_key;
        cfg.microsoft_translator_region = microsoft_translator_region;
        cfg.ocr_endpoint = ocr_endpoint;
        cfg.ocr_engine = DEFAULT_OCR_ENGINE.to_string();
        cfg.ocr_model_profile = DEFAULT_OCR_MODEL_PROFILE.to_string();
        cfg.ocr_preload_on_startup = ocr_preload_on_startup;
    }
    persist_managed_settings(app, config, tray_behavior)
}

pub fn update_and_persist_theme(
    app: &tauri::AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    tray_behavior: &Arc<RwLock<TrayBehaviorConfig>>,
    theme: String,
) -> Result<(), String> {
    {
        let mut cfg = config.write();
        cfg.theme = theme;
    }
    persist_managed_settings(app, config, tray_behavior)
}

pub fn update_and_persist_global_shortcuts(
    app: &tauri::AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    tray_behavior: &Arc<RwLock<TrayBehaviorConfig>>,
    global_shortcut: String,
    screenshot_shortcut: String,
) -> Result<(), String> {
    {
        let mut cfg = config.write();
        cfg.global_shortcut = global_shortcut;
        cfg.screenshot_shortcut = screenshot_shortcut;
    }
    persist_managed_settings(app, config, tray_behavior)
}

pub fn update_and_persist_tray_behavior(
    app: &tauri::AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    tray_behavior: &Arc<RwLock<TrayBehaviorConfig>>,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut tb = tray_behavior.write();
        tb.enabled = enabled;
    }
    persist_managed_settings(app, config, tray_behavior)
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
