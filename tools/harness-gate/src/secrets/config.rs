#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretConfig {
    version: u32,
    placeholders: PlaceholderConfig,
    rules: Vec<SecretRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlaceholderConfig {
    pub(super) minimum_unique_characters: usize,
    pub(super) maximum_nonalphanumeric_characters: usize,
    pub(super) markers: Vec<String>,
    pub(super) exact: Vec<String>,
    pub(super) prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalTestDatabasePolicy {
    pub(super) hosts: Vec<String>,
    pub(super) database_suffixes: Vec<String>,
    pub(super) require_username_equals_password: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum SecretRule {
    Direct {
        id: String,
        pattern: String,
    },
    Value {
        id: String,
        pattern: String,
        capture: usize,
        minimum_length: usize,
    },
    PostgresUrl {
        id: String,
        pattern: String,
        username_capture: usize,
        password_capture: usize,
        host_capture: usize,
        database_capture: usize,
        minimum_length: usize,
        #[serde(default)]
        local_test_policy: Option<LocalTestDatabasePolicy>,
    },
    WebhookUrl {
        id: String,
        pattern: String,
        capture: usize,
        query_parameters: Vec<String>,
        query_minimum_length: usize,
        path_minimum_length: usize,
    },
}

impl SecretRule {
    fn id(&self) -> &str {
        match self {
            Self::Direct { id, .. }
            | Self::Value { id, .. }
            | Self::PostgresUrl { id, .. }
            | Self::WebhookUrl { id, .. } => id,
        }
    }

    fn pattern(&self) -> &str {
        match self {
            Self::Direct { pattern, .. }
            | Self::Value { pattern, .. }
            | Self::PostgresUrl { pattern, .. }
            | Self::WebhookUrl { pattern, .. } => pattern,
        }
    }
}

pub(super) enum CompiledRule {
    Direct {
        pattern: Regex,
    },
    Value {
        pattern: Regex,
        capture: usize,
        minimum_length: usize,
    },
    PostgresUrl {
        pattern: Regex,
        username_capture: usize,
        password_capture: usize,
        host_capture: usize,
        database_capture: usize,
        minimum_length: usize,
        local_test_policy: Option<CompiledLocalTestDatabasePolicy>,
    },
    WebhookUrl {
        pattern: Regex,
        capture: usize,
        query_parameters: HashSet<String>,
        query_minimum_length: usize,
        path_minimum_length: usize,
    },
}

pub(super) struct CompiledLocalTestDatabasePolicy {
    pub(super) hosts: HashSet<String>,
    pub(super) database_suffixes: Vec<String>,
    pub(super) require_username_equals_password: bool,
}

pub(super) struct SecretScanner {
    placeholders: PlaceholderConfig,
    rules: Vec<CompiledRule>,
}

impl SecretScanner {
    pub(super) fn load(path: &Path) -> Result<Self> {
        let source = crate::config::read_config_source(path)
            .with_context(|| format!("read secret scan configuration {}", path.display()))?;
        Self::from_source(&source)
            .with_context(|| format!("parse secret scan configuration {}", path.display()))
    }

    pub(super) fn from_source(source: &str) -> Result<Self> {
        let config: SecretConfig =
            toml::from_str(source).context("parse secret scan configuration")?;
        if config.version != SECRET_CONFIG_VERSION {
            bail!(
                "unsupported secret scan config version {}; expected {}",
                config.version,
                SECRET_CONFIG_VERSION
            );
        }
        if config.rules.is_empty() {
            bail!("secret scan configuration requires at least one rule");
        }
        if config.placeholders.minimum_unique_characters == 0
            || config
                .placeholders
                .markers
                .iter()
                .chain(&config.placeholders.exact)
                .chain(&config.placeholders.prefixes)
                .any(|value| value.trim().is_empty())
        {
            bail!("secret scan placeholder policy contains an invalid value");
        }

        let mut ids = HashSet::new();
        let mut rules = Vec::with_capacity(config.rules.len());
        for rule in config.rules {
            let id = rule.id().to_string();
            if id.is_empty()
                || !id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
                || !ids.insert(id.clone())
            {
                bail!("secret scan rule IDs must be non-empty, portable, and unique: {id:?}");
            }
            let pattern = Regex::new(rule.pattern())
                .with_context(|| format!("secret scan rule {id:?} has an invalid regex"))?;
            let capture_count = pattern.captures_len();
            let compiled = match rule {
                SecretRule::Direct { .. } => CompiledRule::Direct { pattern },
                SecretRule::Value {
                    capture,
                    minimum_length,
                    ..
                } => {
                    validate_capture(&id, capture, capture_count)?;
                    validate_minimum(&id, minimum_length)?;
                    CompiledRule::Value {
                        pattern,
                        capture,
                        minimum_length,
                    }
                }
                SecretRule::PostgresUrl {
                    username_capture,
                    password_capture,
                    host_capture,
                    database_capture,
                    minimum_length,
                    local_test_policy,
                    ..
                } => {
                    let captures = [
                        username_capture,
                        password_capture,
                        host_capture,
                        database_capture,
                    ];
                    for capture in captures {
                        validate_capture(&id, capture, capture_count)?;
                    }
                    if captures.into_iter().collect::<HashSet<_>>().len() != captures.len() {
                        bail!("secret scan rule {id:?} requires distinct PostgreSQL captures");
                    }
                    validate_minimum(&id, minimum_length)?;
                    let local_test_policy = local_test_policy
                        .map(|policy| compile_local_test_policy(&id, policy))
                        .transpose()?;
                    CompiledRule::PostgresUrl {
                        pattern,
                        username_capture,
                        password_capture,
                        host_capture,
                        database_capture,
                        minimum_length,
                        local_test_policy,
                    }
                }
                SecretRule::WebhookUrl {
                    capture,
                    query_parameters,
                    query_minimum_length,
                    path_minimum_length,
                    ..
                } => {
                    validate_capture(&id, capture, capture_count)?;
                    validate_minimum(&id, query_minimum_length)?;
                    validate_minimum(&id, path_minimum_length)?;
                    if query_parameters.is_empty()
                        || query_parameters.iter().any(|value| value.trim().is_empty())
                    {
                        bail!("secret scan rule {id:?} requires query parameters");
                    }
                    CompiledRule::WebhookUrl {
                        pattern,
                        capture,
                        query_parameters: query_parameters
                            .into_iter()
                            .map(|value| value.to_ascii_lowercase())
                            .collect(),
                        query_minimum_length,
                        path_minimum_length,
                    }
                }
            };
            rules.push(compiled);
        }

        Ok(Self {
            placeholders: config.placeholders,
            rules,
        })
    }

    pub(super) fn is_match(&self, bytes: &[u8]) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.is_match(bytes, &self.placeholders))
    }
}
use super::matcher::{compile_local_test_policy, validate_capture, validate_minimum};
use anyhow::{bail, Context, Result};
use regex::bytes::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

const SECRET_CONFIG_VERSION: u32 = 2;
