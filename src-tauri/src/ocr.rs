use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info};

#[derive(Debug, Serialize)]
struct OcrRequest<'a> {
    image: &'a str,
}

#[derive(Debug, Deserialize)]
struct OcrTextResponse {
    text: Option<String>,
}

pub async fn recognize_text(endpoint: &str, image_base64: &str) -> Result<String, String> {
    validate_endpoint(endpoint)?;

    let payload = image_base64
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(image_base64);

    info!("调用 Paddle OCR HTTP 服务");

    let response = crate::translator::http_client()
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(&OcrRequest { image: payload })
        .send()
        .await
        .map_err(|e| {
            let err_msg = format!("Paddle OCR 请求失败: {}", e);
            error!("{}", err_msg);
            err_msg
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let err_msg = format!("Paddle OCR 返回错误: {} {}", status, body);
        error!("{}", err_msg);
        return Err(err_msg);
    }

    let value: Value = response.json().await.map_err(|e| {
        let err_msg = format!("Paddle OCR 响应解析失败: {}", e);
        error!("{}", err_msg);
        err_msg
    })?;

    extract_text(&value).ok_or_else(|| "Paddle OCR 未返回可识别文本".to_string())
}

pub async fn check_service(endpoint: &str) -> Result<String, String> {
    validate_endpoint(endpoint)?;
    let health_url = health_url_from_endpoint(endpoint)?;

    let response = crate::translator::http_client()
        .get(&health_url)
        .send()
        .await
        .map_err(|e| format!("Paddle OCR 服务连接失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Paddle OCR 服务健康检查失败: {}",
            response.status()
        ));
    }

    Ok(format!("Paddle OCR 服务正常: {}", health_url))
}

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err("请先在设置中配置 Paddle OCR HTTP 地址".to_string());
    }
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err("Paddle OCR HTTP 地址必须以 http:// 或 https:// 开头".to_string());
    }
    Ok(())
}

fn health_url_from_endpoint(endpoint: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(endpoint.trim())
        .map_err(|e| format!("Paddle OCR HTTP 地址无效: {}", e))?;
    url.set_path("/health");
    url.set_query(None);
    Ok(url.to_string())
}

fn extract_text(value: &Value) -> Option<String> {
    if let Ok(response) = serde_json::from_value::<OcrTextResponse>(value.clone()) {
        if let Some(text) = clean_text(response.text) {
            return Some(text);
        }
    }

    for key in ["data", "result", "results", "texts", "ocrResults"] {
        if let Some(node) = value.get(key) {
            if let Some(text) = extract_text_from_node(node) {
                return Some(text);
            }
        }
    }

    extract_text_from_node(value)
}

fn extract_text_from_node(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => clean_text(Some(text.clone())),
        Value::Array(items) => {
            let lines = items
                .iter()
                .filter_map(extract_text_from_node)
                .collect::<Vec<_>>();
            clean_text(Some(lines.join("\n")))
        }
        Value::Object(map) => {
            for key in ["text", "recText", "transcription", "words", "label"] {
                if let Some(text) = map.get(key).and_then(extract_text_from_node) {
                    return Some(text);
                }
            }

            let lines = map
                .values()
                .filter_map(extract_text_from_node)
                .collect::<Vec<_>>();
            clean_text(Some(lines.join("\n")))
        }
        _ => None,
    }
}

fn clean_text(text: Option<String>) -> Option<String> {
    let text = text?;
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_text, health_url_from_endpoint};
    use serde_json::json;

    #[test]
    fn extracts_plain_text_response() {
        let value = json!({ "text": " hello\nworld " });

        assert_eq!(extract_text(&value), Some("hello\nworld".to_string()));
    }

    #[test]
    fn extracts_paddle_style_nested_lines() {
        let value = json!({
            "result": [
                { "recText": "first line" },
                { "recText": "second line" }
            ]
        });

        assert_eq!(
            extract_text(&value),
            Some("first line\nsecond line".to_string())
        );
    }

    #[test]
    fn builds_health_url_from_ocr_endpoint() {
        assert_eq!(
            health_url_from_endpoint("http://127.0.0.1:8866/ocr").unwrap(),
            "http://127.0.0.1:8866/health"
        );
        assert_eq!(
            health_url_from_endpoint("https://example.com/api/ocr?token=x").unwrap(),
            "https://example.com/health"
        );
    }
}
