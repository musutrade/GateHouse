//! The built-in isolated-postgres recipe.
//!
//! This module owns every Postgres-specific decision: external-value
//! validation, the frozen legacy v1 compatibility mapping and the service it
//! constructs for the v1 migration. Docker/Podman execution, timeouts, leases
//! and resource cleanup stay in the generic service layer; the recipe only
//! supplies data and validation through its public functions.

use crate::config::{ContainerRuntimeKind, ExternalValuePolicy, ServiceConfig};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::net::IpAddr;
use url::{Host, Url};

pub(crate) const ALLOW_REMOTE_TEST_DATABASE_ENV: &str = "HARNESS_GATE_ALLOW_REMOTE_TEST_DATABASE";
pub(crate) const LEGACY_SERVICE_ID: &str = "test-postgres";

const DEFAULT_PORT: u16 = 5432;
const HEALTHCHECK_PROGRAM: &str = "pg_isready";
const IMAGE_ENV: &str = "HARNESS_GATE_POSTGRES_IMAGE";
const TIMEOUT_ENV: &str = "HARNESS_GATE_DATABASE_TIMEOUT_SECS";
const EXTERNAL_ENV: &str = "TEST_DATABASE_URL";

/// Narrow input for the legacy v1 database mapping. The generic migration
/// layer reads the legacy TOML shape and passes only these fields.
pub(crate) struct LegacyPostgres<'a> {
    pub(crate) image: &'a str,
    pub(crate) startup_timeout_secs: u64,
    pub(crate) container_port: u16,
    pub(crate) user: &'a str,
    pub(crate) password: &'a str,
    pub(crate) name: &'a str,
}

/// Construct the legacy `test-postgres` service. This is the frozen
/// compatibility mapping for version 1 configurations; its output must not
/// change without a reviewed migration.
pub(crate) fn legacy_service(legacy: &LegacyPostgres<'_>) -> ServiceConfig {
    ServiceConfig::Docker {
        runtime: ContainerRuntimeKind::Docker,
        image: legacy.image.to_string(),
        image_env: Some(IMAGE_ENV.into()),
        external_env: Some(EXTERNAL_ENV.into()),
        inject_env: EXTERNAL_ENV.into(),
        external_value_policy: ExternalValuePolicy::IsolatedPostgres,
        startup_timeout_secs: legacy.startup_timeout_secs,
        timeout_env: Some(TIMEOUT_ENV.into()),
        container_port: legacy.container_port,
        environment: BTreeMap::from([
            ("POSTGRES_USER".into(), legacy.user.to_string()),
            ("POSTGRES_PASSWORD".into(), legacy.password.to_string()),
            ("POSTGRES_DB".into(), legacy.name.to_string()),
        ]),
        healthcheck: vec![
            HEALTHCHECK_PROGRAM.into(),
            "-U".into(),
            legacy.user.to_string(),
            "-d".into(),
            legacy.name.to_string(),
        ],
        connection: format!(
            "postgres://{}:{}@127.0.0.1:{{host_port}}/{}",
            legacy.user, legacy.password, legacy.name
        ),
    }
}

pub(crate) fn validate_external_value(value: &str) -> Result<()> {
    let production_url = std::env::var("DATABASE_URL").ok();
    let allow_remote = std::env::var(ALLOW_REMOTE_TEST_DATABASE_ENV)
        .is_ok_and(|flag| matches!(flag.as_str(), "1" | "true"));
    validate_isolated_postgres_url(value, production_url.as_deref(), allow_remote)
}

#[derive(Debug, PartialEq, Eq)]
struct PostgresTarget {
    host: String,
    port: u16,
    database: String,
    loopback: bool,
}

fn parse_postgres_target(value: &str) -> Result<PostgresTarget> {
    let url = Url::parse(value).context("test database URL must be a valid URL")?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        bail!("test database URL must use the postgres or postgresql scheme");
    }
    let (host, loopback) = match url
        .host()
        .context("test database URL must include a host")?
    {
        Host::Domain(host) => {
            let normalized = host.to_ascii_lowercase();
            let loopback = host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback());
            (normalized, loopback)
        }
        Host::Ipv4(address) => (address.to_string(), IpAddr::V4(address).is_loopback()),
        Host::Ipv6(address) => (address.to_string(), IpAddr::V6(address).is_loopback()),
    };
    let mut segments = url
        .path_segments()
        .context("test database URL must include a database name")?;
    let database = segments
        .next()
        .filter(|segment| !segment.is_empty())
        .context("test database URL must include a database name")?
        .to_ascii_lowercase();
    if segments.any(|segment| !segment.is_empty()) {
        bail!("test database URL must contain exactly one database name");
    }
    Ok(PostgresTarget {
        host,
        port: url.port().unwrap_or(DEFAULT_PORT),
        database,
        loopback,
    })
}

