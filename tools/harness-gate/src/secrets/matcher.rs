use super::config::{
    CompiledLocalTestDatabasePolicy, CompiledRule, LocalTestDatabasePolicy, PlaceholderConfig,
};
use anyhow::{bail, Result};
use std::collections::HashSet;
use url::Url;

impl CompiledRule {
    pub(super) fn is_match(&self, bytes: &[u8], placeholders: &PlaceholderConfig) -> bool {
        match self {
            Self::Direct { pattern } => pattern.is_match(bytes),
            Self::Value {
                pattern,
                capture,
                minimum_length,
            } => pattern.captures_iter(bytes).any(|captures| {
                captures.get(*capture).is_some_and(|value| {
                    looks_like_secret(value.as_bytes(), *minimum_length, placeholders)
                })
            }),
            Self::PostgresUrl {
                pattern,
                username_capture,
                password_capture,
                host_capture,
                database_capture,
                minimum_length,
                local_test_policy,
            } => pattern.captures_iter(bytes).any(|captures| {
                let values = [
                    *username_capture,
                    *password_capture,
                    *host_capture,
                    *database_capture,
                ]
                .map(|index| captures.get(index).map(|value| value.as_bytes()));
                let [Some(username), Some(password), Some(host), Some(database)] = values else {
                    return false;
                };
                if local_test_policy.as_ref().is_some_and(|policy| {
                    is_local_test_database(username, password, host, database, policy)
                }) {
                    return false;
                }
                looks_like_secret(password, *minimum_length, placeholders)
            }),
            Self::WebhookUrl {
                pattern,
                capture,
                query_parameters,
                query_minimum_length,
                path_minimum_length,
            } => pattern.captures_iter(bytes).any(|captures| {
                captures.get(*capture).is_some_and(|value| {
                    webhook_url_contains_secret(
                        value.as_bytes(),
                        query_parameters,
                        *query_minimum_length,
                        *path_minimum_length,
                        placeholders,
                    )
                })
            }),
        }
    }
}

pub(super) fn validate_capture(id: &str, capture: usize, capture_count: usize) -> Result<()> {
    if capture == 0 || capture >= capture_count {
        bail!(
            "secret scan rule {id:?} references capture {capture}, but its regex has {} capture group(s)",
            capture_count.saturating_sub(1)
        );
    }
    Ok(())
}

pub(super) fn validate_minimum(id: &str, minimum: usize) -> Result<()> {
    if minimum == 0 {
        bail!("secret scan rule {id:?} minimum length must be positive");
    }
    Ok(())
}

pub(super) fn compile_local_test_policy(
    id: &str,
    policy: LocalTestDatabasePolicy,
) -> Result<CompiledLocalTestDatabasePolicy> {
    if policy.hosts.is_empty()
        || policy.database_suffixes.is_empty()
        || policy
            .hosts
            .iter()
            .chain(&policy.database_suffixes)
            .any(|value| value.trim().is_empty())
    {
        bail!("secret scan rule {id:?} has an invalid local test database policy");
    }
    Ok(CompiledLocalTestDatabasePolicy {
        hosts: policy
            .hosts
            .into_iter()
            .map(|host| host.to_ascii_lowercase())
            .collect(),
        database_suffixes: policy
            .database_suffixes
            .into_iter()
            .map(|suffix| suffix.to_ascii_lowercase())
            .collect(),
        require_username_equals_password: policy.require_username_equals_password,
    })
}

pub(super) fn is_local_test_database(
    username: &[u8],
    password: &[u8],
    host: &[u8],
    database: &[u8],
    policy: &CompiledLocalTestDatabasePolicy,
) -> bool {
    let host = String::from_utf8_lossy(host).to_ascii_lowercase();
    let database = String::from_utf8_lossy(database).to_ascii_lowercase();
    policy.hosts.contains(&host)
        && policy
            .database_suffixes
            .iter()
            .any(|suffix| database.ends_with(suffix))
        && (!policy.require_username_equals_password || username == password)
}

fn webhook_url_contains_secret(
    value: &[u8],
    query_parameters: &HashSet<String>,
    query_minimum_length: usize,
    path_minimum_length: usize,
    placeholders: &PlaceholderConfig,
) -> bool {
    let Ok(value) = std::str::from_utf8(value) else {
        return false;
    };
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if url.query_pairs().any(|(name, value)| {
        query_parameters.contains(name.to_ascii_lowercase().as_str())
            && looks_like_secret(value.as_bytes(), query_minimum_length, placeholders)
    }) {
        return true;
    }
    url.path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .is_some_and(|segment| {
            looks_like_secret(segment.as_bytes(), path_minimum_length, placeholders)
        })
}

pub(super) fn looks_like_secret(
    value: &[u8],
    minimum_length: usize,
    placeholders: &PlaceholderConfig,
) -> bool {
    let value = String::from_utf8_lossy(value);
    let value = value.trim_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '"' | '\'')
    });
    if value.len() < minimum_length
        || placeholders
            .prefixes
            .iter()
            .any(|prefix| value.starts_with(prefix))
    {
        return false;
    }

    let lowercase = value.to_ascii_lowercase();
    if placeholders
        .markers
        .iter()
        .any(|marker| lowercase.contains(&marker.to_ascii_lowercase()))
        || placeholders
            .exact
            .iter()
            .any(|placeholder| lowercase == placeholder.to_ascii_lowercase())
    {
        return false;
    }

    let significant = value
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric())
        .collect::<Vec<_>>();
    let unique = significant.iter().copied().collect::<HashSet<_>>();
    significant.len()
        >= minimum_length.saturating_sub(placeholders.maximum_nonalphanumeric_characters)
        && unique.len() >= placeholders.minimum_unique_characters
}
