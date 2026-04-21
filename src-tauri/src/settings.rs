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
    fn load_returns_error_for_invalid_json() {
        let temp_dir = TempDirGuard::new("translation-tool-settings-invalid");
        fs::write(settings_file_path(temp_dir.path()), "{invalid json}").unwrap();

        let err = load_settings(temp_dir.path()).unwrap_err();

        assert!(err.contains("解析设置文件失败"));
    }
}
