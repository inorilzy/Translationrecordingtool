use crate::{local_dictionary::FreeDictionarySupplement, translation_domain::TranslationContent};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info};

// 有道翻译 API 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct YoudaoResponse {
    #[serde(rename = "errorCode")]
    pub error_code: String,
    pub translation: Option<Vec<String>>,
    pub basic: Option<YoudaoBasic>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YoudaoBasic {
    pub phonetic: Option<String>,
    pub explains: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MicrosoftTranslatorItem {
    translations: Vec<MicrosoftTranslationItem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MicrosoftTranslationItem {
    text: String,
}

#[derive(Debug, Serialize)]
struct MicrosoftTranslatorRequest<'a> {
    #[serde(rename = "Text")]
    text: &'a str,
}

// Free Dictionary API 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct FreeDictionaryResponse {
    pub word: Option<String>,
    pub phonetics: Option<Vec<Phonetic>>,
    pub meanings: Option<Vec<Meaning>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Phonetic {
    pub text: Option<String>,
    pub audio: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meaning {
    #[serde(rename = "partOfSpeech")]
    pub part_of_speech: Option<String>,
    pub definitions: Option<Vec<Definition>>,
    pub synonyms: Option<Vec<String>>,
    pub antonyms: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Definition {
    pub definition: Option<String>,
    pub example: Option<String>,
    pub synonyms: Option<Vec<String>>,
    pub antonyms: Option<Vec<String>>,
}

// Only explicitly recognized provider codes may enter logs or propagated errors.
// Unknown strings can contain echoed credentials, input, or forged log lines.
fn safe_youdao_error_code(code: &str) -> &str {
    match code {
        "0" | "101" | "102" | "103" | "104" | "105" | "106" | "107" | "108" | "109"
        | "110" | "111" | "112" | "113" | "201" | "202" | "203" | "301" | "302"
        | "303" | "304" | "401" | "411" | "412" => code,
        _ => "unknown",
    }
}

fn validate_youdao_response(result: &YoudaoResponse) -> Result<(), String> {
    let code = safe_youdao_error_code(&result.error_code);
    debug!(
        "有道翻译 API 响应: 错误码 {}, 译文条数 {}",
        code,
        result.translation.as_ref().map_or(0, Vec::len)
    );
    if result.error_code != "0" {
        let message = format!("翻译失败，错误码: {}", code);
        error!("{}", message);
        return Err(message);
    }
    Ok(())
}

#[cfg(test)]
mod youdao_log_tests {
    use super::*;

    #[test]
    fn response_logs_allow_only_known_codes_and_never_response_content() {
        for (raw_code, safe_code) in [
            ("SYNTHETIC_ERROR_CODE\nforged log", "unknown"),
            ("401 SYNTHETIC_ERROR_CODE", "unknown"),
            ("999999", "unknown"),
            ("401", "401"),
            ("0", "0"),
        ] {
            let response: YoudaoResponse = serde_json::from_value(serde_json::json!({
                "errorCode": raw_code,
                "translation": ["SYNTHETIC_TRANSLATION"],
                "basic": {"phonetic": "SYNTHETIC_PHONETIC", "explains": ["SYNTHETIC_EXPLAINS"]}
            })).unwrap();
            let logs = crate::logger::capture_logs_for_test(|| {
                let result = validate_youdao_response(&response);
                if raw_code == "0" {
                    assert!(result.is_ok());
                } else {
                    assert_eq!(result.unwrap_err(), format!("翻译失败，错误码: {}", safe_code));
                }
            });
            assert!(logs.contains(&format!("有道翻译 API 响应: 错误码 {}, 译文条数 1", safe_code)));
            if raw_code != "0" {
                assert!(logs.contains(&format!("翻译失败，错误码: {}", safe_code)));
            }
            for marker in ["SYNTHETIC_ERROR_CODE", "forged log", "SYNTHETIC_TRANSLATION", "SYNTHETIC_PHONETIC", "SYNTHETIC_EXPLAINS", "999999"] {
                assert!(!logs.contains(marker));
            }
        }
    }
}

pub async fn translate_text(
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<TranslationContent, String> {
    let salt = uuid::Uuid::new_v4().to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let input = if text.len() > 20 {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let start: String = chars.iter().take(10).collect();
        let end: String = chars.iter().skip(len - 10).collect();
        format!("{}{}{}", start, len, end)
    } else {
        text.to_string()
    };

    let sign_str = format!("{}{}{}{}{}", app_key, input, salt, timestamp, app_secret);
    let mut hasher = Sha256::new();
    hasher.update(sign_str.as_bytes());
    let sign = format!("{:x}", hasher.finalize());

    let form_data = format!(
        "q={}&from=auto&to=zh-CHS&appKey={}&salt={}&sign={}&signType=v3&curtime={}",
        urlencoding::encode(text),
        urlencoding::encode(app_key),
        urlencoding::encode(&salt),
        urlencoding::encode(&sign),
        urlencoding::encode(&timestamp)
    );

    info!("调用有道翻译 API");

    let response = crate::http_client::shared_client()
        .post("https://openapi.youdao.com/api")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_data)
        .send()
        .await
        .map_err(|e| {
            let err_msg = provider_http_error("有道翻译 API", "请求失败", e);
            error!("{}", err_msg);
            err_msg
        })?;

    let result: YoudaoResponse = response.json().await.map_err(|e| {
        let err_msg = provider_http_error("有道翻译 API", "解析响应失败", e);
        error!("{}", err_msg);
        err_msg
    })?;

    validate_youdao_response(&result)?;

    let translated_text = result
        .translation
        .and_then(|items| items.first().cloned())
        .ok_or("未获取到翻译结果".to_string())?;

    let mut explains = Vec::new();
    let mut phonetic = None;

    if let Some(basic) = result.basic {
        phonetic = basic.phonetic;
        if let Some(items) = basic.explains {
            explains = items;
        }
    }

    Ok(TranslationContent {
        translated_text,
        phonetic,
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains,
        examples: Vec::new(),
        synonyms: Vec::new(),
        word_type: None,
    })
}

pub async fn translate_with_microsoft(
    text: &str,
    key: &str,
    region: &str,
) -> Result<TranslationContent, String> {
    if key.trim().is_empty() {
        return Err("使用微软翻译需要配置 Microsoft Translator Key".to_string());
    }

    info!("调用微软翻译 API");

    let mut request = crate::http_client::shared_client()
        .post("https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&to=zh-Hans")
        .header("Content-Type", "application/json")
        .header("Ocp-Apim-Subscription-Key", key)
        .json(&vec![MicrosoftTranslatorRequest { text }]);

    let region = region.trim();
    if !region.is_empty() {
        request = request.header("Ocp-Apim-Subscription-Region", region);
    }

    let response = request.send().await.map_err(|e| {
        let err_msg = provider_http_error("微软翻译 API", "请求失败", e);
        error!("{}", err_msg);
        err_msg
    })?;

    if !response.status().is_success() {
        // 工单03：错误响应体可能回显输入或账户信息，只保留 HTTP 状态码。
        let err_msg = microsoft_status_error(response.status());
        error!("{}", err_msg);
        return Err(err_msg);
    }

    let result: Vec<MicrosoftTranslatorItem> = response.json().await.map_err(|e| {
        let err_msg = provider_http_error("微软翻译 API", "解析响应失败", e);
        error!("{}", err_msg);
        err_msg
    })?;

    let translated_text = result
        .first()
        .and_then(|item| item.translations.first())
        .map(|item| item.text.clone())
        .ok_or_else(|| "未获取到微软翻译结果".to_string())?;

    Ok(TranslationContent {
        translated_text,
        phonetic: None,
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains: Vec::new(),
        examples: Vec::new(),
        synonyms: Vec::new(),
        word_type: None,
    })
}

#[derive(Debug, Deserialize)]
struct GoogleTranslateResponse {
    data: Option<GoogleTranslateData>,
    error: Option<GoogleApiError>,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslateData {
    translations: Option<Vec<GoogleTranslationItem>>,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslationItem {
    #[serde(rename = "translatedText")]
    translated_text: Option<String>,
}

// Do not retain provider error details: they may echo credentials or user input.
#[derive(Debug, Deserialize)]
struct GoogleApiError {}

impl GoogleTranslateResponse {
    fn service_error(&self, status: reqwest::StatusCode) -> Option<String> {
        if self.error.is_some() || !status.is_success() {
            Some(format!(
                "Google 翻译 API 返回错误: HTTP {}",
                status.as_u16()
            ))
        } else {
            None
        }
    }
}

fn google_http_error(stage: &str, error: reqwest::Error) -> String {
    format!("Google 翻译 API {}: {}", stage, error.without_url())
}

/// 通用 provider HTTP 错误格式化：去除 URL（可能含查询单词或凭据），只保留阶段与错误类别。
/// reqwest::Error 的 Display 不展开 source，因此不会带出响应体片段。
fn provider_http_error(provider: &str, stage: &str, error: reqwest::Error) -> String {
    format!("{} {}: {}", provider, stage, error.without_url())
}

fn microsoft_status_error(status: reqwest::StatusCode) -> String {
    format!("微软翻译 API 返回错误: HTTP {}", status.as_u16())
}

pub async fn translate_with_google(
    text: &str,
    api_key: &str,
) -> Result<TranslationContent, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("使用 Google 翻译需要配置 API Key".to_string());
    }

    info!("调用 Google Cloud Translation API");

    let url = format!(
        "https://translation.googleapis.com/language/translate/v2?key={}",
        urlencoding::encode(api_key)
    );

    let body = serde_json::json!({
        "q": text,
        "target": "zh-CN",
        "format": "text"
    });

    let response = crate::http_client::shared_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let err_msg = google_http_error("请求失败", e);
            error!("{}", err_msg);
            err_msg
        })?;

    let status = response.status();
    let result: GoogleTranslateResponse = response.json().await.map_err(|e| {
        let err_msg = google_http_error("解析响应失败", e);
        error!("{}", err_msg);
        err_msg
    })?;

    if let Some(err_msg) = result.service_error(status) {
        error!("{}", err_msg);
        return Err(err_msg);
    }

    let translated_text = result
        .data
        .and_then(|data| data.translations)
        .and_then(|items| items.into_iter().next())
        .and_then(|item| item.translated_text)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| "未获取到 Google 翻译结果".to_string())?;

    Ok(TranslationContent {
        translated_text,
        phonetic: None,
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains: Vec::new(),
        examples: Vec::new(),
        synonyms: Vec::new(),
        word_type: None,
    })
}

