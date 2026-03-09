// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod database;
mod translator;
mod clipboard;
pub mod local_dictionary;

use database::{Translation, INIT_SQL};
use local_dictionary::OfflineDictionaryEntry;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, Emitter, Listener, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use mouse_position::mouse_position::Mouse;
use enigo::{Enigo, Key, Keyboard, Settings};

// 配置结构
#[derive(Clone, Default)]
struct AppConfig {
    api_key: String,
    api_secret: String,
}

#[derive(Clone)]
struct TrayBehaviorConfig {
    enabled: bool,
}

impl Default for TrayBehaviorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
        }
    }
}

#[derive(Default)]
struct PopupRuntimeState {
    ready: bool,
    active_request_id: u64,
}

fn is_local_dictionary_candidate(text: &str) -> bool {
    !text.contains(' ')
        && !text.contains(',')
        && !text.contains('.')
        && text.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn some_if_not_empty(items: Vec<String>) -> Option<Vec<String>> {
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn to_translation_content(entry: OfflineDictionaryEntry) -> translator::TranslationContent {
    translator::TranslationContent {
        translated_text: entry.translated_text,
        phonetic: entry.phonetic,
        us_phonetic: entry.us_phonetic,
        uk_phonetic: entry.uk_phonetic,
        audio_url: entry.audio_url,
        explains: entry.explains,
        examples: entry.examples,
        synonyms: entry.synonyms,
        word_type: entry.word_type,
    }
}

fn build_translation_with_timestamp(
    text: String,
    content: translator::TranslationContent,
    created_at: i64,
) -> Translation {
    Translation {
        id: None,
        source_text: text,
        translated_text: content.translated_text,
        phonetic: content.phonetic,
        us_phonetic: content.us_phonetic,
        uk_phonetic: content.uk_phonetic,
        audio_url: content.audio_url,
        explains: some_if_not_empty(content.explains),
        examples: some_if_not_empty(content.examples),
        synonyms: some_if_not_empty(content.synonyms),
        source_lang: "en".to_string(),
        target_lang: "zh".to_string(),
        word_type: content.word_type,
        created_at,
        access_count: 1,
        is_favorite: 0,
    }
}

fn build_translation(text: String, content: translator::TranslationContent) -> Translation {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    build_translation_with_timestamp(text, content, now)
}

fn build_translation_from_existing(
    base: &Translation,
    content: translator::TranslationContent,
) -> Translation {
    Translation {
        id: base.id,
        source_text: base.source_text.clone(),
        translated_text: content.translated_text,
        phonetic: content.phonetic,
        us_phonetic: content.us_phonetic,
        uk_phonetic: content.uk_phonetic,
        audio_url: content.audio_url,
        explains: some_if_not_empty(content.explains),
        examples: some_if_not_empty(content.examples),
        synonyms: some_if_not_empty(content.synonyms),
        source_lang: base.source_lang.clone(),
        target_lang: base.target_lang.clone(),
        word_type: content.word_type,
        created_at: base.created_at,
        access_count: base.access_count,
        is_favorite: base.is_favorite,
    }
}

fn next_popup_request_id(state: &Arc<RwLock<PopupRuntimeState>>) -> u64 {
    let mut popup_state = state.write().unwrap();
    popup_state.active_request_id += 1;
    popup_state.active_request_id
}

fn is_active_popup_request(state: &Arc<RwLock<PopupRuntimeState>>, request_id: u64) -> bool {
    state.read().unwrap().active_request_id == request_id
}

fn is_popup_ready(state: &Arc<RwLock<PopupRuntimeState>>) -> bool {
    state.read().unwrap().ready
}

fn mark_popup_ready(state: &Arc<RwLock<PopupRuntimeState>>, ready: bool) {
    state.write().unwrap().ready = ready;
}

fn lookup_local_translation(
    app: &tauri::AppHandle,
    text: &str,
) -> Result<Option<(OfflineDictionaryEntry, Translation)>, String> {
    if !is_local_dictionary_candidate(text) {
        return Ok(None);
    }

    let Some(entry) = local_dictionary::lookup_word(app, text)? else {
        println!("本地词典未命中: {}", text);
        return Ok(None);
    };

    println!("本地词典命中: {}", text);
    let translation = build_translation(text.to_string(), to_translation_content(entry.clone()));
    Ok(Some((entry, translation)))
}

async fn resolve_translation(
    app: &tauri::AppHandle,
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<Translation, String> {
    if let Some((entry, base_translation)) = lookup_local_translation(app, text)? {
            let supplement = match translator::fetch_free_dictionary_supplement(text).await {
                Ok(supplement) => supplement,
                Err(error) => {
                    println!("Free Dictionary 补全失败: {}", error);
                    None
                }
            };
            let merged = local_dictionary::merge_free_dictionary_supplement(entry, supplement);
            return Ok(build_translation_from_existing(
                &base_translation,
                to_translation_content(merged),
            ));
    }

    if app_key.is_empty() || app_secret.is_empty() {
        return Err("未命中本地词典，且未配置翻译 API".to_string());
    }

    let content = translator::translate_text(text, app_key, app_secret).await?;
    Ok(build_translation(text.to_string(), content))
}

// 快捷键处理函数
fn handle_shortcut(
    app: tauri::AppHandle,
    config: Arc<RwLock<AppConfig>>,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) {
    tauri::async_runtime::spawn(async move {
        println!("开始执行翻译流程...");
        let request_id = next_popup_request_id(&popup_state);

        // 1. 获取鼠标位置（在复制前获取，避免位置变化）
        let cursor_pos = get_cursor_position();
        let baseline_clipboard_sequence = clipboard::clipboard_sequence_number();

        // 2. 模拟 Ctrl+C 复制选中文本
        std::thread::sleep(std::time::Duration::from_millis(50));
        println!("正在模拟 Ctrl+C 复制...");

        let mut enigo = Enigo::new(&Settings::default()).unwrap();
        enigo.key(Key::Control, enigo::Direction::Press).ok();
        enigo.key(Key::Unicode('c'), enigo::Direction::Click).ok();
        enigo.key(Key::Control, enigo::Direction::Release).ok();

        println!("复制完成，等待剪贴板更新...");

        // 3. 读取剪贴板，避免系统尚未完成更新时直接失败
        let text = match clipboard::read_clipboard_after_update(
            &app,
            baseline_clipboard_sequence,
            6,
            80,
        ) {
            Ok(t) => {
                println!("读取到剪贴板内容: {}", t);
                t
            },
            Err(e) => {
                println!("读取剪贴板失败: {:?}", e);
                if is_active_popup_request(&popup_state, request_id) {
                    let _ = close_popup_window(&app);
                }
                return;
            },
        };

        if text.is_empty() {
            println!("剪贴板内容为空");
            if is_active_popup_request(&popup_state, request_id) {
                let _ = close_popup_window(&app);
            }
            return;
        }

        if let Some((local_entry, local_translation)) = match lookup_local_translation(&app, &text) {
            Ok(result) => result,
            Err(error) => {
                println!("本地词典查询失败: {}", error);
                None
            }
        } {
            if !is_active_popup_request(&popup_state, request_id) {
                return;
            }

            let _ = show_popup_translation(
                &app,
                &popup_state,
                request_id,
                "translation-result",
                local_translation.clone(),
                cursor_pos,
                true,
            );

            let app_clone = app.clone();
            let popup_state_clone = popup_state.clone();
            let text_clone = text.clone();
            tauri::async_runtime::spawn(async move {
                let supplement = match translator::fetch_free_dictionary_supplement(&text_clone).await {
                    Ok(Some(supplement)) => supplement,
                    Ok(None) => return,
                    Err(error) => {
                        println!("Free Dictionary 补全失败: {}", error);
                        return;
                    }
                };

                if !is_active_popup_request(&popup_state_clone, request_id) {
                    return;
                }

                let merged = local_dictionary::merge_free_dictionary_supplement(
                    local_entry,
                    Some(supplement),
                );
                let enriched_translation = build_translation_from_existing(
                    &local_translation,
                    to_translation_content(merged),
                );

                if enriched_translation != local_translation {
                    let _ = show_popup_translation(
                        &app_clone,
                        &popup_state_clone,
                        request_id,
                        "translation-update",
                        enriched_translation,
                        cursor_pos,
                        false,
                    );
                }
            });

            return;
        }

        // 4. 获取配置
        let (app_key, app_secret) = {
            let cfg = config.read().unwrap();
            (cfg.api_key.clone(), cfg.api_secret.clone())
        };

        println!("开始调用翻译 API...");
        let _ = show_loading_popup(&app, &popup_state, request_id, cursor_pos);

        let result = match resolve_translation(&app, &text, &app_key, &app_secret).await {
            Ok(translation) => {
                println!("翻译成功: {} -> {}", text, translation.translated_text);
                translation
            }
            Err(error) => {
                println!("翻译失败: {:?}", error);
                if is_active_popup_request(&popup_state, request_id) {
                    let _ = close_popup_window(&app);
                }
                return;
            }
        };

        if !is_active_popup_request(&popup_state, request_id) {
            return;
        }

        let _ = show_popup_translation(
            &app,
            &popup_state,
            request_id,
            "translation-result",
            result,
            cursor_pos,
            true,
        );
    });
}

#[tauri::command]
fn update_api_config(
    state: tauri::State<Arc<RwLock<AppConfig>>>,
    api_key: String,
    api_secret: String,
) -> Result<(), String> {
    let mut config = state.write().unwrap();
    config.api_key = api_key;
    config.api_secret = api_secret;
    Ok(())
}

#[tauri::command]
fn update_global_shortcut(
    app: tauri::AppHandle,
    old_shortcut: String,
    new_shortcut: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    // 注销旧快捷键
    if let Ok(old) = old_shortcut.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(old);
    }

    // 注册新快捷键
    let shortcut: Shortcut = new_shortcut.parse()
        .map_err(|e| format!("无效的快捷键格式: {}", e))?;

    let config = app.state::<Arc<RwLock<AppConfig>>>();
    let config_clone = config.inner().clone();
    let popup_state = app.state::<Arc<RwLock<PopupRuntimeState>>>();
    let popup_state_clone = popup_state.inner().clone();

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            handle_shortcut(app.clone(), config_clone.clone(), popup_state_clone.clone());
        })
        .map_err(|e| format!("注册快捷键失败: {}", e))?;

    Ok(())
}

