#!/usr/bin/env python3
"""Offline tests for release tag and main-CI eligibility policy."""

from __future__ import annotations

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).parents[1] / "release_policy.py"
SPEC = importlib.util.spec_from_file_location("release_policy", MODULE_PATH)
assert SPEC and SPEC.loader
policy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(policy)


class ReleasePolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="harness-gate-release-policy-")
        self.repo = Path(self.temp.name) / "repo"
        self.repo.mkdir()
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Harness Gate Test")
        self.git("config", "user.email", "test@example.invalid")
        self.write_manifest("0.3.3")
        self.git("add", "Cargo.toml")
        self.git("commit", "-m", "initial")
        self.commit = self.git("rev-parse", "HEAD")
        self.git("tag", "v0.3.3")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def git(self, *args: str) -> str:
        result = subprocess.run(
            ["git", *args],
            cwd=self.repo,
            check=True,
            capture_output=True,
            text=True,
        )
        return result.stdout.strip()

    def write_manifest(self, version: str) -> None:
        (self.repo / "Cargo.toml").write_text(
            f'[package]\nname = "fixture"\nversion = "{version}"\n',
            encoding="utf-8",
        )

    def test_exact_semver_tag_and_main_ancestry_pass(self) -> None:
        self.assertEqual(policy.version_from_tag("v0.3.3"), "0.3.3")
        self.assertEqual(
            policy.verify_git_state(self.repo, "v0.3.3", self.commit, "main"),
            self.commit,
        )

    def test_prerelease_semver_is_supported(self) -> None:
        self.assertEqual(policy.version_from_tag("v1.2.3-rc.1+build.7"), "1.2.3-rc.1+build.7")

    def test_collector_tag_is_explicit_and_preserves_git_checks(self) -> None:
        tag = "rust-collector-v0.3.3"
        self.assertEqual(policy.version_from_tag(tag, "rust-collector-v"), "0.3.3")
        for candidate, prefix in ((tag, "v"), ("v0.3.3", "rust-collector-v"),
                                  ("rust-collector-v01.2.3", "rust-collector-v")):
            with self.subTest(tag=candidate), self.assertRaises(policy.PolicyError):
                policy.version_from_tag(candidate, prefix)
        self.git("tag", tag)
        self.assertEqual(policy.verify_git_state(self.repo, tag, self.commit, "main"), self.commit)

    def test_collector_manifest_mismatch_fails_before_network(self) -> None:
        arguments = policy.parser().parse_args([
            "verify", "--tag", "rust-collector-v0.3.4", "--tag-prefix", "rust-collector-v",
            "--commit", self.commit, "--repository", "example/repository",
            "--repo", str(self.repo), "--manifest", str(self.repo / "Cargo.toml")])
        with self.assertRaisesRegex(policy.PolicyError, "does not match package version"):
            policy.verify(arguments)

    def test_non_semver_tag_is_rejected(self) -> None:
        for tag in ("0.3.3", "v1.2", "v01.2.3", "v1.2.3-01"):
            with self.subTest(tag=tag), self.assertRaises(policy.PolicyError):
                policy.version_from_tag(tag)

    def test_tag_must_resolve_to_requested_commit(self) -> None:
        (self.repo / "README.md").write_text("next\n", encoding="utf-8")
        self.git("add", "README.md")
        self.git("commit", "-m", "next")
        next_commit = self.git("rev-parse", "HEAD")
        with self.assertRaisesRegex(policy.PolicyError, "not requested commit"):
            policy.verify_git_state(self.repo, "v0.3.3", next_commit, "main")

    def test_tag_commit_must_be_reachable_from_main(self) -> None:
        self.git("switch", "-c", "feature")
        self.write_manifest("0.3.4")
        self.git("add", "Cargo.toml")
        self.git("commit", "-m", "feature")
        feature_commit = self.git("rev-parse", "HEAD")
        self.git("tag", "v0.3.4")
        with self.assertRaisesRegex(policy.PolicyError, "not reachable"):
            policy.verify_git_state(self.repo, "v0.3.4", feature_commit, "main")

    def test_manifest_version_is_read(self) -> None:
        self.assertEqual(policy.manifest_version(self.repo / "Cargo.toml"), "0.3.3")

    def test_package_version_mismatch_fails_before_network_access(self) -> None:
        arguments = policy.parser().parse_args(
            [
                "verify",
                "--tag",
                "v0.3.4",
                "--commit",
                self.commit,
                "--repository",
                "example/repository",
                "--repo",
                str(self.repo),
                "--manifest",
                str(self.repo / "Cargo.toml"),
            ]
        )
        with self.assertRaisesRegex(policy.PolicyError, "does not match package version"):
            policy.verify(arguments)

    def test_exact_successful_main_push_run_is_selected(self) -> None:
        payload = {
            "workflow_runs": [
                {
                    "id": 1,
                    "run_attempt": 1,
                    "event": "pull_request",
                    "head_branch": "main",
                    "head_sha": self.commit,
                    "path": policy.WORKFLOW_PATH,
                    "status": "completed",
                    "conclusion": "success",
                },
                {
                    "id": 2,
                    "run_attempt": 2,
                    "event": "push",
                    "head_branch": "main",
                    "head_sha": self.commit,
                    "path": policy.WORKFLOW_PATH,
                    "status": "completed",
                    "conclusion": "success",
                },
            ]
        }
        selected = policy.select_eligible_run(payload, self.commit, "main")
        self.assertEqual(selected["id"], 2)

    def test_failed_or_wrong_commit_ci_run_is_rejected(self) -> None:
        payload = {
            "workflow_runs": [
                {
                    "id": 1,
                    "event": "push",
                    "head_branch": "main",
                    "head_sha": "0" * 40,
                    "path": policy.WORKFLOW_PATH,
                    "status": "completed",
                    "conclusion": "success",
                },
                {
                    "id": 2,
                    "event": "push",
                    "head_branch": "main",
                    "head_sha": self.commit,
                    "path": policy.WORKFLOW_PATH,
                    "status": "completed",
                    "conclusion": "failure",
                },
            ]
        }
        with self.assertRaisesRegex(policy.PolicyError, "no successful"):
            policy.select_eligible_run(payload, self.commit, "main")

    def test_required_quality_aggregate_must_be_successful(self) -> None:
        successful = {
            "jobs": [
                {
                    "id": 10,
                    "name": policy.REQUIRED_AGGREGATE,
                    "status": "completed",
                    "conclusion": "success",
                }
            ]
        }
        self.assertEqual(policy.require_aggregate_job(successful)["id"], 10)

        failed = {
            "jobs": [
                {
                    "id": 11,
                    "name": policy.REQUIRED_AGGREGATE,
                    "status": "completed",
                    "conclusion": "failure",
                }
            ]
        }
        with self.assertRaisesRegex(policy.PolicyError, "did not complete"):
            policy.require_aggregate_job(failed)

        duplicate = {
            "jobs": [
                {
                    "id": 12,
                    "name": policy.REQUIRED_AGGREGATE,
                    "status": "completed",
                    "conclusion": "success",
                },
                {
                    "id": 13,
                    "name": policy.REQUIRED_AGGREGATE,
                    "status": "completed",
                    "conclusion": "success",
                },
            ]
        }
        with self.assertRaisesRegex(policy.PolicyError, "exactly one"):
            policy.require_aggregate_job(duplicate)


if __name__ == "__main__":
    unittest.main()
