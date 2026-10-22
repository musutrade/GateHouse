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

pub mod native_create;

pub mod native_image;

#[cfg(target_os = "linux")]
pub mod plan_record;

#[cfg(target_os = "linux")]
pub mod plan_storage;

#[cfg(target_os = "linux")]
pub mod installation;

pub mod capture_binding;

#[cfg(target_os = "linux")]
pub mod capture_wire;
