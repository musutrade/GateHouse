//! Unreleased Podman profile boundary. No requests, execution or migration authority.
//! Docker lifecycle-v1 remains separate. Service identity and API wire semantics
//! must be specified before this profile can be wired into a host or adapter.
pub mod profile;

pub mod encoding;

pub mod observation;

pub mod native_info;

mod native_json;
pub mod native_status;

#[cfg(target_os = "linux")]
pub mod protected_context;

#[cfg(target_os = "linux")]
pub mod readonly;

pub mod native_exec;

pub mod ownership;
