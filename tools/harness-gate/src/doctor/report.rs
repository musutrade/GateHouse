use crate::project::Project;
use crate::ui;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
struct Check {
    level: Level,
    name: String,
    detail: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    project_root: String,
    checks: Vec<Check>,
    pub failures: usize,
    pub warnings: usize,
}

impl DoctorReport {
    pub(crate) fn new(project: &Project) -> Self {
        Self {
            project_root: project.root.to_string_lossy().to_string(),
            checks: Vec::new(),
            failures: 0,
            warnings: 0,
        }
    }

    pub(crate) fn record_pass(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.checks.push(Check {
            level: Level::Pass,
            name: name.into(),
            detail: detail.into(),
        });
    }

    pub(crate) fn record_failure(
        &mut self,
        required: bool,
        name: impl Into<String>,
        detail: impl Into<String>,
    ) {
        let level = if required { Level::Fail } else { Level::Warn };
        match level {
            Level::Fail => self.failures += 1,
            Level::Warn => self.warnings += 1,
            Level::Pass => {}
        }
        self.checks.push(Check {
            level,
            name: name.into(),
            detail: detail.into(),
        });
    }

    pub fn print(&self) {
        println!("{}", ui::heading("harness-gate doctor"));
        println!("Project: {}\n", self.project_root);
        for check in &self.checks {
            let marker = match check.level {
                Level::Pass => ui::pass("PASS"),
                Level::Warn => ui::warning("WARN"),
                Level::Fail => ui::failure("FAIL"),
            };
            println!("[{marker}] {:<22} {}", check.name, check.detail);
        }
        println!(
            "\nSummary: {} failure(s), {} warning(s)",
            self.failures, self.warnings
        );
    }
}
