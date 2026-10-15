"""Run the Linux design experiment in temporary storage; retain no evidence."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if sys.platform != "linux":
    raise SystemExit("This design experiment requires Linux; no cross-platform claim.")
source = Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix="gatehouse-runtime-design-") as scratch:
    root = Path(scratch)
    for path in source.glob("*.py"):
        if path.name != "run.py":
            shutil.copyfile(path, root / path.name)
    for name in ("check.py", "check_concurrency.py", "check_termination.py", "check_contract.py"):
        subprocess.run([sys.executable, "-B", str(root / name)], check=True, timeout=30,
                       cwd=root, env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
print("Design experiment passed; temporary reports removed. No production runtime invoked.")
