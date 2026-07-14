// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod app_state;
mod clipboard;
mod database;
pub mod local_dictionary;
mod logger;
mod ocr_contracts;
mod native_ocr;
mod ocr;
mod ocr_service;
pub mod popup_window;
mod screenshot;
mod selection_reader;
mod settings;
mod shortcut_handler;
mod translation_domain;
mod translation_flow;
mod translation_workflow;
mod translator;

use app_state::{
    load_persisted_settings, migrate_legacy_app_data, persist_managed_settings,
    save_persisted_settings, update_and_persist_api_config, update_and_persist_global_shortcuts,
    update_and_persist_theme, update_and_persist_tray_behavior, AppConfig, PopupRuntimeState,
    TrayBehaviorConfig,
};
use database::{
    get_translation_by_id_in_connection, load_favorites_in_connection, load_history_in_connection,
    open_translations_connection, toggle_favorite_in_connection, TranslationRecord,
};
use ocr_contracts::{OcrRuntimeConfig, OcrServiceStatus};
use translation_workflow::AppTranslationWorkflow;
use settings::{
    PersistedSettings, DEFAULT_GLOBAL_SHORTCUT, DEFAULT_OCR_MODEL_PROFILE,
    DEFAULT_SCREENSHOT_SHORTCUT,
};
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
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
    translation_provider: String,
    microsoft_translator_key: String,
    microsoft_translator_region: String,
    ocr_endpoint: String,
    ocr_engine: String,
    ocr_model_profile: String,
    ocr_preload_on_startup: bool,
) -> Result<(), String> {
    update_and_persist_api_config(
        &app,
        state.inner(),
        tray_behavior.inner(),
        api_key,
        api_secret,
        translation_provider,
        microsoft_translator_key,
        microsoft_translator_region,
        ocr_endpoint,
        ocr_engine,
        ocr_model_profile,
        ocr_preload_on_startup,
    )
}

fn ocr_runtime_config_from_state(config: &Arc<RwLock<AppConfig>>) -> OcrRuntimeConfig {
    let config = config.read();
    OcrRuntimeConfig {
        endpoint: config.ocr_endpoint.clone(),
        engine: config.ocr_engine.clone(),
        model_profile: config.ocr_model_profile.clone(),
        preload_on_startup: config.ocr_preload_on_startup,
    }
}

fn adapt_ocr_settings_for_packaged_runtime(
    app: &tauri::AppHandle,
    settings: &mut PersistedSettings,
) -> bool {
    let (native_profile, native_model_dir) =
        native_ocr::model_status(app, DEFAULT_OCR_MODEL_PROFILE);
    if native_model_dir.is_none() {
        return false;
    }

    let changed = settings.ocr_engine != native_ocr::engine_name()
        || settings.ocr_model_profile != native_profile;
    settings.ocr_engine = native_ocr::engine_name().to_string();
    settings.ocr_model_profile = native_profile;
    changed
}


#[tauri::command]
fn update_global_shortcut(
    app: tauri::AppHandle,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    old_shortcut: String,
    new_shortcut: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    let config = app.state::<Arc<RwLock<AppConfig>>>();
    let screenshot_shortcut = config.read().screenshot_shortcut.clone();
    if new_shortcut == screenshot_shortcut {
        return Err("两个快捷键不能相同".to_string());
    }

    let previous_shortcut = if old_shortcut.trim().is_empty() {
        config.read().global_shortcut.clone()
    } else {
        old_shortcut
    };

    if let Ok(old) = previous_shortcut.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(old);
    }

    let popup_state = app.state::<Arc<RwLock<PopupRuntimeState>>>();
    let popup_state_clone = popup_state.inner().clone();

    if let Err(error) = shortcut_handler::register_shortcut_handler(
        &app,
        &new_shortcut,
        popup_state_clone.clone(),
    ) {
        return restore_global_shortcut(
            &app,
            &previous_shortcut,
            popup_state_clone,
            format!("注册新快捷键失败: {}", error),
        );
    }

    {
        let mut config_state = config.write();
        config_state.global_shortcut = new_shortcut.clone();
    }

    if let Err(error) = persist_managed_settings(&app, config.inner(), tray_behavior.inner()) {
        if let Ok(new) = new_shortcut.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(new);
        }
        {
            let mut config_state = config.write();
            config_state.global_shortcut = previous_shortcut.clone();
        }
        return restore_global_shortcut(
            &app,
            &previous_shortcut,
            popup_state_clone,
            format!("保存快捷键失败: {}", error),
        );
    }

    Ok(())
}