#[tauri::command]
fn update_tray_behavior(
    state: tauri::State<Arc<RwLock<TrayBehaviorConfig>>>,
    enabled: bool,
) -> Result<(), String> {
    let mut config = state.write().unwrap();
    config.enabled = enabled;
    Ok(())
}

#[tauri::command]
async fn translate_from_clipboard(
    app: tauri::AppHandle,
    app_key: String,
    app_secret: String,
) -> Result<Translation, String> {
    // 读取剪贴板
    let text = clipboard::read_clipboard(&app)?;

    if text.trim().is_empty() {
        return Err("剪贴板为空".to_string());
    }

    resolve_translation(&app, &text.trim(), &app_key, &app_secret).await
}

#[tauri::command]
fn get_init_sql() -> String {
    INIT_SQL.to_string()
}

#[tauri::command]
async fn save_translation(
    _app: tauri::AppHandle,
    _translation: Translation,
) -> Result<i64, String> {
    // TODO: 实现数据库保存
    Ok(1)
}

#[tauri::command]
async fn toggle_favorite(
    _app: tauri::AppHandle,
    _id: i64,
    _is_favorite: bool,
) -> Result<(), String> {
    // TODO: 实现数据库更新
    Ok(())
}

#[tauri::command]
async fn load_favorites(_app: tauri::AppHandle) -> Result<Vec<Translation>, String> {
    // TODO: 实现查询收藏列表
    Ok(vec![])
}

