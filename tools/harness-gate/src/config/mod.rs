mod diagnostic;
pub(crate) mod import;
mod loader;
mod migration;
mod model;
mod path;
pub(crate) mod quality;
mod scope;
mod validation;

#[cfg(test)]
mod tests;

pub(crate) use diagnostic::{report_for_error, ConfigDiagnostics, MINIMAL_CONFIG_SNIPPET};
#[cfg(test)]
pub(crate) use diagnostic::{ConfigDiagnostic, DiagnosticSeverity};
pub(crate) use loader::read_config_source;
pub use loader::schema_json;
pub use migration::migrate_v1;
pub(crate) use model::{
    BuiltinGateType, ContainerRuntimeKind, DoctorCheck, DoctorCheckKind, ExternalValuePolicy,
    FlowConfig, ParserConfig, PathScope, PathType, RunnerConfig, RunnerResultFormat, ServiceConfig,
    StepConfig, StepInput, StepKind, TestIsolation, UnmatchedScope, WaiverConfig, WebhookConfig,
    CONFIG_VERSION, DEFAULT_CONFIG_PATH,
};
pub use path::resolve_config_path;

pub(crate) use scope::CompiledScopeRules;
