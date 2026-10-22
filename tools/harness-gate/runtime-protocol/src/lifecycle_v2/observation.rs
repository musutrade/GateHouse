//! Native Libpod observation comparison, not daemon authentication or restart proof.
//! Adapter-side HTTP parsing and Core-held socket/config verification remain required.
use serde::{Deserialize, Serialize};

pub const API: &str = "6.1.4";
pub const INFO_ROUTE: &str = "/v6.1.4/libpod/info";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocketObservation {
    pub path: String,
    pub device: u64,
    pub inode: u64,
    pub uid: u32,
    /// Full Unix st_mode, including socket type; never inferred from a URI.
    pub mode: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteSocketObservation {
    pub path: String,
    pub exists: bool,
}

/// Normalized selected native-info fields plus local socket metadata.
/// These are not the HTTP response schema. No compat ID or daemon_id is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub engine: String,
    pub api_surface: String,
    pub selected_api: String,
    pub server_version: String,
    pub hostname: String,
    pub rootless: bool,
    pub graph_root: String,
    pub run_root: String,
    pub remote_socket: Option<RemoteSocketObservation>,
    pub socket: SocketObservation,
}

/// Operator assertion from protected configuration, never an engine observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedContext {
    pub operator_epoch: String,
    pub principal_uid: u32,
    pub observation: Observation,
}

fn absolute_path(value: &str) -> bool {
    value.starts_with('/')
        && value.len() <= 4096
        && value
            .split('/')
            .skip(1)
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && !value.chars().any(char::is_control)
}

impl Observation {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.engine != "podman"
            || self.api_surface != "libpod"
            || self.selected_api != API
            || self.server_version != API
        {
            return Err("unsupported native Podman observation profile");
        }
        if self.hostname.is_empty()
            || self.hostname.len() > 255
            || self
                .hostname
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            || !absolute_path(&self.graph_root)
            || !absolute_path(&self.run_root)
            || !absolute_path(&self.socket.path)
            || self.socket.inode == 0
            || self.socket.mode & 0o170000 != 0o140000
            || self.socket.mode & 0o7000 != 0
            || self.socket.mode & 0o007 != 0
            || self.socket.mode & !0o177777 != 0
        {
            return Err("invalid native Podman observation");
        }
        if let Some(remote) = &self.remote_socket {
            if !remote.exists || remote.path != self.socket.path {
                return Err("Podman advertised socket differs from selected socket");
            }
        }
        Ok(())
    }
}

