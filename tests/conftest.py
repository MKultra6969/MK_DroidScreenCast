import os
import sys
import tempfile
from pathlib import Path

# The app resolves paths at import time, so redirect them before mkdsc loads.
os.environ.setdefault("MKDSC_DATA_DIR", tempfile.mkdtemp(prefix="mkdsc-tests-"))

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