pub async fn fetch_free_dictionary_supplement(
    word: &str,
) -> Result<Option<FreeDictionarySupplement>, String> {
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);

    // 工单03：URL 路径包含查询单词，日志只记录字符数。
    debug!("查询 Free Dictionary API，单词字符数: {}", word.chars().count());

    let response = crate::http_client::shared_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            let err_msg = provider_http_error("Free Dictionary API", "请求失败", e);
            error!("{}", err_msg);
            err_msg
        })?;

    if !response.status().is_success() {
        debug!(
            "Free Dictionary API 未找到该单词: HTTP {}",
            response.status().as_u16()
        );
        return Ok(None);
    }

    let results: Vec<FreeDictionaryResponse> = response.json().await.map_err(|e| {
        let err_msg = provider_http_error("Free Dictionary API", "解析响应失败", e);
        error!("{}", err_msg);
        err_msg
    })?;

    debug!("Free Dictionary API 响应条目数: {}", results.len());

    let Some(first) = results.first() else {
        return Ok(None);
    };

    let mut phonetic = None;
    let mut audio_url = None;

    if let Some(phonetics) = &first.phonetics {
        for item in phonetics {
            if let Some(audio) = &item.audio {
                if !audio.is_empty() {
                    audio_url = Some(audio.clone());
                    if let Some(text) = &item.text {
                        phonetic = Some(text.clone());
                    }
                    break;
                }
            }
        }

        if phonetic.is_none() {
            phonetic = phonetics.iter().find_map(|item| item.text.clone());
        }
    }

    let mut explains = Vec::new();
    let mut examples = Vec::new();
    let mut synonyms = Vec::new();

    if let Some(meanings) = &first.meanings {
        for meaning in meanings.iter().take(6) {
            if let Some(items) = &meaning.synonyms {
                for item in items {
                    push_unique(&mut synonyms, item.clone());
                }
            }

            let part_of_speech = meaning.part_of_speech.as_deref().unwrap_or("释义");
            if let Some(definitions) = &meaning.definitions {
                for definition in definitions.iter().take(4) {
                    if let Some(text) = &definition.definition {
                        push_unique(&mut explains, format!("{}. {}", part_of_speech, text));
                    }
                    if let Some(example) = &definition.example {
                        push_unique(&mut examples, example.clone());
                    }
                    if let Some(items) = &definition.synonyms {
                        for item in items {
                            push_unique(&mut synonyms, item.clone());
                        }
                    }
                }
            }
        }
    }

    // 音标/音频 URL 属于词条内容，只记录是否存在与数量。
    debug!(
        "解析词典信息 - 有音标: {}, 有音频: {}, 释义数量: {}, 例句数量: {}, 近义词数量: {}",
        phonetic.is_some(),
        audio_url.is_some(),
        explains.len(),
        examples.len(),
        synonyms.len()
    );

    Ok(Some(FreeDictionarySupplement {
        phonetic,
        audio_url,
        explains,
        examples,
        synonyms,
    }))
}

