"""Only the reviewed placeholder transition may bridge repositories."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'maintenance'))
import migration_baseline as bridge


class MigrationBaselineTests(unittest.TestCase):
    def test_ordinary_commits_never_fetch_or_override_base(self):
        with patch.object(bridge.subprocess, 'check_output', side_effect=AssertionError('git')):
            self.assertEqual(bridge.resolve('b'*40, 'c'*40, 'musutrade/GateHouse', True), 'b'*40)

    def test_invalid_or_missing_base_fails(self):
        for base in ('', '0'*40, 'HEAD^', 'abc'):
            with self.subTest(base=base), self.assertRaises(ValueError):
                bridge.resolve(base, 'c'*40, 'musutrade/GateHouse')

    def test_seed_in_another_repository_fails(self):
        with self.assertRaisesRegex(ValueError, 'restricted'):
            bridge.resolve(bridge.SEED, 'c'*40, 'musutrade/Other')

    def test_exact_committed_binding_is_required(self):
        good={'schema':'repository-migration/v1','seed_commit':bridge.SEED,'source_repository':'musutrade/Harness-Gate','destination_repository':'musutrade/GateHouse','source_commit':'a'*40}
        with patch.object(bridge.subprocess,'check_output',return_value=json.dumps(good).encode()) as command:
            self.assertEqual(bridge.resolve(bridge.SEED,'c'*40,'musutrade/GateHouse'),'a'*40)
            self.assertIn('c'*40+':.github/repository-migration.json',command.call_args.args[0])
        for key in good:
            bad=dict(good,**{key:'invalid'})
            with self.subTest(key=key), patch.object(bridge.subprocess,'check_output',return_value=json.dumps(bad).encode()), self.assertRaises(ValueError):
                bridge.resolve(bridge.SEED,'c'*40,'musutrade/GateHouse')

    def test_fetch_or_source_difference_failure_propagates(self):
        binding={'schema':'repository-migration/v1','seed_commit':bridge.SEED,'source_repository':'musutrade/Harness-Gate','destination_repository':'musutrade/GateHouse','source_commit':'a'*40}
        for fail_at in (0,1):
            calls=[None, None];calls[fail_at]=subprocess.CalledProcessError(1,['git'])
            with patch.object(bridge.subprocess,'check_output',side_effect=[json.dumps(binding).encode(),'a'*40+'\n']), patch.object(bridge.subprocess,'run',side_effect=calls), self.assertRaises(subprocess.CalledProcessError):
                bridge.resolve(bridge.SEED,'c'*40,'musutrade/GateHouse',True)
