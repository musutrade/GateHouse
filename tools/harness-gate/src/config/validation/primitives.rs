use anyhow::{bail, Result};
use std::path::{Component as PathComponent, Path};

pub(super) fn validate_id(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
    {
        bail!("{name} must be a lowercase identifier, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_program(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '+')
        })
    {
        bail!("{name} must be a bare executable name, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_env_name(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        bail!("{name} must be an uppercase environment variable name, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_image(value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('-')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '/' | ':' | '@' | '-')
        })
    {
        bail!("Docker image must be an OCI image reference, found {value:?}");
    }
    Ok(())
}

pub(super) fn validate_repo_path(name: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    let windows_prefixed = value.starts_with("\\\\")
        || value.starts_with("//")
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
        || value.starts_with('\\');
    if value.is_empty() || value.contains('\0') || path.is_absolute() || windows_prefixed {
        bail!("{name} must be a non-empty repository-relative path");
    }
    // `Path` follows the host platform. Reject Windows-style traversal on Unix
    // too, so a configuration checked in CI cannot become an escape when it is
    // later used on Windows.
    if value.split(['/', '\\']).any(|component| component == "..") {
        bail!("{name} may not escape the repository: {value:?}");
    }
    if path.components().any(|component| {
        matches!(
            component,
            PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
        )
    }) {
        bail!("{name} may not escape the repository: {value:?}");
    }
    Ok(())
}
