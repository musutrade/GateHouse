"""Core cancellation fixture: retain a leader after the shared adapter finishes."""

import runpy
import sys
import time


if __name__ == "__main__":
    # Reuse the unchanged protocol fixture; only process lifetime is Core-local.
    adapter = runpy.run_path(sys.argv[1])
    status = adapter["main"]()
    sys.stdout.flush()
    time.sleep(1.5)
    raise SystemExit(status)
