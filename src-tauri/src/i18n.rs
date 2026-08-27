//! Строки интерфейса — данные для `GET /api/i18n`.
//!
//! Словарь лежит рядом в `lexicon_web.json` и встраивается в бинарь на
//! компиляции (`include_str!`), поэтому на диске рядом с приложением ему быть
//! не нужно. JSON, а не литерал в коде: файл на 500 строк удобнее править и
//! сверять глазами — таким он и остался с тех пор, когда его читали ещё и из
//! Python.

use std::sync::OnceLock;

use serde_json::{Map, Value, json};

/// Словарь как есть.
const LEXICON_JSON: &str = include_str!("lexicon_web.json");

/// Языки интерфейса — ключи `lexicon_web.json`.
///
/// Список объявлен здесь, а не выводится из словаря: он уезжает в
/// `GET /api/config` и определяет содержимое переключателя языка, то есть это
/// контракт с интерфейсом. За тем, чтобы он не разъехался со словарём, следит
/// тест `lexicon_parses`.
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

    /// Встроенный словарь разбирается, и в нём есть ровно объявленные языки:
    /// список уезжает в `GET /api/config` и определяет переключатель языка.
    #[test]
    fn lexicon_parses() {
        let languages: Vec<&str> = lexicon().keys().map(String::as_str).collect();
        assert_eq!(languages, LANGUAGES.to_vec());

        for language in LANGUAGES {
            let map = language_map(language).expect("язык на месте");
            assert!(!map.is_empty(), "словарь {language} пуст");
        }
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
