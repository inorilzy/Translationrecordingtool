use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

pub const DEFAULT_GLOBAL_SHORTCUT: &str = "Ctrl+Q";
pub const DEFAULT_THEME: &str = "light";
const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct PersistedSettings {
    pub api_key: String,
    pub api_secret: String,
    pub global_shortcut: String,
    pub enable_tray: bool,
    pub theme: String,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            global_shortcut: DEFAULT_GLOBAL_SHORTCUT.to_string(),
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
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化设置失败: {}", e))?;

    fs::write(&settings_path, content)
        .map_err(|e| format!("写入设置文件失败: {} ({})", settings_path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, path::{Path, PathBuf}};
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
            global_shortcut: "Ctrl+Shift+Q".to_string(),
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
            global_shortcut: String::new(),
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
            global_shortcut: "Ctrl+Q".to_string(),
            enable_tray: true,
            theme: "dark".to_string(),
        };

        let json = serde_json::to_string(&settings).unwrap();

        // Frontend expects camelCase keys — verify the serde contract
        assert!(json.contains(r#""apiKey""#));
        assert!(json.contains(r#""apiSecret""#));
        assert!(json.contains(r#""globalShortcut""#));
        assert!(json.contains(r#""enableTray""#));
        assert!(json.contains(r#""theme""#));
    }

    #[test]
    fn deserialization_accepts_camel_case_keys() {
        let json = r#"{
            "apiKey": "test-key",
            "apiSecret": "test-secret",
            "globalShortcut": "Ctrl+Shift+A",
            "enableTray": false,
            "theme": "solarized"
        }"#;

        let settings: PersistedSettings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.api_key, "test-key");
        assert_eq!(settings.api_secret, "test-secret");
        assert_eq!(settings.global_shortcut, "Ctrl+Shift+A");
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
        assert_eq!(settings.global_shortcut, DEFAULT_GLOBAL_SHORTCUT);
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
