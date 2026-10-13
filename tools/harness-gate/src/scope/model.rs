use crate::project::Project;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub enum ScopeMode {
    WorkingTree,
    Staged,
    Base(String),
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeResult {
    pub mode: String,
    pub changed_files: Vec<String>,
    pub components: BTreeSet<String>,
    pub unmatched_files: Vec<String>,
}

impl ScopeResult {
    pub fn all(project: &Project) -> Self {
        Self {
            mode: "all".to_string(),
            changed_files: Vec::new(),
            components: project.config.components(),
            unmatched_files: Vec::new(),
        }
    }
}
