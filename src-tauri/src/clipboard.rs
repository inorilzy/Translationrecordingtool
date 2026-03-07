pub fn read_clipboard(app: &tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .read_text()
        .map_err(|e| format!("读取剪贴板失败: {}", e))
}
