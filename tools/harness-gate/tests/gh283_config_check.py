import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

FIXTURE_FILES = {'flow.toml': 'version = 2\n\n[project]\nname = "baseline-fixture"\ndefault_profile = "full"\nhook_profile = "hook"\n\n[paths]\nreports = ".harness-gate/reports"\naudit_config = ".harness-gate/audit.toml"\nsecrets_config = ".harness-gate/secrets.toml"\n\n[paths.aliases]\n\n[policy]\nrequired_steps = [\n    "project.diff-check",\n    "project.staged-diff-check",\n]\nwaivers = []\n\n[[doctor.checks]]\nid = "tool.git"\nlabel = "git"\nrequired = true\ntimeout_secs = 15\nkind = "command"\nprogram = "git"\nargs = ["--version"]\n\n[[doctor.checks]]\nid = "git.remotes"\nlabel = "Git remotes"\nrequired = true\ntimeout_secs = 15\nkind = "git-remotes"\n\n[services]\n\n[parsers]\n\n[report_templates]\n\n[execution]\nparallel = false\n\n[execution.retries]\n\n[execution.shards]\n\n[notifications]\nwebhooks = []\n\n[scope]\nunmatched = "all"\n\n[[scope.rules]]\npatterns = ["**"]\ncomponents = ["project"]\n\n[[steps]]\nid = "project.diff-check"\nlabel = "Git whitespace check"\ncomponent = "project"\nprofiles = ["full"]\nprogram = "git"\nargs = [\n    "diff",\n    "--check",\n]\ncwd = "{root}"\nlog = "git_diff_check.log"\ntimeout_secs = 60\nservices = []\nremove_env = []\ndepends_on = []\ninput = "repository"\n\n[[steps]]\nid = "project.staged-diff-check"\nlabel = "staged Git whitespace check"\ncomponent = "project"\nprofiles = ["hook"]\nprogram = "git"\nargs = [\n    "diff",\n    "--cached",\n    "--check",\n]\ncwd = "{root}"\nlog = "git_staged_diff_check.log"\ntimeout_secs = 60\nservices = []\nremove_env = []\ndepends_on = []\ninput = "repository"\n', 'audit.toml': 'version = 2\n\n[engine]\nignore_filename = ".auditignore"\njson_report_filename = "review_context.json"\nmarkdown_report_filename = "review_context.md"\nmarkdown_max_bytes = 4096\nmarkdown_occurrences_per_rule = 3\n\n[engine.comment_syntax.rs]\nline = ["//"]\nblock = [{ start = "/*", end = "*/", nested = true }]\nstrings = [\n  { start = \'r###"\', end = \'"###\' },\n  { start = \'r##"\', end = \'"##\' },\n  { start = \'r#"\', end = \'"#\' },\n  { start = \'r"\', end = \'"\' },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n]\n\n[engine.comment_syntax.sql]\nline = ["--"]\nblock = [{ start = "/*", end = "*/" }]\nstrings = [\n  { start = "\'", end = "\'" },\n  { start = \'"\', end = \'"\' },\n]\n\n[engine.comment_syntax.ts]\nline = ["//"]\nblock = [{ start = "/*", end = "*/" }]\nstrings = [\n  { start = "\'", end = "\'", escape = \'\\\' },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n  { start = "`", end = "`", escape = \'\\\' },\n]\n\n[engine.comment_syntax.tsx]\nline = ["//"]\nblock = [{ start = "/*", end = "*/" }]\nstrings = [\n  { start = "\'", end = "\'", escape = \'\\\' },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n  { start = "`", end = "`", escape = \'\\\' },\n]\n\n[engine.comment_syntax.js]\nline = ["//"]\nblock = [{ start = "/*", end = "*/" }]\nstrings = [\n  { start = "\'", end = "\'", escape = \'\\\' },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n  { start = "`", end = "`", escape = \'\\\' },\n]\n\n[engine.comment_syntax.jsx]\nline = ["//"]\nblock = [{ start = "/*", end = "*/" }]\nstrings = [\n  { start = "\'", end = "\'", escape = \'\\\' },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n  { start = "`", end = "`", escape = \'\\\' },\n]\n\n[engine.comment_syntax.toml]\nline = ["#"]\nstrings = [\n  { start = "\'", end = "\'" },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n]\n\n[engine.comment_syntax.yaml]\nline = ["#"]\nstrings = [\n  { start = "\'", end = "\'" },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n]\n\n[engine.comment_syntax.yml]\nline = ["#"]\nstrings = [\n  { start = "\'", end = "\'" },\n  { start = \'"\', end = \'"\', escape = \'\\\' },\n]\n\n[paths]\nexclude = ["target", "node_modules", "dist", ".git", ".harness-gate/reports"]\n', 'secrets.toml': 'version = 2\n\n[placeholders]\nminimum_unique_characters = 4\nmaximum_nonalphanumeric_characters = 2\nprefixes = ["${", "{{", "<"]\nmarkers = [\n  "change-me",\n  "changeme",\n  "replace-me",\n  "replace_me",\n  "placeholder",\n  "example",\n  "your-",\n  "your_",\n  "dummy",\n  "not-a-secret",\n  "not_a_secret",\n  "secret-here",\n  "secret_here",\n  "for-testing",\n  "for_testing",\n]\nexact = [\n  "postgres",\n  "password",\n  "secret",\n  "admin",\n  "test-password",\n  "local-password",\n]\n\n[[rules]]\nid = "provider-token"\nkind = "direct"\npattern = \'\'\'github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|glpat-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|npm_[A-Za-z0-9]{36}|eyJ[A-Za-z0-9_-]{8,}\\.[A-Za-z0-9_-]{8,}\\.[A-Za-z0-9_-]{8,}|https?://[^/@\\s]+:[^@\\s]+@[^\\s]+|https://hooks\\.slack\\.com/services/[A-Za-z0-9_-]{8,}/[A-Za-z0-9_-]{8,}/[A-Za-z0-9_-]{20,}|-----BEGIN ([A-Z0-9 ]+ )?PRIVATE KEY-----\'\'\'\n\n[[rules]]\nid = "named-secret"\nkind = "value"\npattern = \'\'\'(?ix)\n  \\b(?:\n    jwt_(?:secret|signing_(?:key|secret)|encryption_key) |\n    (?:access_|refresh_)?token_secret |\n    session_secret |\n    cookie_secret |\n    auth_secret |\n    (?:wecom|wechat_work|weixin|dingtalk|ding_talk)[a-z0-9_]*(?:secret|token|key) |\n    [a-z0-9_]*webhook_(?:secret|token|key)\n  )\\b\n  (?:\n    ["\']?\\s*[:=]\\s*["\']? |\n    \\s*:\\s*[^=\\r\\n]{1,40}=\\s*["\']?\n  )\n  ([A-Za-z0-9][A-Za-z0-9._~+/=-]{7,})\n\'\'\'\ncapture = 1\nminimum_length = 12\n\n[[rules]]\nid = "postgres-credentials"\nkind = "postgres-url"\npattern = \'\'\'(?ix)\n  \\bpostgres(?:ql)?://\n  ([^:/@\\s"\'<>]+):\n  ([^@/\\s"\'<>]+)@\n  (\\[[^\\]]+\\]|[^:/\\s"\'<>]+)\n  (?::[^/\\s"\'<>]+)?\n  /([A-Za-z0-9_.-]+)\n\'\'\'\nusername_capture = 1\npassword_capture = 2\nhost_capture = 3\ndatabase_capture = 4\nminimum_length = 8\nlocal_test_policy = { hosts = ["localhost", "127.0.0.1", "::1", "[::1]"], database_suffixes = ["_test", "-test"], require_username_equals_password = true }\n\n[[rules]]\nid = "secret-bearing-webhook-url"\nkind = "webhook-url"\npattern = \'\'\'(?ix)\n  \\b[a-z0-9_]*(?:webhook|callback)(?:_url)?\\b\n  ["\']?\\s*[:=]\\s*["\']?\n  (https?://[^\\s"\'<>]+)\n\'\'\'\ncapture = 1\nquery_parameters = ["key", "token", "access_token", "secret", "signature", "sig"]\nquery_minimum_length = 12\npath_minimum_length = 20\n\n[[rules]]\nid = "wecom-webhook"\nkind = "value"\npattern = \'\'\'(?i)https://qyapi\\.weixin\\.qq\\.com/cgi-bin/webhook/send\\?key=([A-Za-z0-9_-]{16,})\'\'\'\ncapture = 1\nminimum_length = 16\n\n[[rules]]\nid = "dingtalk-webhook"\nkind = "value"\npattern = \'\'\'(?i)https://(?:oapi|api)\\.dingtalk\\.com/[^\\s"\'<>]*[?&]access_token=([A-Za-z0-9_-]{16,})\'\'\'\ncapture = 1\nminimum_length = 16\n'}
CASES = {
    'root_auto_human_json_agree': True,
    'one_level_auto_human_json_agree': True,
    'multi_level_auto_human_json_agree': True,
    'explicit_project_root_override_human_json_agree': True,
    'explicit_config_override_human_json_agree': True,
    'missing_project_human_json_agree': False,
    'malformed_config_human_json_agree': False,
    'invalid_config_human_json_agree': False,
}

