// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod app_state;
mod clipboard;
mod database;
pub mod local_dictionary;
mod logger;
mod popup_window;
mod settings;
mod shortcut_handler;
mod translation_flow;
mod translator;

use app_state::{
    AppConfig, PopupRuntimeState, TrayBehaviorConfig, migrate_legacy_app_data,
    persist_managed_settings, load_persisted_settings,
};
use database::{
    get_translation_by_id_in_connection, load_favorites_in_connection, load_history_in_connection,
    open_translations_connection, save_translation_in_connection, toggle_favorite_in_connection,
    Translation,
};
use settings::{PersistedSettings, DEFAULT_GLOBAL_SHORTCUT};
use std::sync::{Arc, RwLock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tracing::{error, info, warn};

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn update_api_config(
    app: tauri::AppHandle,
    state: tauri::State<Arc<RwLock<AppConfig>>>,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    api_key: String,
    api_secret: String,
) -> Result<(), String> {
    {
        let mut config = state.write().unwrap();
        config.api_key = api_key;
        config.api_secret = api_secret;
    }
    persist_managed_settings(&app, state.inner(), tray_behavior.inner())
}

#[tauri::command]
fn update_global_shortcut(
    app: tauri::AppHandle,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    old_shortcut: String,
    new_shortcut: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    if let Ok(old) = old_shortcut.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(old);
    }

    let config = app.state::<Arc<RwLock<AppConfig>>>();
    let config_clone = config.inner().clone();
    let popup_state = app.state::<Arc<RwLock<PopupRuntimeState>>>();
    let popup_state_clone = popup_state.inner().clone();

    shortcut_handler::register_shortcut_handler(&app, &new_shortcut, config_clone, popup_state_clone)?;

    {
        let mut config_state = config.write().unwrap();
        config_state.global_shortcut = new_shortcut;
    }

    persist_managed_settings(&app, config.inner(), tray_behavior.inner())
}

#[tauri::command]
fn update_tray_behavior(
    app: tauri::AppHandle,
    app_config: tauri::State<Arc<RwLock<AppConfig>>>,
    state: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut config = state.write().unwrap();
        config.enabled = enabled;
    }
    persist_managed_settings(&app, app_config.inner(), state.inner())
}

#[tauri::command]
fn update_theme(
    app: tauri::AppHandle,
    state: tauri::State<Arc<RwLock<AppConfig>>>,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    theme: String,
) -> Result<(), String> {
    {
        let mut config = state.write().unwrap();
        config.theme = theme;
    }
    persist_managed_settings(&app, state.inner(), tray_behavior.inner())
}

#[tauri::command]
fn get_settings(
    state: tauri::State<Arc<RwLock<AppConfig>>>,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
) -> PersistedSettings {
    let config = state.read().unwrap();
    let tray_behavior = tray_behavior.read().unwrap();
    app_state::to_persisted_settings(&config, &tray_behavior)
}

#[tauri::command]
async fn translate_from_clipboard(
    app: tauri::AppHandle,
    app_key: String,
    app_secret: String,
) -> Result<Translation, String> {
    let text = translation_flow::read_current_clipboard_text(&app)?;
    translation_flow::resolve_translation(&app, &text, &app_key, &app_secret).await
}

#[tauri::command]
async fn translate_text(
    app: tauri::AppHandle,
    text: String,
    app_key: String,
    app_secret: String,
) -> Result<Translation, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("输入文本为空".to_string());
    }
    translation_flow::resolve_translation(&app, text, &app_key, &app_secret).await
}

#[tauri::command]
async fn save_translation(
    app: tauri::AppHandle,
    translation: Translation,
    increment_access_count: Option<bool>,
) -> Result<Translation, String> {
    let connection = open_translations_connection(&app)?;
    save_translation_in_connection(&connection, &translation, increment_access_count.unwrap_or(true))
}

#[tauri::command]
async fn toggle_favorite(
    app: tauri::AppHandle,
    id: i64,
    is_favorite: bool,
) -> Result<(), String> {
    let connection = open_translations_connection(&app)?;
    toggle_favorite_in_connection(&connection, id, is_favorite)
}

#[tauri::command]
async fn load_favorites(app: tauri::AppHandle) -> Result<Vec<Translation>, String> {
    let connection = open_translations_connection(&app)?;
    load_favorites_in_connection(&connection)
}