fn push_unique(items: &mut Vec<String>, value: String) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }

    let exists = items
        .iter()
        .any(|item| item.trim().eq_ignore_ascii_case(trimmed));
    if !exists {
        items.push(trimmed.to_string());
    }
}

#[cfg(test)]
mod google_error_tests {
    use super::*;

    const FAKE_KEY: &str = "audit-only key+/=&?";

    fn assert_no_credential(message: &str) {
        assert!(!message.contains(FAKE_KEY));
        assert!(!message.contains(urlencoding::encode(FAKE_KEY).as_ref()));
        assert!(!message.contains("?key="));
    }

    fn error_with_credential_url() -> reqwest::Error {
        // Construct an error without sending a request or reading app settings.
        let url = reqwest::Url::parse(&format!(
            "https://translation.googleapis.com/language/translate/v2?key={}",
            urlencoding::encode(FAKE_KEY)
        ))
        .unwrap();
        reqwest::Client::new()
            .get("http://[")
            .build()
            .unwrap_err()
            .with_url(url)
    }

    #[test]
    fn google_request_error_removes_credential_url() {
        let error = error_with_credential_url();
        assert!(error.to_string().contains("?key="));
        let message = google_http_error("请求失败", error);
        assert_no_credential(&message);
        assert!(message.contains("请求失败"));
    }

