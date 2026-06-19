/// Global shortcut registration and translation flow orchestration.
use std::sync::{Arc, RwLock};

use tracing::{error, info, warn};

use crate::app_state::{
    AppConfig, PopupRuntimeState, is_active_popup_request, next_popup_request_id,
};
use crate::popup_window::{
    close_popup_window, get_cursor_position, show_loading_popup, show_popup_translation,
};
use crate::translation_flow;

// ─── Shortcut Registration ───────────────────────────────────────────────────

pub fn register_shortcut_handler(
    app: &tauri::AppHandle,
    shortcut_value: &str,
    config: Arc<RwLock<AppConfig>>,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let shortcut: Shortcut = shortcut_value
        .parse()
        .map_err(|e| format!("无效的快捷键格式: {}", e))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            handle_shortcut(app.clone(), config.clone(), popup_state.clone());
        })
        .map_err(|e| format!("注册快捷键失败: {}", e))
}

// ─── Shortcut Handler ────────────────────────────────────────────────────────

pub fn handle_shortcut(
    app: tauri::AppHandle,
    config: Arc<RwLock<AppConfig>>,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) {
    tauri::async_runtime::spawn(async move {
        info!("开始执行翻译流程");
        let request_id = next_popup_request_id(&popup_state);

        // 1. 获取鼠标位置（复制前获取）
        let cursor_pos = get_cursor_position();
        let baseline_clipboard_sequence = crate::clipboard::clipboard_sequence_number();

        // 2. 模拟 Ctrl+C
        std::thread::sleep(std::time::Duration::from_millis(50));
        info!("正在模拟 Ctrl+C 复制");

        use enigo::{Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default()).unwrap();
        enigo.key(Key::Control, enigo::Direction::Press).ok();
        enigo.key(Key::Unicode('c'), enigo::Direction::Click).ok();
        enigo.key(Key::Control, enigo::Direction::Release).ok();

        info!("复制完成，等待剪贴板更新");

        // 3. 读取剪贴板
        let text = match crate::clipboard::read_clipboard_after_update(
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

        // 4. 先查本地词典（快速路径）
        if let Some((local_entry, local_translation)) =
            match translation_flow::lookup_local_translation(&app, &text) {
                Ok(result) => result,
                Err(error) => {
                    error!("本地词典查询失败: {}", error);
                    None
                }
            }
        {
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

            // 异步补全 Free Dictionary
            let app_clone = app.clone();
            let popup_state_clone = popup_state.clone();
            let text_clone = text.clone();
            tauri::async_runtime::spawn(async move {
                let supplement =
                    match crate::translator::fetch_free_dictionary_supplement(&text_clone).await {
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

                let merged = crate::local_dictionary::merge_free_dictionary_supplement(
                    local_entry,
                    Some(supplement),
                );
                let enriched_translation = translation_flow::build_translation_from_existing(
                    &local_translation,
                    translation_flow::to_translation_content(merged),
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

        // 5. 本地未命中 → 查询远程翻译
        let translation_config = {
            let cfg = config.read().unwrap();
            translation_flow::TranslationConfig {
                provider: cfg.translation_provider.clone(),
                youdao_app_key: cfg.api_key.clone(),
                youdao_app_secret: cfg.api_secret.clone(),
                microsoft_key: cfg.microsoft_translator_key.clone(),
                microsoft_region: cfg.microsoft_translator_region.clone(),
            }
        };

        info!("开始查询翻译");
        let _ = show_loading_popup(&app, &popup_state, request_id, cursor_pos);

        let result = match translation_flow::resolve_remote_translation(&text, &translation_config).await {
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