#[tauri::command]
async fn load_history(_app: tauri::AppHandle) -> Result<Vec<Translation>, String> {
    // TODO: 实现查询历史记录
    Ok(vec![])
}

#[tauri::command]
async fn get_translation_by_id(_app: tauri::AppHandle, _id: i64) -> Result<Translation, String> {
    // TODO: 实现根据 ID 查询
    Err("Not implemented".to_string())
}

// 获取鼠标位置
fn get_cursor_position() -> (i32, i32) {
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        Mouse::Error => (100, 100), // 默认位置
    }
}

// 关闭弹窗
fn close_popup_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("popup") {
        window.hide().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

fn set_popup_position(
    window: &tauri::WebviewWindow,
    cursor_pos: (i32, i32),
) -> Result<(), String> {
    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: cursor_pos.0,
            y: cursor_pos.1,
        }))
        .map_err(|e: tauri::Error| e.to_string())
}

fn build_popup_window(
    app: &tauri::AppHandle,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) -> Result<tauri::WebviewWindow, String> {
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "popup",
        tauri::WebviewUrl::App("index.html".into()),
    )
        .title("翻译")
        .inner_size(420.0, 380.0)
        .decorations(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .initialization_script("window.location.hash = '#/popup';")
        .build()
        .map_err(|e| e.to_string())?;

    let popup_state_clone = popup_state.clone();
    window.listen("popup-ready", move |_event| {
        mark_popup_ready(&popup_state_clone, true);
    });

    Ok(window)
}

