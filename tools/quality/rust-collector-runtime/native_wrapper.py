"""Cargo wrapper using only the private interpreter and adapter."""
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
import rust_native_driver
sys.exit(rust_native_driver.wrapper(sys.argv[1:]))
