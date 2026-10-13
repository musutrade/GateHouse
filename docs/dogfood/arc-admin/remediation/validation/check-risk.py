import os
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, 'tools/quality')
import ci_quality
root = Path.cwd()
original = subprocess.run
def run(argv, *args, **kwargs):
    if argv[0] == 'git' and Path(kwargs.get('cwd', root)).resolve() == root:
        argv = ['git', '--git-dir=' + str(root / 'target/gh-207-git'), '--work-tree=' + str(root), *argv[1:]]
    return original(argv, *args, **kwargs)
subprocess.run = run
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
collector = ci_quality.Collector(root / 'target/quality/gh-207/risk', 'ed0270a82be8b699f6bcb7916e7a03462f0a52c6', head, 'gh-207-local')
collector.risk()
print('PASS: committed base/head function-risk comparison')