fn ensure_popup_window(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("popup") {
        return Ok(window);
    }

    mark_popup_ready(popup_state, false);
    build_popup_window(app, popup_state.clone())
}

fn with_popup_ready<F>(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    cursor_pos: (i32, i32),
    action: F,
) -> Result<(), String>
where
    F: Fn(&tauri::WebviewWindow) + Send + Sync + 'static,
{
    let window = ensure_popup_window(app, popup_state)?;
    set_popup_position(&window, cursor_pos)?;

    if is_popup_ready(popup_state) {
        action(&window);
        return Ok(());
    }

    let popup_state_clone = popup_state.clone();
    let window_clone = window.clone();
    let action = Arc::new(action);
    let deferred_action = action.clone();

    window.once("popup-ready", move |_event| {
        if !is_active_popup_request(&popup_state_clone, request_id) {
            return;
        }

        let _ = set_popup_position(&window_clone, cursor_pos);
        deferred_action(&window_clone);
    });

    Ok(())
}

fn show_loading_popup(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    cursor_pos: (i32, i32),
) -> Result<(), String> {
    if !is_active_popup_request(popup_state, request_id) {
        return Ok(());
    }

    with_popup_ready(app, popup_state, request_id, cursor_pos, |window| {
        let _ = window.emit("translation-started", ());
        let _ = window.show();
    })
}

fn show_popup_translation(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    event_name: &'static str,
    translation: Translation,
    cursor_pos: (i32, i32),
    should_show: bool,
) -> Result<(), String> {
    if !is_active_popup_request(popup_state, request_id) {
        return Ok(());
    }

    with_popup_ready(app, popup_state, request_id, cursor_pos, move |window| {
        let _ = window.emit(event_name, &translation);
        if should_show {
            let _ = window.show();
        }
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // 初始化配置状态
            let config = Arc::new(RwLock::new(AppConfig::default()));
            app.manage(config.clone());

            let tray_behavior = Arc::new(RwLock::new(TrayBehaviorConfig::default()));
            app.manage(tray_behavior.clone());

            let popup_state = Arc::new(RwLock::new(PopupRuntimeState::default()));
            app.manage(popup_state.clone());

            match local_dictionary::ensure_runtime_dictionary(&app.handle().clone()) {
                Ok(Some(path)) => {
                    println!("本地词典已就绪: {}", path.display());
                }
                Ok(None) => {
                    println!("未找到内置词典资源，单词查询将回退到在线链路");
                }
                Err(error) => {
                    println!("初始化本地词典失败: {}", error);
                }
            }

            if let Err(error) = ensure_popup_window(&app.handle().clone(), &popup_state) {
                println!("预热弹窗失败: {}", error);
            }

            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
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
                    }
                })
                .build(app)?;

            // 监听主窗口关闭事件，隐藏而不是退出
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

            // 注册全局快捷键
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

            let shortcut: Shortcut = "Ctrl+Q".parse().unwrap();
            let config_clone = config.clone();
            let popup_state_clone = popup_state.clone();

            app.global_shortcut()
                .on_shortcut(shortcut, move |app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    handle_shortcut(app.clone(), config_clone.clone(), popup_state_clone.clone());
                })
                .map_err(|e| e.to_string())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            translate_from_clipboard,
            get_init_sql,
            save_translation,
            toggle_favorite,
            load_favorites,
            load_history,
            get_translation_by_id,
            update_api_config,
            update_global_shortcut,
            update_tray_behavior
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