impl ExpectedContext {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.operator_epoch.len() != 32
            || !self
                .operator_epoch
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("explicit operator epoch required");
        }
        self.observation.validate()
    }

    /// Supported local deployment policy, not an authentication claim.
    /// Rootful sockets must belong to root; rootless sockets to the non-root
    /// configured principal. Other deployments require an explicit future profile.
    pub fn validate_local_socket_policy(&self) -> Result<(), &'static str> {
        self.validate()?;
        let uid = self.observation.socket.uid;
        if self.observation.rootless {
            if self.principal_uid == 0 || uid != self.principal_uid {
                return Err("rootless socket must belong to non-root principal");
            }
        } else if uid != 0 {
            return Err("rootful socket must belong to root");
        }
        Ok(())
    }

    /// Integration entry on Unix: compare the configured principal to actual
    /// process euid, not another caller-supplied identity. Config protection is
    /// still the host's responsibility; this does not open any file.
    #[cfg(unix)]
    pub fn match_current_operation(
        &self,
        bound: &Self,
        before: &Observation,
        after: &Observation,
    ) -> Result<(), &'static str> {
        if self.principal_uid != unsafe { libc::geteuid() } {
            return Err("Podman configured principal differs from process euid");
        }
        self.match_operation(bound, before, after)
    }

    /// Compare the exact protected caller context and both observations.
    /// Equal observations can occur across socket activation or service restart.
    /// Success therefore says nothing about daemon lifetime or authentication.
    pub fn match_operation(
        &self,
        bound: &Self,
        before: &Observation,
        after: &Observation,
    ) -> Result<(), &'static str> {
        self.validate()?;
        bound.validate()?;
        before.validate()?;
        after.validate()?;
        if self != bound || &self.observation != before || before != after {
            return Err("Podman operation context or observations changed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> ExpectedContext {
        ExpectedContext {
            operator_epoch: "a".repeat(32),
            principal_uid: 1000,
            observation: Observation {
                engine: "podman".into(),
                api_surface: "libpod".into(),
                selected_api: API.into(),
                server_version: API.into(),
                hostname: "fixture".into(),
                rootless: false,
                graph_root: "/var/lib/containers/storage".into(),
                run_root: "/run/containers/storage".into(),
                remote_socket: None,
                socket: SocketObservation {
                    path: "/run/podman/podman.sock".into(),
                    device: 1,
                    inode: 2,
                    uid: 0,
                    mode: 0o140660,
                },
            },
        }
    }

    #[test]
    fn explicit_rootful_and_rootless_contexts_match() {
        for rootless in [false, true] {
            let mut c = context();
            c.observation.rootless = rootless;
            if rootless {
                c.observation.socket.uid = 1000;
            }
            c.match_operation(&c, &c.observation, &c.observation)
                .unwrap();
        }
    }

    #[test]
    fn epoch_and_principal_changes_are_rejected() {
        let c = context();
        let mut epoch = c.clone();
        epoch.operator_epoch = "b".repeat(32);
        let mut principal = c.clone();
        principal.principal_uid += 1;
        for changed in [epoch, principal] {
            assert!(c
                .match_operation(&changed, &c.observation, &c.observation)
                .is_err());
        }
        for value in ["".into(), "A".repeat(32), "a".repeat(31)] {
            let mut changed = c.clone();
            changed.operator_epoch = value;
            assert!(changed.validate().is_err());
        }
    }

    #[test]
    fn every_observation_dimension_is_bound() {
        let c = context();
        let original = serde_json::to_value(&c.observation).unwrap();
        for (field, value) in [
            ("engine", serde_json::json!("docker")),
            ("api_surface", serde_json::json!("compat")),
            ("selected_api", serde_json::json!("1.44.0")),
            ("server_version", serde_json::json!("6.1.5")),
            ("hostname", serde_json::json!("other")),
            ("rootless", serde_json::json!(true)),
            ("graph_root", serde_json::json!("/other")),
            ("run_root", serde_json::json!("/other")),
        ] {
            let mut v = original.clone();
            v[field] = value;
            let changed: Observation = serde_json::from_value(v).unwrap();
            assert!(
                c.match_operation(&c, &c.observation, &changed).is_err(),
                "{field}"
            );
            assert!(
                c.match_operation(&c, &changed, &changed).is_err(),
                "{field}"
            );
        }
        for (field, value) in [
            ("device", 3),
            ("inode", 3),
            ("uid", 1000),
            ("mode", 0o140600),
        ] {
            let mut v = original.clone();
            v["socket"][field] = serde_json::json!(value);
            let changed: Observation = serde_json::from_value(v).unwrap();
            assert!(
                c.match_operation(&c, &c.observation, &changed).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn optional_remote_socket_is_exact_when_present() {
        let mut c = context();
        c.observation.remote_socket = Some(RemoteSocketObservation {
            path: c.observation.socket.path.clone(),
            exists: true,
        });
        c.validate().unwrap();
        let mut changed = c.observation.clone();
        changed.remote_socket = None;
        assert!(c.match_operation(&c, &c.observation, &changed).is_err());
        c.observation.remote_socket.as_mut().unwrap().exists = false;
        assert!(c.validate().is_err());
    }

    #[test]
    fn daemon_id_epoch_in_response_and_duplicate_fields_are_rejected() {
        let o = context().observation;
        for key in ["daemon_id", "ID", "operator_epoch"] {
            let mut v = serde_json::to_value(&o).unwrap();
            v[key] = "fabricated".into();
            assert!(serde_json::from_value::<Observation>(v).is_err());
        }
        let text = serde_json::to_string(&o).unwrap();
        let duplicate = text.replacen('{', "{\"engine\":\"podman\",", 1);
        assert!(serde_json::from_str::<Observation>(&duplicate).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn actual_principal_cannot_be_replaced_by_self_consistent_caller_values() {
        let mut c = context();
        c.principal_uid = unsafe { libc::geteuid() };
        c.match_current_operation(&c, &c.observation, &c.observation)
            .unwrap();
        c.principal_uid = c.principal_uid.wrapping_add(1);
        assert!(c
            .match_current_operation(&c, &c.observation, &c.observation)
            .is_err());
    }

    #[test]
    fn different_valid_socket_path_is_not_the_same_context() {
        let c = context();
        let mut changed = c.observation.clone();
        changed.socket.path = "/run/podman/other.sock".into();
        changed.validate().unwrap();
        assert!(c.match_operation(&c, &changed, &changed).is_err());
    }

    #[test]
    fn unsafe_paths_and_socket_types_are_rejected() {
        for path in [
            "relative",
            "/run/../socket",
            "/run//socket",
            "/run/socket/",
            "/run/so\0cket",
        ] {
            let mut c = context();
            c.observation.socket.path = path.into();
            assert!(c.validate().is_err());
        }
        for mode in [0o100600, 0o140666, 0o144600] {
            let mut c = context();
            c.observation.socket.mode = mode;
            assert!(c.validate().is_err());
        }
    }
    #[test]
    fn local_socket_owner_policy_is_explicit() {
        let mut c = context();
        c.principal_uid = 1000;
        c.observation.rootless = false;
        c.observation.socket.uid = 0;
        assert!(c.validate_local_socket_policy().is_ok());
        c.observation.socket.uid = 1000;
        assert!(c.validate_local_socket_policy().is_err());
        c.observation.rootless = true;
        assert!(c.validate_local_socket_policy().is_ok());
        c.observation.socket.uid = 1001;
        assert!(c.validate_local_socket_policy().is_err());
        c.principal_uid = 0;
        c.observation.socket.uid = 0;
        assert!(c.validate_local_socket_policy().is_err());
    }
}
