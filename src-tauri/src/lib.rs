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

fn build_translation(text: String, content: translator::TranslationContent) -> Translation {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

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
        created_at: now,
        access_count: 1,
        is_favorite: 0,
    }
}

async fn resolve_translation(
    app: &tauri::AppHandle,
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<Translation, String> {
    if is_local_dictionary_candidate(text) {
        if let Some(entry) = local_dictionary::lookup_word(app, text)? {
            println!("本地词典命中: {}", text);
            let supplement = match translator::fetch_free_dictionary_supplement(text).await {
                Ok(supplement) => supplement,
                Err(error) => {
                    println!("Free Dictionary 补全失败: {}", error);
                    None
                }
            };
            let merged = local_dictionary::merge_free_dictionary_supplement(entry, supplement);
            return Ok(build_translation(text.to_string(), to_translation_content(merged)));
        }

        println!("本地词典未命中: {}", text);
    }

    if app_key.is_empty() || app_secret.is_empty() {
        return Err("未命中本地词典，且未配置翻译 API".to_string());
    }

    let content = translator::translate_text(text, app_key, app_secret).await?;
    Ok(build_translation(text.to_string(), content))
}

// 快捷键处理函数
fn handle_shortcut(app: tauri::AppHandle, config: Arc<RwLock<AppConfig>>) {
    tauri::async_runtime::spawn(async move {
        println!("开始执行翻译流程...");

        // 1. 获取鼠标位置（在复制前获取，避免位置变化）
        let cursor_pos = get_cursor_position();

        // 2. 立即显示加载窗口
        if let Err(e) = show_loading_popup(&app, cursor_pos) {
            println!("显示加载窗口失败: {}", e);
            return;
        }

        // 3. 模拟 Ctrl+C 复制选中文本
        std::thread::sleep(std::time::Duration::from_millis(50));
        println!("正在模拟 Ctrl+C 复制...");

        let mut enigo = Enigo::new(&Settings::default()).unwrap();
        enigo.key(Key::Control, enigo::Direction::Press).ok();
        enigo.key(Key::Unicode('c'), enigo::Direction::Click).ok();
        enigo.key(Key::Control, enigo::Direction::Release).ok();

        // 等待剪贴板更新
        std::thread::sleep(std::time::Duration::from_millis(150));
        println!("复制完成，等待剪贴板更新...");

        // 4. 读取剪贴板
        let text = match clipboard::read_clipboard(&app) {
            Ok(t) => {
                let trimmed = t.trim().to_string();
                println!("读取到剪贴板内容: {}", trimmed);
                trimmed
            },
            Err(e) => {
                println!("读取剪贴板失败: {:?}", e);
                let _ = close_popup_window(&app);
                return;
            },
        };

        if text.is_empty() {
            println!("剪贴板内容为空");
            let _ = close_popup_window(&app);
            return;
        }

        // 5. 获取配置
        let (app_key, app_secret) = {
            let cfg = config.read().unwrap();
            (cfg.api_key.clone(), cfg.api_secret.clone())
        };

        println!("开始调用翻译 API...");

        let result = match resolve_translation(&app, &text, &app_key, &app_secret).await {
            Ok(translation) => {
                println!("翻译成功: {} -> {}", text, translation.translated_text);
                translation
            }
            Err(error) => {
                println!("翻译失败: {:?}", error);
                let _ = close_popup_window(&app);
                return;
            }
        };

        // 7. 更新窗口内容
        let _ = update_popup_window(&app, result);
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

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            handle_shortcut(app.clone(), config_clone.clone());
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

// 显示加载窗口
fn show_loading_popup(
    app: &tauri::AppHandle,
    cursor_pos: (i32, i32),
) -> Result<(), String> {
    // 检查窗口是否已存在
    if let Some(window) = app.get_webview_window("popup") {
        // 更新位置并显示
        window.set_position(tauri::Position::Physical(
            tauri::PhysicalPosition { x: cursor_pos.0, y: cursor_pos.1 }
        )).map_err(|e: tauri::Error| e.to_string())?;

        window.show().map_err(|e: tauri::Error| e.to_string())?;
        window.set_focus().map_err(|e: tauri::Error| e.to_string())?;
    } else {
        // 创建新窗口（先隐藏）
        let window = tauri::WebviewWindowBuilder::new(
            app,
            "popup",
            tauri::WebviewUrl::App("index.html".into())
        )
        .title("翻译")
        .inner_size(420.0, 380.0)
        .decorations(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .position(cursor_pos.0 as f64, cursor_pos.1 as f64)
        .visible(false)  // 先隐藏
        .initialization_script("window.location.hash = '#/popup';")
        .build()
        .map_err(|e| e.to_string())?;

        // 等待窗口准备好后再显示
        std::thread::sleep(std::time::Duration::from_millis(50));
        window.show().map_err(|e: tauri::Error| e.to_string())?;
        window.set_focus().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

// 更新窗口内容
fn update_popup_window(
    app: &tauri::AppHandle,
    translation: Translation,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("popup") {
        window.emit("translation-result", &translation).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

// 关闭弹窗
fn close_popup_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("popup") {
        window.hide().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

// 显示悬浮窗（保留用于手动翻译）
fn show_popup_window(
    app: &tauri::AppHandle,
    translation: Translation,
    cursor_pos: (i32, i32),
) -> Result<(), String> {
    // 检查窗口是否已存在
    if let Some(window) = app.get_webview_window("popup") {
        // 更新位置
        window.set_position(tauri::Position::Physical(
            tauri::PhysicalPosition { x: cursor_pos.0, y: cursor_pos.1 }
        )).map_err(|e: tauri::Error| e.to_string())?;

        // 发送翻译结果
        window.emit("translation-result", &translation).map_err(|e: tauri::Error| e.to_string())?;
        window.show().map_err(|e: tauri::Error| e.to_string())?;
        window.set_focus().map_err(|e: tauri::Error| e.to_string())?;
    } else {
        // 创建新窗口
        let window = tauri::WebviewWindowBuilder::new(
            app,
            "popup",
            tauri::WebviewUrl::App("index.html".into())
        )
        .title("翻译")
        .inner_size(420.0, 380.0)
        .decorations(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .position(cursor_pos.0 as f64, cursor_pos.1 as f64)
        .visible(false)  // 先隐藏
        .initialization_script("window.location.hash = '#/popup';")
        .build()
        .map_err(|e| e.to_string())?;

        // 监听前端就绪事件
        let translation_clone = translation.clone();
        let window_clone = window.clone();

        window.once("popup-ready", move |_event| {
            // 前端已就绪，发送翻译结果
            let _ = window_clone.emit("translation-result", &translation_clone);
            let _ = window_clone.show();
        });
    }

    Ok(())
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

            app.global_shortcut()
                .on_shortcut(shortcut, move |app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    handle_shortcut(app.clone(), config_clone.clone());
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
