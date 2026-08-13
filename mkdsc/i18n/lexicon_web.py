"""Словарь строк веб-панели.

Сами строки лежат рядом в ``lexicon_web.json``, а не в этом файле: их читают
две стороны — Python (здесь) и Rust (``src-tauri/src/i18n.rs`` через
``include_str!``). Пока `GET /api/i18n` отдаёт и десктоп, и веб-панель,
источник правды должен быть один, иначе словари разъедутся при первой же
правке.

Модуль остаётся ради обратной совместимости импорта ``LEXICON_WEB``.
"""
import json
from pathlib import Path

# ``__file__`` работает и во frozen-сборке: PyInstaller разворачивает модуль в
# ``_MEIPASS/mkdsc/i18n/``, куда ``--add-data`` кладёт и сам JSON
# (см. ``scripts/build_tauri_backend.py``).
LEXICON_PATH = Path(__file__).resolve().parent / "lexicon_web.json"

LEXICON_WEB = json.loads(LEXICON_PATH.read_text(encoding="utf-8"))
