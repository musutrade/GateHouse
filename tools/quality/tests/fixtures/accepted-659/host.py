#!/usr/bin/python3 -I
"""Install the reviewed package at /opt/gatehouse/trusted_gate, root owned."""
import os
from pathlib import Path
import stat
import sys

root = Path(__file__).resolve().parent
for path in (root, *root.parents):
    info = path.lstat()
    if info.st_uid != 0 or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
        raise SystemExit('host package must be installed in a protected root-owned path')
if os.geteuid() != 0:
    raise SystemExit('host controller requires its protected system identity')
sys.path.insert(0, str(root.parent))
from trusted_gate.cli import main

raise SystemExit(main())
