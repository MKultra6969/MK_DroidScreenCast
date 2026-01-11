from .lexicon_cli import LEXICON_CLI
from .lexicon_web import LEXICON_WEB


def translate(lexicon, lang, key, **kwargs):
    lang_map = lexicon.get(lang, {})
    text = lang_map.get(key) or lexicon.get("en", {}).get(key) or key
    try:
        return text.format(**kwargs)
    except Exception:
        return text


def get_translator(lexicon, lang):
    return lambda key, **kwargs: translate(lexicon, lang, key, **kwargs)


def available_languages(lexicon):
    return list(lexicon.keys())
