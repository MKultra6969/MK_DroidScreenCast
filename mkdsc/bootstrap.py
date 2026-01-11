import hashlib
import os
import subprocess
import sys
from pathlib import Path

from .paths import BASE_DIR


def _in_project_venv(venv_dir: Path) -> bool:
    try:
        return Path(sys.prefix).resolve() == venv_dir.resolve()
    except FileNotFoundError:
        return False


def _build_venv_env(venv_dir: Path) -> dict:
    env = os.environ.copy()
    scripts_dir = venv_dir / ("Scripts" if os.name == "nt" else "bin")
    env["VIRTUAL_ENV"] = str(venv_dir)
    env["VIRTUAL_ENV_PROMPT"] = venv_dir.name
    env["PATH"] = f"{scripts_dir}{os.pathsep}{env.get('PATH', '')}"
    env.pop("PYTHONHOME", None)
    return env


def _venv_python():
    if os.name == "nt":
        return BASE_DIR / ".venv" / "Scripts" / "python.exe"
    return BASE_DIR / ".venv" / "bin" / "python"


def _hash_requirements(path):
    data = path.read_bytes()
    return hashlib.sha256(data).hexdigest()


def ensure_runtime():
    venv_dir = BASE_DIR / ".venv"
    python_path = _venv_python()

    if _in_project_venv(venv_dir):
        return

    if not python_path.exists():
        subprocess.run([sys.executable, "-m", "venv", str(venv_dir)], check=True)

    requirements_path = BASE_DIR / "requirements.txt"
    marker_path = venv_dir / "requirements.sha256"

    if requirements_path.exists():
        requirements_hash = _hash_requirements(requirements_path)
        marker_hash = marker_path.read_text(encoding="utf-8").strip() if marker_path.exists() else ""

        if requirements_hash != marker_hash:
            subprocess.run([str(python_path), "-m", "pip", "install", "-r", str(requirements_path)], check=True)
            marker_path.write_text(requirements_hash, encoding="utf-8")

    args = [str(python_path), str(BASE_DIR / "gateway.py")] + sys.argv[1:]
    result = subprocess.run(args, env=_build_venv_env(venv_dir))
    raise SystemExit(result.returncode)