#[tauri::command]
fn update_screenshot_shortcut(
    app: tauri::AppHandle,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    old_shortcut: String,
    new_shortcut: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    let config = app.state::<Arc<RwLock<AppConfig>>>();
    let global_shortcut = config.read().global_shortcut.clone();
    if new_shortcut == global_shortcut {
        return Err("两个快捷键不能相同".to_string());
    }

    let previous_shortcut = if old_shortcut.trim().is_empty() {
        config.read().screenshot_shortcut.clone()
    } else {
        old_shortcut
    };

    if let Ok(old) = previous_shortcut.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(old);
    }

    let popup_state = app.state::<Arc<RwLock<PopupRuntimeState>>>();
    let popup_state_clone = popup_state.inner().clone();

    if let Err(error) = shortcut_handler::register_screenshot_shortcut_handler(
        &app,
        &new_shortcut,
        popup_state_clone.clone(),
    ) {
        return restore_screenshot_shortcut(
            &app,
            &previous_shortcut,
            popup_state_clone,
            format!("注册新截图快捷键失败: {}", error),
        );
    }

    {
        let mut config_state = config.write();
        config_state.screenshot_shortcut = new_shortcut.clone();
    }

    if let Err(error) = persist_managed_settings(&app, config.inner(), tray_behavior.inner()) {
        if let Ok(new) = new_shortcut.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(new);
        }
        {
            let mut config_state = config.write();
            config_state.screenshot_shortcut = previous_shortcut.clone();
        }
        return restore_screenshot_shortcut(
            &app,
            &previous_shortcut,
            popup_state_clone,
            format!("保存截图快捷键失败: {}", error),
        );
    }

    Ok(())
}

fn restore_global_shortcut(
    app: &tauri::AppHandle,
    previous_shortcut: &str,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
    original_error: String,
) -> Result<(), String> {
    match shortcut_handler::register_shortcut_handler(app, previous_shortcut, popup_state) {
        Ok(()) => Err(format!(
            "{}，已恢复旧快捷键 {}",
            original_error, previous_shortcut
        )),
        Err(restore_error) => Err(format!(
            "{}，且恢复旧快捷键 {} 失败: {}",
            original_error, previous_shortcut, restore_error
        )),
    }
}

fn restore_screenshot_shortcut(
    app: &tauri::AppHandle,
    previous_shortcut: &str,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
    original_error: String,
) -> Result<(), String> {
    match shortcut_handler::register_screenshot_shortcut_handler(
        app,
        previous_shortcut,
        popup_state,
    ) {
        Ok(()) => Err(format!(
            "{}，已恢复旧截图快捷键 {}",
            original_error, previous_shortcut
        )),
        Err(restore_error) => Err(format!(
            "{}，且恢复旧截图快捷键 {} 失败: {}",
            original_error, previous_shortcut, restore_error
        )),
    }
}

#[tauri::command]
fn update_tray_behavior(
    app: tauri::AppHandle,
    app_config: tauri::State<Arc<RwLock<AppConfig>>>,
    state: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    enabled: bool,
) -> Result<(), String> {
    update_and_persist_tray_behavior(&app, app_config.inner(), state.inner(), enabled)
}

#[tauri::command]
fn update_theme(
    app: tauri::AppHandle,
    state: tauri::State<Arc<RwLock<AppConfig>>>,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    theme: String,
) -> Result<(), String> {
    update_and_persist_theme(&app, state.inner(), tray_behavior.inner(), theme)
}

#[tauri::command]
fn get_settings(
    state: tauri::State<Arc<RwLock<AppConfig>>>,
    tray_behavior: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
) -> PersistedSettings {
    let config = state.read();
    let tray_behavior = tray_behavior.read();
    app_state::to_persisted_settings(&config, &tray_behavior)
}

#[tauri::command]
async fn translate_from_clipboard(
    app: tauri::AppHandle,
    workflow: tauri::State<'_, AppTranslationWorkflow>,
) -> Result<TranslationRecord, String> {
    let text = clipboard::read_clipboard(&app)?;
    if text.trim().is_empty() {
        return Err("剪贴板为空".to_string());
    }

    workflow
        .translate_text(text.trim(), &mut |_| {}, &|| false)
        .await
}

#[tauri::command]
async fn translate_text(
    workflow: tauri::State<'_, AppTranslationWorkflow>,
    text: String,
) -> Result<TranslationRecord, String> {
    workflow
        .translate_text(&text, &mut |_| {}, &|| false)
        .await
}

