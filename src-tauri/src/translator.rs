use crate::local_dictionary::FreeDictionarySupplement;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info};

#[derive(Debug, Clone, Default)]
pub struct TranslationContent {
    pub translated_text: String,
    pub phonetic: Option<String>,
    pub us_phonetic: Option<String>,
    pub uk_phonetic: Option<String>,
    pub audio_url: Option<String>,
    pub explains: Vec<String>,
    pub examples: Vec<String>,
    pub synonyms: Vec<String>,
    pub word_type: Option<String>,
}

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

fn http_client() -> &'static reqwest::Client {
    static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP_CLIENT.get_or_init(reqwest::Client::new)
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

    let response = http_client()
        .post("https://openapi.youdao.com/api")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_data)
        .send()
        .await
        .map_err(|e| {
            let err_msg = format!("有道翻译 API 请求失败: {}", e);
            error!("{}", err_msg);
            err_msg
        })?;

    let result: YoudaoResponse = response.json().await.map_err(|e| {
        let err_msg = format!("有道翻译 API 解析响应失败: {}", e);
        error!("{}", err_msg);
        err_msg
    })?;

    debug!("有道翻译 API 响应: {:?}", result);

    if result.error_code != "0" {
        let err_msg = format!("翻译失败，错误码: {}", result.error_code);
        error!("{}", err_msg);
        return Err(err_msg);
    }

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

pub async fn fetch_free_dictionary_supplement(
    word: &str,
) -> Result<Option<FreeDictionarySupplement>, String> {
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);

    debug!("查询 Free Dictionary API: {}", url);

    let response = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            let err_msg = format!("Free Dictionary API 请求失败: {}", e);
            error!("{}", err_msg);
            err_msg
        })?;

    if !response.status().is_success() {
        debug!("Free Dictionary API 未找到该单词");
        return Ok(None);
    }

    let results: Vec<FreeDictionaryResponse> = response
        .json()
        .await
        .map_err(|e| {
            let err_msg = format!("Free Dictionary API 解析响应失败: {}", e);
            error!("{}", err_msg);
            err_msg
        })?;

    debug!("Free Dictionary API 响应: {:?}", results);

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

    debug!(
        "解析词典信息 - 音标: {:?}, 音频: {:?}, 释义数量: {}, 例句数量: {}, 近义词数量: {}",
        phonetic,
        audio_url,
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