def scenario(binary, case):
    with tempfile.TemporaryDirectory(prefix='relay-gh283-fixed-') as temporary:
        root = Path(temporary) / 'project'
        root.mkdir()
        config_dir = root / '.harness-gate'
        config_dir.mkdir()
        for name, content in FIXTURE_FILES.items():
            (config_dir / name).write_text(content, encoding='utf-8')
        # A private Git boundary is needed by the existing config-override discovery.
        subprocess.run(['git', '-c', 'core.hooksPath=' + os.devnull, 'init', '--quiet', str(root)], check=True)
        child = root / 'src'; nested = child / 'module' / 'deep'
        nested.mkdir(parents=True)
        cwd = root
        overrides = []
        if case == 'one_level_auto_human_json_agree':
            cwd = child
        elif case == 'multi_level_auto_human_json_agree':
            cwd = nested
        elif case == 'explicit_project_root_override_human_json_agree':
            cwd = nested
            (cwd / '.harness-gate').mkdir()
            (cwd / '.harness-gate/flow.toml').write_text('version = 999\n')
            overrides = ['--project-root', str(root)]
        elif case == 'explicit_config_override_human_json_agree':
            cwd = nested
            alternate = root / 'alternate.toml'
            alternate.write_text(FIXTURE_FILES['flow.toml'], encoding='utf-8')
            (config_dir / 'flow.toml').write_text('not = [valid toml')
            overrides = ['--config', str(alternate)]
        elif case == 'missing_project_human_json_agree':
            cwd = Path(temporary) / 'missing' / 'nested'
            cwd.mkdir(parents=True)
            assert not any((parent / '.harness-gate/flow.toml').is_file() for parent in [cwd, *cwd.parents])
        elif case == 'malformed_config_human_json_agree':
            (config_dir / 'flow.toml').write_text('not = [valid toml')
        elif case == 'invalid_config_human_json_agree':
            (config_dir / 'flow.toml').write_text(FIXTURE_FILES['flow.toml'].replace('version = 2', 'version = 999', 1), encoding='utf-8')
        records = []
        for fmt in ['human', 'json']:
            argv = [str(binary), '--color', 'never', *overrides, 'config', 'check', '--format', fmt]
            run = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, timeout=30)
            document = None
            if fmt == 'json':
                document = json.loads(run.stdout)
                assert isinstance(document, dict) and type(document.get('valid')) is bool
                assert isinstance(document.get('diagnostics'), list)
            record = {'case': case, 'format': fmt, 'argv': argv, 'cwd': str(cwd),
                      'exit_code': run.returncode, 'decision': document['valid'] if document is not None else run.returncode == 0,
                      'json_parseable': document is not None}
            records.append(record)
        expected = CASES[case]
        assert records[0]['exit_code'] == records[1]['exit_code'] == (0 if expected else 1), json.dumps(records)
        assert records[0]['decision'] == records[1]['decision'] == expected, json.dumps(records)
        if expected:
            assert 'Configuration valid:' in subprocess.run([str(binary), '--color', 'never', *overrides, 'config', 'check'], cwd=cwd, capture_output=True, text=True, timeout=30).stdout
        return records

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--case', choices=sorted(CASES))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    cases = [args.case] if args.case else list(CASES)
    records = []
    for case in cases:
        records.extend(scenario(binary, case))
    print(json.dumps({'status': 'passed', 'cases': len(cases), 'records': records}))

if __name__ == '__main__':
    main()
