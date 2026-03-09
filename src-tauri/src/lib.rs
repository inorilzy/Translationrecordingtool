// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod clipboard;
mod database;
pub mod local_dictionary;
mod logger;
mod translator;

use database::{Translation, INIT_SQL};
use enigo::{Enigo, Key, Keyboard, Settings};
use local_dictionary::OfflineDictionaryEntry;
use mouse_position::mouse_position::Mouse;
use std::{
    fs,
    path::Path,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Listener, Manager,
};
use tracing::{error, info, warn};

const LEGACY_APP_DATA_DIR_NAME: &str = "com.zhiyu_liu.translation-tool";
const RUNTIME_DATA_FILES: &[&str] = &["translations.db", "dictionary.db"];

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
        Self { enabled: true }
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

fn read_current_clipboard_text(app: &tauri::AppHandle) -> Result<String, String> {
    let text = clipboard::read_clipboard(app)?;
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Err("剪贴板为空".to_string());
    }

    Ok(trimmed.to_string())
}

fn migrate_legacy_app_data(app: &tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    migrate_legacy_app_data_dir(&app_data_dir, LEGACY_APP_DATA_DIR_NAME, RUNTIME_DATA_FILES)
}

fn migrate_legacy_app_data_dir(
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
        info!("本地词典未命中: {}", text);
        return Ok(None);
    };

    info!("本地词典命中: {}", text);
    let translation = build_translation(text.to_string(), to_translation_content(entry.clone()));
    Ok(Some((entry, translation)))
}

fn is_single_word(text: &str) -> bool {
    let trimmed = text.trim();

    // 包含空格、逗号、句号等标点符号，判定为句子
    if trimmed.contains(' ') || trimmed.contains(',') || trimmed.contains('.') || trimmed.contains('!') || trimmed.contains('?') {
        return false;
    }

    // 包含驼峰命名（如 localStorage, getElementById），判定为代码/术语，不是普通单词
    let has_internal_uppercase = trimmed.chars().skip(1).any(|c| c.is_uppercase());
    if has_internal_uppercase {
        return false;
    }

    // 只包含字母（允许撇号，如 "don't"）和连字符（如 "well-known"）
    trimmed.chars().all(|c| c.is_alphabetic() || c == '\'' || c == '-')
}

