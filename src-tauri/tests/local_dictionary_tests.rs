use rusqlite::Connection;
use translation_tool_lib::local_dictionary::{
    lookup_word_in_connection, merge_free_dictionary_supplement, FreeDictionarySupplement,
    OfflineDictionaryEntry,
};

fn base_entry() -> OfflineDictionaryEntry {
    OfflineDictionaryEntry {
        word: "easily".to_string(),
        translated_text: "容易地, 轻易地, 流利地".to_string(),
        phonetic: Some("'i:zili".to_string()),
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains: vec![
            "r. with ease (`easy' is sometimes used informally for `easily')".to_string(),
            "r. without question".to_string(),
        ],
        examples: vec![
            "she was easily excited".to_string(),
            "he won easily".to_string(),
        ],
        synonyms: vec!["well".to_string()],
        word_type: Some("adv.".to_string()),
    }
}

#[test]
fn merge_keeps_local_translation_and_adds_missing_remote_fields() {
    let merged = merge_free_dictionary_supplement(
        base_entry(),
        Some(FreeDictionarySupplement {
            phonetic: Some("/ˈiː.zə.liː/".to_string()),
            audio_url: Some(
                "https://api.dictionaryapi.dev/media/pronunciations/en/easily-us.mp3".to_string(),
            ),
            explains: vec![
                "Comfortably, without discomfort or anxiety.".to_string(),
                "Without difficulty.".to_string(),
            ],
            examples: vec![
                "Individuals without a family network are easily controlled.".to_string(),
                "he won easily".to_string(),
            ],
            synonyms: vec!["easy".to_string(), "well".to_string()],
        }),
    );

    assert_eq!(merged.translated_text, "容易地, 轻易地, 流利地");
    assert_eq!(
        merged.audio_url.as_deref(),
        Some("https://api.dictionaryapi.dev/media/pronunciations/en/easily-us.mp3")
    );
    assert_eq!(merged.phonetic.as_deref(), Some("'i:zili"));
    assert!(merged.synonyms.iter().any(|item| item == "well"));
    assert!(merged.synonyms.iter().any(|item| item == "easy"));
    assert_eq!(
        merged
            .examples
            .iter()
            .filter(|item| item.as_str() == "he won easily")
            .count(),
        1
    );
}

#[test]
fn merge_without_remote_data_returns_local_entry_unchanged() {
    let entry = base_entry();
    let merged = merge_free_dictionary_supplement(entry.clone(), None);

    assert_eq!(merged.translated_text, entry.translated_text);
    assert_eq!(merged.phonetic, entry.phonetic);
    assert_eq!(merged.examples, entry.examples);
    assert_eq!(merged.synonyms, entry.synonyms);
    assert_eq!(merged.explains, entry.explains);
}

#[test]
fn lookup_reads_local_dictionary_tables_and_merges_wordnet_rows() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "
            CREATE TABLE ecdict_entries (
                word TEXT PRIMARY KEY,
                phonetic TEXT,
                definition TEXT,
                translation TEXT,
                pos TEXT,
                exchange TEXT,
                tag TEXT
            );
            CREATE TABLE wordnet_synonyms (
                word TEXT NOT NULL,
                synonym TEXT NOT NULL,
                PRIMARY KEY (word, synonym)
            );
            CREATE TABLE wordnet_glosses (
                word TEXT NOT NULL,
                gloss TEXT NOT NULL,
                PRIMARY KEY (word, gloss)
            );
            CREATE TABLE wordnet_examples (
                word TEXT NOT NULL,
                example TEXT NOT NULL,
                PRIMARY KEY (word, example)
            );
            ",
        )
        .unwrap();

    connection
        .execute(
            "INSERT INTO ecdict_entries (word, phonetic, definition, translation, pos, exchange, tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                "easily",
                "'i:zili",
                "r. with ease\nr. without question",
                "adv. 容易地, 轻易地, 流利地",
                "",
                "",
                "cet4",
            ),
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO wordnet_synonyms (word, synonym) VALUES (?1, ?2)",
            ("easily", "well"),
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO wordnet_glosses (word, gloss) VALUES (?1, ?2)",
            ("easily", "indicating high probability"),
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO wordnet_examples (word, example) VALUES (?1, ?2)",
            ("easily", "he won easily"),
        )
        .unwrap();

    let result = lookup_word_in_connection(&connection, "easily")
        .unwrap()
        .expect("entry should exist");

    assert_eq!(result.translated_text, "容易地, 轻易地, 流利地");
    assert_eq!(result.word_type.as_deref(), Some("adv."));
    assert!(result.explains.iter().any(|item| item == "r. with ease"));
    assert!(result
        .explains
        .iter()
        .any(|item| item == "indicating high probability"));
    assert_eq!(result.examples, vec!["he won easily".to_string()]);
    assert_eq!(result.synonyms, vec!["well".to_string()]);
}