pub(crate) fn validate_isolated_postgres_url(
    value: &str,
    production_url: Option<&str>,
    allow_remote: bool,
) -> Result<()> {
    let target = parse_postgres_target(value)?;
    if !target.database.ends_with("_test") && !target.database.ends_with("-test") {
        bail!("test database name must end with _test or -test");
    }
    if !target.loopback && !allow_remote {
        bail!(
            "remote test databases require {ALLOW_REMOTE_TEST_DATABASE_ENV}=1 after isolation is verified"
        );
    }
    if let Some(production_url) = production_url {
        if value == production_url {
            bail!("TEST_DATABASE_URL must not equal DATABASE_URL");
        }
        if let Ok(production) = parse_postgres_target(production_url) {
            let same_host =
                target.host == production.host || (target.loopback && production.loopback);
            if same_host && target.port == production.port && target.database == production.database
            {
                bail!("TEST_DATABASE_URL must not target the DATABASE_URL database");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        legacy_service, validate_isolated_postgres_url, LegacyPostgres,
        ALLOW_REMOTE_TEST_DATABASE_ENV, LEGACY_SERVICE_ID,
    };

    #[test]
    fn accepts_an_isolated_loopback_database() {
        let result = validate_isolated_postgres_url(
            "postgres://tester:secret@127.0.0.1:5432/arc_admin_test",
            Some("postgres://developer:secret@localhost:5432/arc_admin"),
            false,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn accepts_an_ipv6_loopback_database() {
        assert!(validate_isolated_postgres_url(
            "postgres://tester:secret@[::1]:5432/arc_admin_test",
            None,
            false,
        )
        .is_ok());
    }

    #[test]
    fn rejects_a_database_without_a_test_suffix() {
        let error = validate_isolated_postgres_url(
            "postgres://tester:secret@localhost/arc_admin",
            None,
            false,
        )
        .expect_err("production-like database name must fail");
        assert!(error.to_string().contains("must end with"));
    }

    #[test]
    fn rejects_a_remote_database_without_an_explicit_override() {
        let error = validate_isolated_postgres_url(
            "postgresql://tester:secret@test-db.example.com/arc_admin_test",
            None,
            false,
        )
        .expect_err("remote database must fail closed");
        assert!(error.to_string().contains(ALLOW_REMOTE_TEST_DATABASE_ENV));
    }

    #[test]
    fn accepts_an_explicitly_allowed_remote_test_database() {
        assert!(validate_isolated_postgres_url(
            "postgres://tester:secret@test-db.example.com/arc-admin-test",
            None,
            true,
        )
        .is_ok());
    }

    #[test]
    fn rejects_the_configured_runtime_database() {
        let error = validate_isolated_postgres_url(
            "postgres://tester:secret@127.0.0.1:5432/arc_admin_test",
            Some("postgresql://runtime:secret@localhost:5432/arc_admin_test"),
            false,
        )
        .expect_err("same database target must fail");
        assert!(
            error.to_string().contains("DATABASE_URL database"),
            "{error:?}"
        );
    }

    #[test]
    fn legacy_mapping_is_the_frozen_v1_contract() {
        let service = legacy_service(&LegacyPostgres {
            image: "postgres:16-alpine",
            startup_timeout_secs: 30,
            container_port: 5432,
            user: "test",
            password: "secret",
            name: "test",
        });
        let value = serde_json::to_value(&service).expect("serialize legacy service");
        assert_eq!(LEGACY_SERVICE_ID, "test-postgres");
        assert_eq!(
            value,
            serde_json::json!({
                "kind": "docker",
                "runtime": "docker",
                "image": "postgres:16-alpine",
                "image_env": "HARNESS_GATE_POSTGRES_IMAGE",
                "external_env": "TEST_DATABASE_URL",
                "inject_env": "TEST_DATABASE_URL",
                "external_value_policy": "isolated-postgres",
                "startup_timeout_secs": 30,
                "timeout_env": "HARNESS_GATE_DATABASE_TIMEOUT_SECS",
                "container_port": 5432,
                "environment": {
                    "POSTGRES_USER": "test",
                    "POSTGRES_PASSWORD": "secret",
                    "POSTGRES_DB": "test"
                },
                "healthcheck": ["pg_isready", "-U", "test", "-d", "test"],
                "connection": "postgres://test:secret@127.0.0.1:{host_port}/test"
            })
        );
    }
}