async fn resolve_translation(
    app: &tauri::AppHandle,
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<Translation, String> {
    let is_word = is_single_word(text);
    info!("翻译文本: {}, 类型: {}", text, if is_word { "单词" } else { "句子" });

    // 如果是单词，只使用本地词典和 Free Dictionary
    if is_word {
        // 1. 先查本地词典
        if let Some((entry, base_translation)) = lookup_local_translation(app, text)? {
            let supplement = match translator::fetch_free_dictionary_supplement(text).await {
                Ok(supplement) => supplement,
                Err(error) => {
                    warn!("Free Dictionary 补全失败: {}", error);
                    None
                }
            };
            let merged = local_dictionary::merge_free_dictionary_supplement(entry, supplement);
            return Ok(build_translation_from_existing(
                &base_translation,
                to_translation_content(merged),
            ));
        }

        // 2. 本地词典未命中，尝试 Free Dictionary
        info!("本地词典未命中，尝试 Free Dictionary");
        match translator::fetch_free_dictionary_supplement(text).await {
            Ok(Some(supplement)) => {
                info!("Free Dictionary 查询成功");
                let content = translator::TranslationContent {
                    translated_text: supplement.explains.first()
                        .and_then(|s| s.split(". ").nth(1))
                        .unwrap_or(text)
                        .to_string(),
                    phonetic: supplement.phonetic.clone(),
                    us_phonetic: supplement.phonetic.clone(),
                    uk_phonetic: None,
                    audio_url: supplement.audio_url,
                    explains: supplement.explains,
                    examples: supplement.examples,
                    synonyms: supplement.synonyms,
                    word_type: None,
                };
                return Ok(build_translation(text.to_string(), content));
            }
            Ok(None) => {
                warn!("Free Dictionary 未找到单词: {}", text);
                return Err(format!("未找到单词 \"{}\" 的释义", text));
            }
            Err(e) => {
                error!("Free Dictionary 查询失败: {}", e);
                return Err(format!("查询单词失败: {}", e));
            }
        }
    }

    // 句子使用有道翻译 API
    resolve_youdao_translation(text, app_key, app_secret).await
}

async fn resolve_youdao_translation(
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<Translation, String> {
    if app_key.is_empty() || app_secret.is_empty() {
        return Err("翻译句子需要配置有道翻译 API，请在设置中配置".to_string());
    }

    info!("使用有道翻译 API");
    let content = translator::translate_text(text, app_key, app_secret).await?;
    Ok(build_translation(text.to_string(), content))
}

async fn resolve_remote_translation(
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<Translation, String> {
    let is_word = is_single_word(text);
    info!("翻译文本: {}, 类型: {}", text, if is_word { "单词" } else { "句子" });

    // 如果是单词，只使用 Free Dictionary
    if is_word {
        info!("尝试 Free Dictionary");
        match translator::fetch_free_dictionary_supplement(text).await {
            Ok(Some(supplement)) => {
                info!("Free Dictionary 查询成功");
                let content = translator::TranslationContent {
                    translated_text: supplement.explains.first()
                        .and_then(|s| s.split(". ").nth(1))
                        .unwrap_or(text)
                        .to_string(),
                    phonetic: supplement.phonetic.clone(),
                    us_phonetic: supplement.phonetic.clone(),
                    uk_phonetic: None,
                    audio_url: supplement.audio_url,
                    explains: supplement.explains,
                    examples: supplement.examples,
                    synonyms: supplement.synonyms,
                    word_type: None,
                };
                return Ok(build_translation(text.to_string(), content));
            }
            Ok(None) => {
                warn!("Free Dictionary 未找到单词: {}", text);
                return Err(format!("未找到单词 \"{}\" 的释义", text));
            }
            Err(e) => {
                error!("Free Dictionary 查询失败: {}", e);
                return Err(format!("查询单词失败: {}", e));
            }
        }
    }

    // 句子使用有道翻译 API
    resolve_youdao_translation(text, app_key, app_secret).await
}

// 快捷键处理函数
fn handle_shortcut(
    app: tauri::AppHandle,
    config: Arc<RwLock<AppConfig>>,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) {
    tauri::async_runtime::spawn(async move {
        info!("开始执行翻译流程");
        let request_id = next_popup_request_id(&popup_state);

        // 1. 获取鼠标位置（在复制前获取，避免位置变化）
        let cursor_pos = get_cursor_position();
        let baseline_clipboard_sequence = clipboard::clipboard_sequence_number();

        // 2. 模拟 Ctrl+C 复制选中文本
        std::thread::sleep(std::time::Duration::from_millis(50));
        info!("正在模拟 Ctrl+C 复制");

        let mut enigo = Enigo::new(&Settings::default()).unwrap();
        enigo.key(Key::Control, enigo::Direction::Press).ok();
        enigo.key(Key::Unicode('c'), enigo::Direction::Click).ok();
        enigo.key(Key::Control, enigo::Direction::Release).ok();

        info!("复制完成，等待剪贴板更新");

        // 3. 读取剪贴板，避免系统尚未完成更新时直接失败
        let text = match clipboard::read_clipboard_after_update(
            &app,
            baseline_clipboard_sequence,
            6,
            80,
        ) {
            Ok(t) => {
                info!("读取到剪贴板内容: {}", t);
                t
            }
            Err(e) => {
                error!("读取剪贴板失败: {:?}", e);
                if is_active_popup_request(&popup_state, request_id) {
                    let _ = close_popup_window(&app);
                }
                return;
            }
        };

        if text.is_empty() {
            warn!("剪贴板内容为空");
            if is_active_popup_request(&popup_state, request_id) {
                let _ = close_popup_window(&app);
            }
            return;
        }

        if let Some((local_entry, local_translation)) = match lookup_local_translation(&app, &text)
        {
            Ok(result) => result,
            Err(error) => {
                error!("本地词典查询失败: {}", error);
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
                let supplement =
                    match translator::fetch_free_dictionary_supplement(&text_clone).await {
                        Ok(Some(supplement)) => supplement,
                        Ok(None) => return,
                        Err(error) => {
                            warn!("Free Dictionary 补全失败: {}", error);
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

        info!("开始查询翻译");
        let _ = show_loading_popup(&app, &popup_state, request_id, cursor_pos);

        let result = match resolve_remote_translation(&text, &app_key, &app_secret).await {
            Ok(translation) => {
                info!("翻译成功: {} -> {}", text, translation.translated_text);
                translation
            }
            Err(error) => {
                error!("翻译失败: {:?}", error);
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
    let shortcut: Shortcut = new_shortcut
        .parse()
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
    let text = read_current_clipboard_text(&app)?;
    resolve_translation(&app, &text, &app_key, &app_secret).await
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

fn set_popup_position(window: &tauri::WebviewWindow, cursor_pos: (i32, i32)) -> Result<(), String> {
    const POPUP_WIDTH: i32 = 420;
    const POPUP_HEIGHT: i32 = 380;
    const OFFSET: i32 = 15; // 鼠标偏移量
    const MARGIN: i32 = 20; // 屏幕边缘留白

    let mut x = cursor_pos.0 + OFFSET;
    let mut y = cursor_pos.1 + OFFSET;

    // 获取主显示器信息进行边界检测
    match window.current_monitor() {
        Ok(Some(monitor)) => {
            let screen_size = monitor.size();
            let screen_position = monitor.position();

            let screen_width = screen_size.width as i32;
            let screen_height = screen_size.height as i32;
            let screen_x = screen_position.x;
            let screen_y = screen_position.y;

            // 计算可用区域（减去边距）
            let usable_right = screen_x + screen_width - MARGIN;
            let usable_bottom = screen_y + screen_height - MARGIN;
            let usable_left = screen_x + MARGIN;
            let usable_top = screen_y + MARGIN;

            // X 轴调整：优先右侧，不够则左侧
            if x + POPUP_WIDTH > usable_right {
                // 尝试放在鼠标左侧
                let left_x = cursor_pos.0 - POPUP_WIDTH - OFFSET;
                if left_x >= usable_left {
                    x = left_x;
                } else {
                    // 左右都放不下，贴右边界
                    x = usable_right - POPUP_WIDTH;
                }
            }

            // Y 轴调整：优先下方，不够则上方
            if y + POPUP_HEIGHT > usable_bottom {
                // 尝试放在鼠标上方
                let top_y = cursor_pos.1 - POPUP_HEIGHT - OFFSET;
                if top_y >= usable_top {
                    y = top_y;
                } else {
                    // 上下都放不下，贴下边界
                    y = usable_bottom - POPUP_HEIGHT;
                }
            }

            // 最终边界保护
            x = x.max(usable_left).min(usable_right - POPUP_WIDTH);
            y = y.max(usable_top).min(usable_bottom - POPUP_HEIGHT);

            info!("弹窗位置: 鼠标({}, {}) -> 窗口({}, {})", cursor_pos.0, cursor_pos.1, x, y);
        }
        Ok(None) => {
            warn!("无法获取显示器信息，使用默认偏移");
        }
        Err(e) => {
            warn!("获取显示器信息失败: {}, 使用默认偏移", e);
        }
    }

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }))
        .map_err(|e: tauri::Error| e.to_string())
}

fn build_popup_window(
    app: &tauri::AppHandle,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) -> Result<tauri::WebviewWindow, String> {
    let window =
        tauri::WebviewWindowBuilder::new(app, "popup", tauri::WebviewUrl::App("index.html".into()))
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
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // 设置 Ctrl+C 信号处理
            let app_handle = app.handle().clone();
            ctrlc::set_handler(move || {
                info!("收到 Ctrl+C 信号，正在退出...");
                app_handle.exit(0);
            })
            .expect("设置 Ctrl+C 处理器失败");

            // 初始化日志系统
            let log_dir = app.path().app_log_dir().expect("无法获取日志目录");
            match logger::init_logger(log_dir) {
                Ok(guard) => {
                    app.manage(guard); // 保持 guard 生命周期
                    info!("应用启动");
                }
                Err(e) => {
                    eprintln!("初始化日志系统失败: {}", e);
                }
            }

            // 初始化配置状态
            let config = Arc::new(RwLock::new(AppConfig::default()));
            app.manage(config.clone());

            let tray_behavior = Arc::new(RwLock::new(TrayBehaviorConfig::default()));
            app.manage(tray_behavior.clone());

            let popup_state = Arc::new(RwLock::new(PopupRuntimeState::default()));
            app.manage(popup_state.clone());

            if let Err(error) = migrate_legacy_app_data(&app.handle().clone()) {
                error!("迁移旧版应用数据失败: {}", error);
            }

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

            if let Err(error) = ensure_popup_window(&app.handle().clone(), &popup_state) {
                error!("预热弹窗失败: {}", error);
            }

            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false) // 左键不显示菜单
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
                        // 左键点击显示主窗口
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
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
            update_tray_behavior,
            logger::get_log_files,
            logger::read_log_file,
            logger::get_log_dir_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod lib_tests {
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
