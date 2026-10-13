mod architecture;
mod hard_rules;

mod filter;
mod lexical;
pub(super) use architecture::scan_arch_rules;
#[cfg(test)]
pub(super) use filter::is_allowlisted;
pub(super) use filter::{
    compile_allowlist, compile_regexes, is_allowlisted_compiled, is_regular_file, read_source,
    resolve_excludes, resolve_rule_roots,
};
pub(super) use hard_rules::scan_files;
use lexical::{comment_ranges, is_comment_offset, source_line_at, source_line_starts};