#[tauri::command]
async fn translate_image(
    workflow: tauri::State<'_, AppTranslationWorkflow>,
    image_base64: String,
) -> Result<TranslationRecord, String> {
    workflow
        .translate_image(&image_base64, &mut |_| {}, &|| false)
        .await
}

#[tauri::command]
async fn check_ocr_service(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<RwLock<AppConfig>>>,
) -> Result<String, String> {
    let config = ocr_runtime_config_from_state(state.inner());
    ocr::ensure_running(&app, &config).await
}

#[tauri::command]
async fn get_ocr_service_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<RwLock<AppConfig>>>,
) -> Result<OcrServiceStatus, String> {
    let config = ocr_runtime_config_from_state(state.inner());
    Ok(ocr::status(&app, &config).await)
}

#[tauri::command]
async fn warmup_ocr_service(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<RwLock<AppConfig>>>,
) -> Result<String, String> {
    let config = ocr_runtime_config_from_state(state.inner());
    ocr::warmup(&app, &config).await
}

#[tauri::command]
async fn restart_ocr_service(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<RwLock<AppConfig>>>,
) -> Result<String, String> {
    let config = ocr_runtime_config_from_state(state.inner());
    ocr::restart(&app, &config).await
}

#[tauri::command]
fn get_ocr_log_path(app: tauri::AppHandle) -> Result<String, String> {
    ocr::log_path(&app).map(|path| path.display().to_string())
}

#[tauri::command]
async fn select_screenshot_area(app: tauri::AppHandle) -> Result<String, String> {
    screenshot::select_and_capture(app).await
}

#[tauri::command]
fn get_screenshot_selection_payload() -> Result<screenshot::SelectionStartPayload, String> {
    screenshot::get_screenshot_selection_payload()
}

#[tauri::command]
fn open_main_translate_window(app: tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("主窗口不存在".to_string());
    };

    window
        .emit("navigate-to-translate", ())
        .map_err(|e| e.to_string())?;

    let is_frontmost = window.is_visible().unwrap_or(false)
        && !window.is_minimized().unwrap_or(false)
        && window.is_focused().unwrap_or(false);

    if is_frontmost {
        return Ok(());
    }

    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}


#[tauri::command]
async fn toggle_favorite(app: tauri::AppHandle, id: i64, is_favorite: bool) -> Result<(), String> {
    let connection = open_translations_connection(&app)?;
    toggle_favorite_in_connection(&connection, id, is_favorite)
}

#[tauri::command]
async fn load_favorites(app: tauri::AppHandle) -> Result<Vec<TranslationRecord>, String> {
    let connection = open_translations_connection(&app)?;
    load_favorites_in_connection(&connection)
}

#[tauri::command]
async fn load_history(app: tauri::AppHandle) -> Result<Vec<TranslationRecord>, String> {
    let connection = open_translations_connection(&app)?;
    load_history_in_connection(&connection)
}

