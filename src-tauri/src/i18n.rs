//! Строки интерфейса — Rust-половина `GET /api/i18n`.
//!
//! Словарь не дублируется: и Python (`mkdsc/i18n/lexicon_web.py`), и Rust
//! читают один и тот же `mkdsc/i18n/lexicon_web.json`. Пока живы обе панели,
//! два независимых словаря разъехались бы на первой же правке, а половина
//! строк приходит в UI именно отсюда.
//!
//! Файл встраивается в бинарь на компиляции (`include_str!`), поэтому на диске
//! рядом с приложением ему быть не нужно.

use std::sync::OnceLock;

use serde_json::{Map, Value, json};

/// Словарь как есть — тот же файл, что читает Python.
const LEXICON_JSON: &str = include_str!("../../mkdsc/i18n/lexicon_web.json");

/// Языки веб-панели — ключи `lexicon_web.json`.
///
/// Список объявлен здесь, а не выводится из словаря: он уезжает в
/// `GET /api/config` и определяет содержимое переключателя языка, то есть это
/// контракт с интерфейсом. За тем, чтобы он не разъехался со словарём, следит
/// тест `languages_match_python_lexicon`.
pub const LANGUAGES: [&str; 2] = ["en", "ru"];

/// Язык по умолчанию: и запасной словарь, и ответ на незнакомый `lang`.
const FALLBACK: &str = "en";

/// Разобранный словарь. Разбор один на процесс — файл на 500 строк, а
/// `GET /api/i18n` дёргается на каждом переключении языка.
fn lexicon() -> &'static Map<String, Value> {
    static LEXICON: OnceLock<Map<String, Value>> = OnceLock::new();
    LEXICON.get_or_init(|| match serde_json::from_str(LEXICON_JSON) {
        Ok(Value::Object(map)) => map,
        // Файл встроен на компиляции и проверяется тестом `lexicon_parses`:
        // сюда можно попасть только сломав сам JSON в репозитории.
        _ => panic!("lexicon_web.json не разобрался как объект"),
    })
}

fn language_map(language: &str) -> Option<&'static Map<String, Value>> {
    lexicon().get(language)?.as_object()
}

/// Тело ответа `GET /api/i18n?lang=xx` — `{language, strings}`.
///
/// `strings` — английский словарь, перекрытый локальным: перевод может отстать
/// от английского, и без слияния недостающий ключ приехал бы в UI пустым.
/// Незнакомый язык отдаёт английский, а не ошибку, — так же, как Python:
/// интерфейс с битым `language` в конфиге должен открываться.
pub fn payload(language: &str) -> Value {
    json!({
        "language": language,
        "strings": strings(language),
    })
}

/// Слитый словарь для языка.
fn strings(language: &str) -> Value {
    let base = language_map(FALLBACK);
    let localized = language_map(language);

    match (base, localized) {
        // Английский отдаётся как есть — слияние с самим собой ничего не меняет.
        (Some(base), _) if language == FALLBACK => Value::Object(base.clone()),
        (Some(base), Some(localized)) => {
            let mut merged = base.clone();
            // `insert` в `serde_json` с `preserve_order` сохраняет позицию уже
            // существующего ключа — порядок получается такой же, как у
            // `{**base, **localized}` в Python.
            for (key, value) in localized {
                merged.insert(key.clone(), value.clone());
            }
            Value::Object(merged)
        }
        (Some(base), None) => Value::Object(base.clone()),
        (None, _) => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexicon_parses() {
        assert!(lexicon().len() >= 2, "языков в словаре меньше двух");
        for language in LANGUAGES {
            let map = language_map(language).expect("язык на месте");
            assert!(!map.is_empty(), "словарь {language} пуст");
        }
    }

    /// Список языков обязан совпадать с ключами словаря: он уезжает в
    /// `GET /api/config` и определяет содержимое переключателя языка.
    ///
    /// Тест с вехи 2. Тогда он разбирал `lexicon_web.py`, теперь читает JSON с
    /// диска — тот самый файл, что читает Python: сравнение с встроенной копией
    /// доказывало бы только то, что `include_str!` работает.
    #[test]
    fn languages_match_python_lexicon() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri лежит в корне репозитория")
            .join("mkdsc")
            .join("i18n")
            .join("lexicon_web.json");
        let source = std::fs::read_to_string(&path).expect("lexicon_web.json на месте");
        let parsed: Map<String, Value> = serde_json::from_str(&source).expect("это объект");

        let languages: Vec<&str> = parsed.keys().map(String::as_str).collect();
        assert_eq!(languages, LANGUAGES.to_vec());
    }

    #[test]
    fn english_is_returned_as_is() {
        let payload = payload("en");
        assert_eq!(payload["language"], json!("en"));
        assert_eq!(payload["strings"]["title"], json!("MK DroidScreenCast"));
        assert_eq!(
            payload["strings"].as_object().map(Map::len),
            language_map("en").map(Map::len)
        );
    }

    #[test]
    fn localized_overrides_english() {
        let strings = payload("ru")["strings"].clone();
        let english = language_map("en").expect("английский на месте");

        // Перекрытый ключ — русский, набор ключей — английский плюс лишние
        // русские (сейчас их нет, но контракт именно такой).
        assert_eq!(strings["language_label"], json!("Язык"));
        assert!(strings.as_object().expect("объект").len() >= english.len());
        for key in english.keys() {
            assert!(strings.get(key).is_some(), "ключ {key} потерялся");
        }
    }

    /// Незнакомый язык — английский словарь, а не ошибка и не пустота.
    #[test]
    fn unknown_language_falls_back_to_english() {
        let unknown = payload("de");
        assert_eq!(unknown["language"], json!("de"));
        assert_eq!(unknown["strings"], payload("en")["strings"]);

        // Пустой `lang` приходит из `?lang=` — тоже английский.
        assert_eq!(payload("")["strings"], payload("en")["strings"]);
    }
}
