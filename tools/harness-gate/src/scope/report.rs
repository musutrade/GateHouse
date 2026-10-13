use super::{ScopeError, ScopeResult};
use crate::project::Project;
use crate::utils::fs as output_fs;
use std::path::Path;

impl ScopeResult {
    pub fn write_reports(&self, project: &Project) -> std::result::Result<(), ScopeError> {
        let changed = if self.changed_files.is_empty() {
            String::new()
        } else {
            format!("{}\n", self.changed_files.join("\n"))
        };
        output_fs::confined_atomic_write(
            &project.reports,
            Path::new("changed_files.txt"),
            changed,
            true,
        )
        .map_err(ScopeError::report)?;
        output_fs::confined_write_json(&project.reports, Path::new("scope.json"), self, true)
            .map_err(ScopeError::report)?;
        Ok(())
    }
}