#[tauri::command]
async fn get_translation_by_id(app: tauri::AppHandle, id: i64) -> Result<TranslationRecord, String> {
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
            let mut persisted_settings = match load_persisted_settings(&app.handle().clone()) {
                Ok(settings) => settings,
                Err(error) => {
                    warn!("加载持久化设置失败，使用默认值: {}", error);
                    PersistedSettings::default()
                }
            };

            if adapt_ocr_settings_for_packaged_runtime(
                &app.handle().clone(),
                &mut persisted_settings,
            ) {
                info!(
                    "已根据当前安装包切换 OCR 运行时: engine={}, profile={}",
                    persisted_settings.ocr_engine, persisted_settings.ocr_model_profile
                );
                if let Err(error) =
                    save_persisted_settings(&app.handle().clone(), &persisted_settings)
                {
                    warn!("保存 OCR 运行时迁移设置失败: {}", error);
                }
            }

            // 初始化状态管理
            let config = Arc::new(RwLock::new(AppConfig {
                api_key: persisted_settings.api_key.clone(),
                api_secret: persisted_settings.api_secret.clone(),
                translation_provider: persisted_settings.translation_provider.clone(),
                microsoft_translator_key: persisted_settings.microsoft_translator_key.clone(),
                microsoft_translator_region: persisted_settings.microsoft_translator_region.clone(),
                ocr_endpoint: persisted_settings.ocr_endpoint.clone(),
                ocr_engine: persisted_settings.ocr_engine.clone(),
                ocr_model_profile: persisted_settings.ocr_model_profile.clone(),
                ocr_preload_on_startup: persisted_settings.ocr_preload_on_startup,
                global_shortcut: persisted_settings.global_shortcut.clone(),
                screenshot_shortcut: persisted_settings.screenshot_shortcut.clone(),
                theme: persisted_settings.theme.clone(),
            }));
            app.manage(config.clone());
            let translation_workflow =
                translation_workflow::create_app_workflow(app.handle().clone(), config.clone());
            app.manage(translation_workflow);

            let tray_behavior = Arc::new(RwLock::new(TrayBehaviorConfig {
                enabled: persisted_settings.enable_tray,
            }));
            app.manage(tray_behavior.clone());

            let popup_state = Arc::new(RwLock::new(PopupRuntimeState::default()));
            app.manage(popup_state.clone());

            let ocr_service_state = Arc::new(ocr_service::OcrServiceState::default());
            app.manage(ocr_service_state);

            // 数据迁移
            if let Err(error) = migrate_legacy_app_data(&app.handle().clone()) {
                error!("迁移旧版应用数据失败: {}", error);
            }

            ocr::spawn_startup_check(
                app.handle().clone(),
                OcrRuntimeConfig {
                    endpoint: persisted_settings.ocr_endpoint.clone(),
                    engine: persisted_settings.ocr_engine.clone(),
                    model_profile: persisted_settings.ocr_model_profile.clone(),
                    preload_on_startup: persisted_settings.ocr_preload_on_startup,
                },
            );

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
            if let Err(error) =
                popup_window::ensure_popup_window(&app.handle().clone(), &popup_state)
            {
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
                    } = event
                    {
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
                        if tray_behavior.read().enabled {
                            let _ = window_clone.hide();
                            api.prevent_close();
                        } else {
                            app_handle.exit(0);
                        }
                    }
                });
            }

            // 注册快捷键
            let desired_shortcut = config.read().global_shortcut.clone();
            let desired_screenshot_shortcut = config.read().screenshot_shortcut.clone();

            if let Err(error) = shortcut_handler::register_shortcut_handler(
                &app.handle().clone(),
                &desired_shortcut,
                popup_state.clone(),
            ) {
                warn!("注册持久化快捷键失败，回退到默认快捷键: {}", error);
                shortcut_handler::register_shortcut_handler(
                    &app.handle().clone(),
                    DEFAULT_GLOBAL_SHORTCUT,
                    popup_state.clone(),
                )
                .map_err(|e| e.to_string())?;

                {
                    let mut config_state = config.write();
                    config_state.global_shortcut = DEFAULT_GLOBAL_SHORTCUT.to_string();
                }

                if let Err(error) =
                    persist_managed_settings(&app.handle().clone(), &config, &tray_behavior)
                {
                    warn!("持久化默认快捷键失败: {}", error);
                }
            }

            if let Err(error) = shortcut_handler::register_screenshot_shortcut_handler(
                &app.handle().clone(),
                &desired_screenshot_shortcut,
                popup_state.clone(),
            ) {
                warn!("注册持久化截图快捷键失败，回退到默认快捷键: {}", error);
                if let Err(default_error) = shortcut_handler::register_screenshot_shortcut_handler(
                    &app.handle().clone(),
                    DEFAULT_SCREENSHOT_SHORTCUT,
                    popup_state.clone(),
                ) {
                    warn!(
                        "注册默认截图快捷键失败，已跳过截图快捷键: {}",
                        default_error
                    );
                    return Ok(());
                }

                let global_shortcut = config.read().global_shortcut.clone();
                if let Err(error) = update_and_persist_global_shortcuts(
                    &app.handle().clone(),
                    &config,
                    &tray_behavior,
                    global_shortcut,
                    DEFAULT_SCREENSHOT_SHORTCUT.to_string(),
                ) {
                    warn!("持久化默认截图快捷键失败: {}", error);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            translate_from_clipboard,
            translate_text,
            translate_image,
            select_screenshot_area,
            get_screenshot_selection_payload,
            open_main_translate_window,
            check_ocr_service,
            get_ocr_service_status,
            warmup_ocr_service,
            restart_ocr_service,
            get_ocr_log_path,
            toggle_favorite,
            load_favorites,
            load_history,
            get_translation_by_id,
            get_settings,
            update_api_config,
            update_global_shortcut,
            update_screenshot_shortcut,
            update_tray_behavior,
            update_theme,
            logger::get_log_files,
            logger::read_log_file,
            logger::get_log_dir_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
