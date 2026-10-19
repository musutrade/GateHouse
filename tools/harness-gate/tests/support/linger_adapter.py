"""Core cancellation fixture: retain a leader after the shared adapter finishes."""

import runpy
import sys
import time
from pathlib import Path


if __name__ == "__main__":
    # Reuse the unchanged protocol fixture; only process lifetime is Core-local.
    adapter = runpy.run_path(sys.argv[1])
    status = adapter["main"]()
    sys.stdout.flush()
    # Signal only after main has closed the artifact and flushed its response.
    Path(sys.argv[2]).touch()
    time.sleep(1.5)
    raise SystemExit(status)
