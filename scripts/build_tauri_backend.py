import os
import platform
import subprocess
import sys
from pathlib import Path

# Файлы данных, которые PyInstaller сам не найдёт: он видит только импорты.
# Формат `--add-data` — "источник<разделитель>каталог назначения", разделитель
# зависит от платформы (`;` на Windows, `:` на остальных).
DATA_FILES = [
    # Словарь строк веб-панели: `mkdsc/i18n/lexicon_web.py` читает его рядом с
    # собой. Без этого frozen-бэкенд падает на импорте с FileNotFoundError.
    ("mkdsc/i18n/lexicon_web.json", "mkdsc/i18n"),
]


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    dist_dir = root / "bin"
    work_dir = root / "build" / "pyinstaller"
    name = "mkdsc-backend"

    dist_dir.mkdir(parents=True, exist_ok=True)
    work_dir.mkdir(parents=True, exist_ok=True)

    data_args = []
    for source, destination in DATA_FILES:
        data_args.extend(["--add-data", f"{root / source}{os.pathsep}{destination}"])

    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "--noconfirm",
        "--clean",
        "--onefile",
        "--name",
        name,
        "--distpath",
        str(dist_dir),
        "--workpath",
        str(work_dir),
        "--specpath",
        str(work_dir),
        "--paths",
        str(root),
        *data_args,
        str(root / "tauri_backend.py"),
    ]

    print("Running:", " ".join(cmd))
    subprocess.run(cmd, check=True)

    exe_name = f"{name}.exe" if platform.system() == "Windows" else name
    output = dist_dir / exe_name
    if not output.exists():
        raise FileNotFoundError(f"Backend binary not found: {output}")

    print(f"Backend built: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