#[tauri::command]
async fn load_history(app: tauri::AppHandle) -> Result<Vec<Translation>, String> {
    let connection = open_translations_connection(&app)?;
    load_history_in_connection(&connection)
}

#[tauri::command]
async fn get_translation_by_id(app: tauri::AppHandle, id: i64) -> Result<Translation, String> {
    let connection = open_translations_connection(&app)?;
    get_translation_by_id_in_connection(&connection, id)
}

// ─── Application Entry ───────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Ctrl+C 信号处理
            let app_handle = app.handle().clone();
            ctrlc::set_handler(move || {
                info!("收到 Ctrl+C 信号，正在退出...");
                app_handle.exit(0);
            })
            .expect("设置 Ctrl+C 处理器失败");

            // 日志初始化
            let log_dir = app.path().app_log_dir().expect("无法获取日志目录");
            match logger::init_logger(log_dir) {
                Ok(guard) => {
                    app.manage(guard);
                    info!("应用启动");
                }
                Err(e) => {
                    eprintln!("初始化日志系统失败: {}", e);
                }
            }

            // 加载持久化设置
            let persisted_settings = match load_persisted_settings(&app.handle().clone()) {
                Ok(settings) => settings,
                Err(error) => {
                    warn!("加载持久化设置失败，使用默认值: {}", error);
                    PersistedSettings::default()
                }
            };

            // 初始化状态管理
            let config = Arc::new(RwLock::new(AppConfig {
                api_key: persisted_settings.api_key.clone(),
                api_secret: persisted_settings.api_secret.clone(),
                global_shortcut: persisted_settings.global_shortcut.clone(),
                theme: persisted_settings.theme.clone(),
            }));
            app.manage(config.clone());

            let tray_behavior = Arc::new(RwLock::new(TrayBehaviorConfig {
                enabled: persisted_settings.enable_tray,
            }));
            app.manage(tray_behavior.clone());

            let popup_state = Arc::new(RwLock::new(PopupRuntimeState::default()));
            app.manage(popup_state.clone());

            // 数据迁移
            if let Err(error) = migrate_legacy_app_data(&app.handle().clone()) {
                error!("迁移旧版应用数据失败: {}", error);
            }

            // 词典初始化
            match local_dictionary::ensure_runtime_dictionary(&app.handle().clone()) {
                Ok(Some(path)) => {
                    info!("本地词典已就绪: {}", path.display());
                }
                Ok(None) => {
                    warn!("未找到内置词典资源，单词查询将使用 Free Dictionary");
                }
                Err(error) => {
                    error!("初始化本地词典失败: {}", error);
                }
            }

            // 弹窗预热
            if let Err(error) = popup_window::ensure_popup_window(&app.handle().clone(), &popup_state) {
                error!("预热弹窗失败: {}", error);
            }

            // 托盘
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 主窗口关闭事件
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                let app_handle = app.handle().clone();
                let tray_behavior = tray_behavior.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if tray_behavior.read().unwrap().enabled {
                            let _ = window_clone.hide();
                            api.prevent_close();
                        } else {
                            app_handle.exit(0);
                        }
                    }
                });
            }

            // 注册快捷键
            let config_clone = config.clone();
            let popup_state_clone = popup_state.clone();
            let desired_shortcut = config.read().unwrap().global_shortcut.clone();

            if let Err(error) = shortcut_handler::register_shortcut_handler(
                &app.handle().clone(),
                &desired_shortcut,
                config_clone,
                popup_state_clone,
            ) {
                warn!("注册持久化快捷键失败，回退到默认快捷键: {}", error);
                shortcut_handler::register_shortcut_handler(
                    &app.handle().clone(),
                    DEFAULT_GLOBAL_SHORTCUT,
                    config.clone(),
                    popup_state.clone(),
                )
                .map_err(|e| e.to_string())?;

                {
                    let mut config_state = config.write().unwrap();
                    config_state.global_shortcut = DEFAULT_GLOBAL_SHORTCUT.to_string();
                }

                if let Err(error) = persist_managed_settings(&app.handle().clone(), &config, &tray_behavior) {
                    warn!("持久化默认快捷键失败: {}", error);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            translate_from_clipboard,
            translate_text,
            save_translation,
            toggle_favorite,
            load_favorites,
            load_history,
            get_translation_by_id,
            get_settings,
            update_api_config,
            update_global_shortcut,
            update_tray_behavior,
            update_theme,
            logger::get_log_files,
            logger::read_log_file,
            logger::get_log_dir_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