    #[test]
    fn google_decode_error_formatter_removes_credential_url() {
        // Even if a reqwest version attaches a URL to a decode error, the same
        // production formatter must remove it before logging or returning it.
        let message = google_http_error("解析响应失败", error_with_credential_url());
        assert_no_credential(&message);
        assert!(message.contains("解析响应失败"));
    }

    #[test]
    fn google_service_error_does_not_echo_provider_details() {
        let response: GoogleTranslateResponse = serde_json::from_value(serde_json::json!({
            "error": {
                "message": format!("bad key: {} / ?key={}", FAKE_KEY, urlencoding::encode(FAKE_KEY)),
                "details": [{"key": FAKE_KEY}]
            }
        }))
        .unwrap();
        for status in [reqwest::StatusCode::BAD_REQUEST, reqwest::StatusCode::OK] {
            let message = response.service_error(status).unwrap();
            assert_no_credential(&message);
            assert_eq!(
                message,
                format!("Google 翻译 API 返回错误: HTTP {}", status.as_u16())
            );
        }
    }

    #[test]
    fn google_service_error_handles_http_failure_without_error_body() {
        let response: GoogleTranslateResponse = serde_json::from_str("{}").unwrap();
        assert_eq!(
            response
                .service_error(reqwest::StatusCode::BAD_GATEWAY)
                .as_deref(),
            Some("Google 翻译 API 返回错误: HTTP 502")
        );
        assert!(response.service_error(reqwest::StatusCode::OK).is_none());
    }
}

#[cfg(test)]
mod log_privacy_tests {
    use super::*;

    const WORD_MARKER: &str = "syntheticwordmarkerzx";

    #[test]
    fn provider_http_error_removes_url_with_query_word() {
        // 构造携带含单词路径 URL 的 reqwest 错误，不发送任何请求。
        let url = reqwest::Url::parse(&format!(
            "https://api.dictionaryapi.dev/api/v2/entries/en/{WORD_MARKER}"
        ))
        .unwrap();
        let error = reqwest::Client::new()
            .get("http://[")
            .build()
            .unwrap_err()
            .with_url(url);
        assert!(error.to_string().contains(WORD_MARKER), "前置条件：原始错误带 URL");
        let message = provider_http_error("Free Dictionary API", "请求失败", error);
        assert!(!message.contains(WORD_MARKER), "{message}");
        assert!(!message.contains("dictionaryapi.dev"), "{message}");
        assert!(message.starts_with("Free Dictionary API 请求失败"));
    }

    #[test]
    fn microsoft_status_error_keeps_only_status_code() {
        assert_eq!(
            microsoft_status_error(reqwest::StatusCode::UNAUTHORIZED),
            "微软翻译 API 返回错误: HTTP 401"
        );
    }

    /// 静态守卫：生产代码不得再以 Debug 格式整体输出 provider 响应结构。
    #[test]
    fn production_code_does_not_debug_print_whole_responses() {
        let source = include_str!("translator.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            concat!("响应: {:?}\", ", "result"),
            concat!("响应: {:?}\", ", "results"),
            concat!("API: {}\", ", "url"),
            concat!("response.", "text()"),
        ] {
            assert!(!production.contains(forbidden), "仍包含 {forbidden}");
        }
    }
}
