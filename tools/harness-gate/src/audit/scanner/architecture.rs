use super::super::{ArchViolation, Config};
use super::{
    comment_ranges, compile_allowlist, compile_regexes, is_allowlisted_compiled, is_comment_offset,
    is_regular_file, resolve_rule_roots, source_line_at, source_line_starts,
};
use anyhow::Result;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) fn scan_arch_rules(
    project_root: &Path,
    config: &Config,
    exclude_dirs: &[PathBuf],
) -> Result<Vec<ArchViolation>> {
    let mut all_violations = Vec::new();

    for rule in &config.arch_rules {
        let root_paths =
            resolve_rule_roots(project_root, &rule.paths, &config.paths.aliases, &rule.name)?;
        let extensions = rule.extensions.clone();
        let patterns = rule.forbidden_patterns.clone();
        let allowed_patterns = rule.allowed_patterns.clone();
        let exclude_patterns = rule.exclude_patterns.clone();
        let allowlist = rule.allowlist.clone();

        if patterns.is_empty() {
            continue;
        }

        let regexes = compile_regexes(&patterns)?;
        let allowed_regexes = compile_regexes(&allowed_patterns)?;
        let exclude_regexes = compile_regexes(&exclude_patterns)?;
        let compiled_allowlist = compile_allowlist(&allowlist, &rule.name)?;

        let rule_name = rule.name.clone();
        let mut walk_builder = WalkBuilder::new(root_paths[0].clone());
        for root_path in root_paths.iter().skip(1) {
            walk_builder.add(root_path);
        }
        // Keep ignore-file traversal in the audit boundary so rule scanning
        // cannot accidentally bypass repository exclusions.
        let entries = walk_builder
            .add_custom_ignore_filename(&config.engine.ignore_filename)
            .follow_links(false)
            .build()
            .collect::<std::result::Result<Vec<_>, ignore::Error>>()?;
        let rule_violations = entries
            .into_par_iter()
            .filter(|entry| {
                let path = entry.path();
                if !is_regular_file(path) {
                    return false;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !extensions.contains(&ext.to_string()) {
                        return false;
                    }
                } else {
                    return false;
                }
                for excl in exclude_dirs {
                    if path.starts_with(excl) {
                        return false;
                    }
                }
                let relative = path.strip_prefix(project_root).unwrap_or(path);
                let path_str = relative.to_str().unwrap_or("");
                if exclude_regexes.iter().any(|re| re.is_match(path_str)) {
                    return false;
                }
                !is_allowlisted_compiled(path, project_root, &compiled_allowlist)
            })
            .map(|entry| -> Result<Vec<ArchViolation>> {
                if crate::process::cancelled() {
                    return Err(anyhow::anyhow!("audit scan cancelled"));
                }
                let path = entry.path();
                let content = super::read_source(path)?;
                let mut violations = Vec::new();
                let mut reported_lines = HashSet::new();
                let line_starts = source_line_starts(&content);
                let extension = path.extension().and_then(|value| value.to_str());
                let comments = extension
                    .and_then(|extension| config.engine.comment_syntax.get(extension))
                    .map(|syntax| comment_ranges(&content, syntax))
                    .unwrap_or_default();

                for re in &regexes {
                    for matched in re.find_iter(&content) {
                        let (line_number, line, _) =
                            source_line_at(&content, &line_starts, matched.start());
                        if is_comment_offset(&comments, matched.start())
                            || allowed_regexes.iter().any(|allowed| allowed.is_match(line))
                            || !reported_lines.insert(line_number)
                        {
                            continue;
                        }
                        violations.push(ArchViolation {
                            file: path
                                .strip_prefix(project_root)
                                .unwrap_or(path)
                                .to_path_buf(),
                            line: line_number,
                            content: line.trim().to_string(),
                            rule_name: rule_name.clone(),
                        });
                    }
                }
                violations.sort_by_key(|violation| violation.line);
                Ok(violations)
            })
            .collect::<Result<Vec<_>>>()?;

        all_violations.extend(rule_violations.into_iter().flatten());
        if crate::process::cancelled() {
            return Err(anyhow::anyhow!("audit scan cancelled"));
        }
    }

    Ok(all_violations)
}
