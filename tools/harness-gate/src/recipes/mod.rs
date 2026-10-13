//! Closed registry of built-in service recipes.
//!
//! A recipe owns everything specific to one managed service flavour: its
//! external-value validation, its legacy-configuration mapping and the
//! service it constructs for the v1 migration. The generic service,
//! migration and validation layers only call the narrow dispatcher below,
//! selected by the closed [`ExternalValuePolicy`] enum. Nothing is discovered
//! dynamically, no platform or plugin abstraction is introduced, and unknown
//! recipes keep failing closed: adding one is a reviewed code change.

pub(crate) mod postgres;

use crate::config::ExternalValuePolicy;
use anyhow::Result;

/// Narrow interface the generic layers use to validate a service's external
/// value. Docker/Podman execution, timeouts, leases and cleanup stay generic
/// and are never part of a recipe.
pub(crate) fn validate_external_value(policy: ExternalValuePolicy, value: &str) -> Result<()> {
    match policy {
        ExternalValuePolicy::None => Ok(()),
        ExternalValuePolicy::IsolatedPostgres => postgres::validate_external_value(value),
    }
}
