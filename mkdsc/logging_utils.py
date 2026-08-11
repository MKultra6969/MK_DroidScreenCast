import logging
import time
from logging.handlers import RotatingFileHandler

from .paths import get_logs_dir

MAX_BYTES = 5 * 1024 * 1024
BACKUP_COUNT = 3
RETENTION_DAYS = 14


def _cleanup_old_logs(logs_dir, retention_days=RETENTION_DAYS):
    """Удаляет логи старше N дней.

    Раньше каждый запуск создавал новый web_YYYYMMDD_HHMMSS.log и ничего
    не чистило — каталог рос бесконечно.
    """
    cutoff = time.time() - retention_days * 86400
    for path in list(logs_dir.glob("*.log")) + list(logs_dir.glob("*.log.*")):
        try:
            if path.stat().st_mtime < cutoff:
                path.unlink()
        except OSError:
            continue


def init_logging(session_name, config=None):
    logs_dir = get_logs_dir(config)
    logs_dir.mkdir(parents=True, exist_ok=True)
    _cleanup_old_logs(logs_dir)

    log_path = logs_dir / f"{session_name}.log"

    logger = logging.getLogger(f"mkdsc.{session_name}")
    logger.setLevel(logging.INFO)
    logger.handlers.clear()

    formatter = logging.Formatter(
        fmt="%(asctime)s | %(levelname)s | %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    file_handler = RotatingFileHandler(
        log_path,
        maxBytes=MAX_BYTES,
        backupCount=BACKUP_COUNT,
        encoding="utf-8",
    )
    file_handler.setFormatter(formatter)
    logger.addHandler(file_handler)

    return logger, log_path
