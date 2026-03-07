use crate::local_dictionary::FreeDictionarySupplement;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub web: Option<Vec<WebTranslation>>,
}

// 有道词典 API 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct YoudaoDictResponse {
    #[serde(rename = "errorCode")]
    pub error_code: String,
    pub result: Option<DictResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DictResult {
    pub ec: Option<DictContent>,
    pub ce: Option<DictContent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DictContent {
    pub basic: Option<DictBasic>,
    pub web: Option<Vec<DictWebItem>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DictBasic {
    pub phonetic: Option<String>,
    #[serde(rename = "us-phonetic")]
    pub us_phonetic: Option<String>,
    #[serde(rename = "uk-phonetic")]
    pub uk_phonetic: Option<String>,
    #[serde(rename = "uk-speech")]
    pub uk_speech: Option<String>,
    #[serde(rename = "us-speech")]
    pub us_speech: Option<String>,
    pub explains: Option<Vec<DictExplain>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DictExplain {
    pub pos: Option<String>,
    pub trans: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DictWebItem {
    pub key: Option<String>,
    pub value: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YoudaoBasic {
    pub phonetic: Option<String>,
    #[serde(rename = "us-phonetic")]
    pub us_phonetic: Option<String>,
    #[serde(rename = "uk-phonetic")]
    pub uk_phonetic: Option<String>,
    pub explains: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebTranslation {
    pub key: Option<String>,
    pub value: Option<Vec<String>>,
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

    let client = reqwest::Client::new();

    let is_single_word =
        !text.contains(' ') && !text.contains(',') && !text.contains('.') && text.chars().all(|c| c.is_alphabetic());

    println!("翻译文本: {}, 是否为单词: {}", text, is_single_word);

    if is_single_word {
        println!("使用有道词典 API");

        let dict_type = if text.chars().all(|c| c.is_ascii_alphabetic()) {
            "ec"
        } else {
            "ce"
        };

        let form_data = format!(
            "q={}&langType=auto&appKey={}&dicts={}&salt={}&sign={}&signType=v3&curtime={}&docType=json",
            urlencoding::encode(text),
            urlencoding::encode(app_key),
            dict_type,
            urlencoding::encode(&salt),
            urlencoding::encode(&sign),
            urlencoding::encode(&timestamp)
        );

        let response = client
            .post("https://openapi.youdao.com/v2/dict")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_data)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        let result: YoudaoDictResponse = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        println!("有道词典 API 响应: {:?}", result);

        if result.error_code == "0" {
            let dict_content = result.result.as_ref().and_then(|r| r.ec.as_ref().or(r.ce.as_ref()));

            if let Some(content) = dict_content {
                let basic = content.basic.as_ref();

                let us_phonetic = basic.and_then(|b| b.us_phonetic.clone());
                let uk_phonetic = basic.and_then(|b| b.uk_phonetic.clone());
                let phonetic = basic.and_then(|b| b.phonetic.clone());
                let audio_url = basic.and_then(|b| b.us_speech.clone().or(b.uk_speech.clone()));

                let explains = basic
                    .and_then(|b| b.explains.as_ref())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| match (&item.pos, &item.trans) {
                                (Some(pos), Some(trans)) => Some(format!("{}. {}", pos, trans)),
                                (None, Some(trans)) => Some(trans.clone()),
                                _ => None,
                            })
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();

                let translated_text = explains
                    .first()
                    .map(|item| strip_part_of_speech(item))
                    .unwrap_or_else(|| text.to_string());

                println!(
                    "词典解析结果 - 美式音标: {:?}, 英式音标: {:?}, 音频: {:?}, 释义: {:?}",
                    us_phonetic, uk_phonetic, audio_url, explains
                );

                if us_phonetic.is_some() || !explains.is_empty() {
                    return Ok(TranslationContent {
                        translated_text,
                        phonetic,
                        us_phonetic,
                        uk_phonetic,
                        audio_url,
                        word_type: infer_word_type(&explains),
                        explains,
                        examples: Vec::new(),
                        synonyms: Vec::new(),
                    });
                }
            }
        }

        println!("有道词典 API 无结果，尝试 Free Dictionary API");

        match fetch_free_dictionary_supplement(text).await {
            Ok(Some(supplement)) => {
                println!(
                    "Free Dictionary 返回 - 音标: {:?}, 音频: {:?}, 释义数量: {}, 例句数量: {}, 近义词数量: {}",
                    supplement.phonetic,
                    supplement.audio_url,
                    supplement.explains.len(),
                    supplement.examples.len(),
                    supplement.synonyms.len()
                );

                println!("调用翻译 API 获取中文翻译");
                let translated_text = match get_youdao_translation(text, app_key, app_secret).await {
                    Ok(translated_text) => translated_text,
                    Err(_) => text.to_string(),
                };

                return Ok(TranslationContent {
                    translated_text,
                    phonetic: supplement.phonetic.clone(),
                    us_phonetic: supplement.phonetic.clone(),
                    uk_phonetic: None,
                    audio_url: supplement.audio_url,
                    word_type: infer_word_type(&supplement.explains),
                    explains: supplement.explains,
                    examples: supplement.examples,
                    synonyms: supplement.synonyms,
                });
            }
            Ok(None) => {}
            Err(e) => {
                println!("Free Dictionary API 失败: {}", e);
            }
        }
    }

    println!("使用有道翻译 API");
    match get_youdao_translation(text, app_key, app_secret).await {
        Ok(translated_text) => Ok(TranslationContent {
            translated_text,
            ..TranslationContent::default()
        }),
        Err(e) => Err(e),
    }
}

pub async fn fetch_free_dictionary_supplement(
    word: &str,
) -> Result<Option<FreeDictionarySupplement>, String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);

    println!("查询 Free Dictionary API: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        println!("Free Dictionary API 未找到该单词");
        return Ok(None);
    }

    let results: Vec<FreeDictionaryResponse> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    println!("Free Dictionary API 响应: {:?}", results);

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

    println!(
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

async fn get_youdao_translation(text: &str, app_key: &str, app_secret: &str) -> Result<String, String> {
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

    let client = reqwest::Client::new();
    let form_data = format!(
        "q={}&from=auto&to=zh-CHS&appKey={}&salt={}&sign={}&signType=v3&curtime={}",
        urlencoding::encode(text),
        urlencoding::encode(app_key),
        urlencoding::encode(&salt),
        urlencoding::encode(&sign),
        urlencoding::encode(&timestamp)
    );

    let response = client
        .post("https://openapi.youdao.com/api")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_data)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let result: YoudaoResponse = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    println!("有道翻译 API 响应: {:?}", result);

    if result.error_code != "0" {
        return Err(format!("翻译失败，错误码: {}", result.error_code));
    }

    result
        .translation
        .and_then(|items| items.first().cloned())
        .ok_or("未获取到翻译结果".to_string())
}

fn infer_word_type(explains: &[String]) -> Option<String> {
    explains.first().and_then(|item| {
        let (prefix, _) = item.split_once(". ")?;
        if prefix.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '/') {
            Some(format!("{}.", prefix))
        } else {
            None
        }
    })
}

fn strip_part_of_speech(text: &str) -> String {
    if let Some((prefix, rest)) = text.split_once(". ") {
        if prefix.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '/') {
            return rest.trim().to_string();
        }
    }

    text.trim().to_string()
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
