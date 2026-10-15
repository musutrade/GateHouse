mod diagnostic;
mod impact;
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
pub use loader::schema_json;
pub(crate) use loader::{read_config_source, MAX_CONFIG_BYTES};
pub use migration::migrate_v1;
#[cfg(unix)]
pub(crate) use model::StepCacheConfig;
pub(crate) use model::{
    BuiltinGateType, ContainerRuntimeKind, DoctorCheck, DoctorCheckKind, FlowConfig, ParserConfig,
    PathScope, PathType, RunnerConfig, RunnerResultFormat, ServiceCheck, ServiceConfig, StepConfig,
    StepInput, StepKind, TestIsolation, UnmatchedScope, WaiverConfig, WebhookConfig,
    CONFIG_VERSION, DEFAULT_CONFIG_PATH,
};
pub use path::resolve_config_path;
#[cfg(unix)]
pub(crate) use validation::steps::validate_cache_path;
#[cfg(unix)]
pub(crate) use validation::steps::validate_step_cache;

pub(crate) use scope::CompiledScopeRules;
