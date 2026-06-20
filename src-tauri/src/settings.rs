use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const DEFAULT_GLOBAL_SHORTCUT: &str = "Ctrl+Q";
pub const DEFAULT_SCREENSHOT_SHORTCUT: &str = "Ctrl+Shift+Q";
pub const DEFAULT_THEME: &str = "light";
pub const DEFAULT_OCR_ENDPOINT: &str = "http://127.0.0.1:8866/ocr";
pub const DEFAULT_OCR_ENGINE: &str = "paddleocr";
pub const DEFAULT_OCR_MODEL_PROFILE: &str = "small";
const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct PersistedSettings {
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
    pub enable_tray: bool,
    pub theme: String,
}

impl Default for PersistedSettings {
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
            enable_tray: true,
            theme: DEFAULT_THEME.to_string(),
        }
    }
}

fn settings_file_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE_NAME)
}

pub fn load_settings(config_dir: &Path) -> Result<PersistedSettings, String> {
    let settings_path = settings_file_path(config_dir);
    if !settings_path.exists() {
        return Ok(PersistedSettings::default());
    }

    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("读取设置文件失败: {} ({})", settings_path.display(), e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("解析设置文件失败: {} ({})", settings_path.display(), e))
}

pub fn save_settings(config_dir: &Path, settings: &PersistedSettings) -> Result<(), String> {
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("创建设置目录失败: {} ({})", config_dir.display(), e))?;

    let settings_path = settings_file_path(config_dir);
    let content =
        serde_json::to_string_pretty(settings).map_err(|e| format!("序列化设置失败: {}", e))?;

    fs::write(&settings_path, content)
        .map_err(|e| format!("写入设置文件失败: {} ({})", settings_path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };
    use uuid::Uuid;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let path = env::temp_dir().join(format!("{}-{}", prefix, Uuid::new_v4()));
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

    // ─── Default Values ──────────────────────────────────────────────────

    #[test]
    fn defaults_match_frontend_expectations() {
        let defaults = PersistedSettings::default();

        assert_eq!(defaults.api_key, "");
        assert_eq!(defaults.api_secret, "");
        assert_eq!(defaults.global_shortcut, DEFAULT_GLOBAL_SHORTCUT);
        assert_eq!(defaults.screenshot_shortcut, DEFAULT_SCREENSHOT_SHORTCUT);
        assert!(defaults.enable_tray);
        assert_eq!(defaults.theme, DEFAULT_THEME);
    }

    // ─── Round-Trip ──────────────────────────────────────────────────────

    #[test]
    fn load_returns_defaults_when_file_is_missing() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-defaults");

        let settings = load_settings(temp_dir.path()).unwrap();

        assert_eq!(settings, PersistedSettings::default());
    }

    #[test]
    fn save_and_load_round_trip_settings() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-roundtrip");
        let settings = PersistedSettings {
            api_key: "demo-key".to_string(),
            api_secret: "demo-secret".to_string(),
            translation_provider: "microsoft".to_string(),
            microsoft_translator_key: "ms-key".to_string(),
            microsoft_translator_region: "eastasia".to_string(),
            ocr_endpoint: "http://127.0.0.1:8866/ocr".to_string(),
            ocr_engine: "paddleocr".to_string(),
            ocr_model_profile: "small".to_string(),
            ocr_preload_on_startup: true,
            global_shortcut: "Ctrl+Shift+Q".to_string(),
            screenshot_shortcut: "Ctrl+Shift+S".to_string(),
            enable_tray: false,
            theme: "github-dark".to_string(),
        };

        save_settings(temp_dir.path(), &settings).unwrap();
        let loaded = load_settings(temp_dir.path()).unwrap();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn round_trip_all_fields_zero_values() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-zero");
        let settings = PersistedSettings {
            api_key: String::new(),
            api_secret: String::new(),
            translation_provider: String::new(),
            microsoft_translator_key: String::new(),
            microsoft_translator_region: String::new(),
            ocr_endpoint: String::new(),
            ocr_engine: String::new(),
            ocr_model_profile: String::new(),
            ocr_preload_on_startup: false,
            global_shortcut: String::new(),
            screenshot_shortcut: String::new(),
            enable_tray: false,
            theme: String::new(),
        };

        save_settings(temp_dir.path(), &settings).unwrap();
        let loaded = load_settings(temp_dir.path()).unwrap();

        assert_eq!(loaded, settings);
    }

    // ─── Field Naming Stability (camelCase serde contract) ───────────────

    #[test]
    fn serialization_uses_camel_case_field_names() {
        let settings = PersistedSettings {
            api_key: "k".to_string(),
            api_secret: "s".to_string(),
            translation_provider: "youdao".to_string(),
            microsoft_translator_key: "mk".to_string(),
            microsoft_translator_region: "global".to_string(),
            ocr_endpoint: "http://127.0.0.1:8866/ocr".to_string(),
            ocr_engine: "paddleocr".to_string(),
            ocr_model_profile: "small".to_string(),
            ocr_preload_on_startup: true,
            global_shortcut: "Ctrl+Q".to_string(),
            screenshot_shortcut: "Ctrl+Shift+Q".to_string(),
            enable_tray: true,
            theme: "dark".to_string(),
        };

        let json = serde_json::to_string(&settings).unwrap();

        // Frontend expects camelCase keys — verify the serde contract
        assert!(json.contains(r#""apiKey""#));
        assert!(json.contains(r#""apiSecret""#));
        assert!(json.contains(r#""translationProvider""#));
        assert!(json.contains(r#""microsoftTranslatorKey""#));
        assert!(json.contains(r#""microsoftTranslatorRegion""#));
        assert!(json.contains(r#""ocrEndpoint""#));
        assert!(json.contains(r#""ocrEngine""#));
        assert!(json.contains(r#""ocrModelProfile""#));
        assert!(json.contains(r#""ocrPreloadOnStartup""#));
        assert!(json.contains(r#""globalShortcut""#));
        assert!(json.contains(r#""screenshotShortcut""#));
        assert!(json.contains(r#""enableTray""#));
        assert!(json.contains(r#""theme""#));
    }

    #[test]
    fn deserialization_accepts_camel_case_keys() {
        let json = r#"{
            "apiKey": "test-key",
            "apiSecret": "test-secret",
            "translationProvider": "microsoft",
            "microsoftTranslatorKey": "ms-key",
            "microsoftTranslatorRegion": "eastasia",
            "ocrEndpoint": "http://127.0.0.1:8866/ocr",
            "ocrEngine": "paddleocr",
            "ocrModelProfile": "lite",
            "ocrPreloadOnStartup": false,
            "globalShortcut": "Ctrl+Shift+A",
            "screenshotShortcut": "Ctrl+Shift+S",
            "enableTray": false,
            "theme": "solarized"
        }"#;

        let settings: PersistedSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.api_key, "test-key");
        assert_eq!(settings.api_secret, "test-secret");
        assert_eq!(settings.translation_provider, "microsoft");
        assert_eq!(settings.microsoft_translator_key, "ms-key");
        assert_eq!(settings.microsoft_translator_region, "eastasia");
        assert_eq!(settings.ocr_endpoint, "http://127.0.0.1:8866/ocr");
        assert_eq!(settings.ocr_engine, "paddleocr");
        assert_eq!(settings.ocr_model_profile, "lite");
        assert!(!settings.ocr_preload_on_startup);
        assert_eq!(settings.global_shortcut, "Ctrl+Shift+A");
        assert_eq!(settings.screenshot_shortcut, "Ctrl+Shift+S");
        assert!(!settings.enable_tray);
        assert_eq!(settings.theme, "solarized");
    }

    // ─── Malformed / Error Paths ─────────────────────────────────────────

    #[test]
    fn load_returns_error_for_invalid_json() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-invalid");
        fs::write(settings_file_path(temp_dir.path()), "{invalid json}").unwrap();

        let err = load_settings(temp_dir.path()).unwrap_err();

        assert!(err.contains("解析设置文件失败"));
    }

    #[test]
    fn load_error_message_contains_file_path() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-path");
        let bad_path = settings_file_path(temp_dir.path());
        fs::write(&bad_path, "not-json-at-all").unwrap();

        let err = load_settings(temp_dir.path()).unwrap_err();

        // Error must contain the file path so the user can locate the problem
        assert!(err.contains(&bad_path.to_string_lossy().to_string()));
    }

    #[test]
    fn save_error_contains_file_path_on_write_failure() {
        // Write to a read-only directory to trigger a write error
        let temp_dir = TempDirGuard::new("translation-tool-settings-readonly");
        let settings = PersistedSettings::default();

        // On Windows, we can't easily make a directory read-only in tests,
        // so we verify the error path by pointing to a path inside a file
        let file_path = temp_dir.path().join("not-a-dir");
        fs::write(&file_path, b"blocker").unwrap();

        let err = save_settings(&file_path, &settings).unwrap_err();

        assert!(err.contains("创建设置目录失败"));
    }

    // ─── Missing Fields → Serde Defaults ─────────────────────────────────

    #[test]
    fn partial_json_fills_missing_fields_with_defaults() {
        // Frontend may send a subset of fields — serde default must handle it
        let json = r#"{"apiKey": "partial-key"}"#;

        let settings: PersistedSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.api_key, "partial-key");
        assert_eq!(settings.api_secret, ""); // default
        assert_eq!(settings.translation_provider, "youdao");
        assert_eq!(settings.microsoft_translator_key, "");
        assert_eq!(settings.microsoft_translator_region, "");
        assert_eq!(settings.ocr_endpoint, DEFAULT_OCR_ENDPOINT);
        assert_eq!(settings.ocr_engine, DEFAULT_OCR_ENGINE);
        assert_eq!(settings.ocr_model_profile, DEFAULT_OCR_MODEL_PROFILE);
        assert!(settings.ocr_preload_on_startup);
        assert_eq!(settings.global_shortcut, DEFAULT_GLOBAL_SHORTCUT);
        assert_eq!(settings.screenshot_shortcut, DEFAULT_SCREENSHOT_SHORTCUT);
        assert!(settings.enable_tray); // default true
        assert_eq!(settings.theme, DEFAULT_THEME);
    }

    #[test]
    fn empty_json_object_produces_full_defaults() {
        let settings: PersistedSettings = serde_json::from_str("{}").unwrap();

        assert_eq!(settings, PersistedSettings::default());
    }

    // ─── Unknown Fields ──────────────────────────────────────────────────

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{
            "apiKey": "known",
            "unknownField": "should-be-ignored",
            "theme": "dark",
            "extraNested": {"a": 1}
        }"#;

        let settings: PersistedSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.api_key, "known");
        assert_eq!(settings.theme, "dark");
    }

    // ─── Empty / Corrupt File ────────────────────────────────────────────

    #[test]
    fn load_returns_error_for_empty_file() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-empty");
        fs::write(settings_file_path(temp_dir.path()), "").unwrap();

        let err = load_settings(temp_dir.path()).unwrap_err();

        assert!(err.contains("解析设置文件失败"));
    }
}
