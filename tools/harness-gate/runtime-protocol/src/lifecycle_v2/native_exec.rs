//! Source-pinned native mutation wire helpers; no dispatch or retry authority.
//! Callers must durably reserve each mutation before sending, retain ambiguous
//! outcomes, and never reconstruct/replay create or start after response loss.
use super::{native_json, native_status::inspect_route, observation::API};
use serde::{Deserialize, Serialize};
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug)]
pub struct HealthPlan {
    container: String,
    argv: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ExecCreate<'a> {
    user: &'a str,
    privileged: bool,
    tty: bool,
    attach_stdin: bool,
    attach_stderr: bool,
    attach_stdout: bool,
    detach_keys: &'a str,
    env: [&'a str; 0],
    working_dir: &'a str,
    cmd: &'a [String],
}
impl HealthPlan {
    pub fn new(container: &str, argv: Vec<String>) -> Result<Self> {
        inspect_route(container)?;
        if argv.is_empty()
            || argv.len() > 128
            || argv[0].is_empty()
            || argv.iter().any(|a| a.len() > 8192 || a.contains('\0'))
            || argv.iter().map(String::len).sum::<usize>() > 32768
        {
            return Err("bounded explicit health argv required");
        }
        Ok(Self {
            container: container.into(),
            argv,
        })
    }
    pub fn create_route(&self) -> String {
        format!("/v{API}/libpod/containers/{}/exec", self.container)
    }
    pub fn create_body(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(&ExecCreate {
            user: "",
            privileged: false,
            tty: false,
            attach_stdin: false,
            attach_stderr: false,
            attach_stdout: false,
            detach_keys: "",
            env: [],
            working_dir: "",
            cmd: &self.argv,
        })
        .map_err(|_| "exec encoding failed")
    }
    pub fn bind_created(&self, status: u16, body: &[u8]) -> Result<String> {
        let v = decode_created(status, body, false)?;
        Ok(v)
    }
    /// Match exact exec and container, selected process fields and terminal state.
    /// CanRemove is necessary: an unstarted exec also reports Running=false/0.
    pub fn inspect(&self, exec: &str, status: u16, body: &[u8]) -> Result<HealthObservation> {
        exec_route(exec, false)?;
        if status != 200 {
            return Err("exec inspect unavailable");
        }
        let i: ExecInspect = serde_json::from_value(native_json::decode(body)?)
            .map_err(|_| "exec inspect fields rejected")?;
        let p = i.process_config;
        if i.id != exec
            || i.container_id != self.container
            || i.open_stdin
            || i.open_stdout
            || i.open_stderr
            || p.privileged
            || p.tty
            || !p.user.is_empty()
            || p.entrypoint != self.argv[0]
            || p.arguments.unwrap_or_default() != self.argv[1..]
        {
            return Err("exec identity or process mismatch");
        }
        if i.running {
            if i.can_remove {
                return Err("inconsistent exec state");
            }
            return Ok(HealthObservation::Pending);
        }
        if !i.can_remove {
            return Ok(HealthObservation::Pending);
        }
        if !(0..=255).contains(&i.exit_code) {
            return Err("invalid exec exit code");
        }
        Ok(HealthObservation::Completed(i.exit_code as u8))
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum HealthObservation {
    Pending,
    Completed(u8),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdResponse {
    #[serde(rename = "Id")]
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateResponse {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Warnings")]
    warnings: Vec<String>,
}
/// Container response only. Nonempty or null/missing warnings fail closed;
/// native create request, ownership inspect and durable unknown retention remain gates.
pub fn decode_created(status: u16, body: &[u8], container: bool) -> Result<String> {
    if status != 201 {
        return Err("mutation response not created");
    }
    let value = native_json::decode(body)?;
    let id = if container {
        let r: CreateResponse =
            serde_json::from_value(value).map_err(|_| "container create fields rejected")?;
        if !r.warnings.is_empty() {
            return Err("container create warnings require review");
        }
        r.id
    } else {
        serde_json::from_value::<IdResponse>(value)
            .map_err(|_| "exec create fields rejected")?
            .id
    };
    inspect_route(&id)?;
    Ok(id)
}
pub fn exec_route(exec: &str, start: bool) -> Result<String> {
    inspect_route(exec)?;
    Ok(format!(
        "/v{API}/libpod/exec/{exec}/{}",
        if start { "start" } else { "json" }
    ))
}
pub const DETACHED_START_BODY: &[u8] = br#"{"Detach":true,"Tty":false,"h":0,"w":0}"#;
/// Empty 200 acknowledges start only, never health completion.
pub fn check_started(status: u16, body: &[u8]) -> Result<()> {
    if status == 200 && body.is_empty() {
        Ok(())
    } else {
        Err("exec start outcome unknown")
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ExecInspect {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "ContainerID")]
    container_id: String,
    can_remove: bool,
    running: bool,
    exit_code: i32,
    open_stdin: bool,
    open_stdout: bool,
    open_stderr: bool,
    process_config: Process,
}
#[derive(Deserialize)]
struct Process {
    entrypoint: String,
    #[serde(deserialize_with = "required_arguments")]
    arguments: Option<Vec<String>>,
    privileged: bool,
    tty: bool,
    user: String,
}

// Native Go emits null for zero arguments. Missing is distinct from explicit null.
fn required_arguments<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Vec<String>>, D::Error> {
    Option::<Vec<String>>::deserialize(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn cid() -> String {
        "a".repeat(64)
    }
    fn eid() -> String {
        "b".repeat(64)
    }
    fn plan() -> HealthPlan {
        HealthPlan::new(&cid(), vec!["/bin/check".into(), "arg".into()]).unwrap()
    }
    fn fixture() -> Value {
        json!({"ID":eid(),"ContainerID":cid(),"CanRemove":true,"Running":false,"ExitCode":0,"OpenStdin":false,"OpenStdout":false,"OpenStderr":false,"ProcessConfig":{"entrypoint":"/bin/check","arguments":["arg"],"privileged":false,"tty":false,"user":""}})
    }
    fn inspect(v: &Value) -> Result<HealthObservation> {
        plan().inspect(&eid(), 200, &serde_json::to_vec(v).unwrap())
    }
    #[test]
    fn exact_request_vectors() {
        assert_eq!(plan().create_body().unwrap(),br#"{"User":"","Privileged":false,"Tty":false,"AttachStdin":false,"AttachStderr":false,"AttachStdout":false,"DetachKeys":"","Env":[],"WorkingDir":"","Cmd":["/bin/check","arg"]}"#);
        assert_eq!(
            plan().create_route(),
            format!("/v6.1.4/libpod/containers/{}/exec", cid())
        );
        assert_eq!(
            exec_route(&eid(), true).unwrap(),
            format!("/v6.1.4/libpod/exec/{}/start", eid())
        );
        assert_eq!(
            exec_route(&eid(), false).unwrap(),
            format!("/v6.1.4/libpod/exec/{}/json", eid())
        );
        assert_eq!(
            DETACHED_START_BODY,
            br#"{"Detach":true,"Tty":false,"h":0,"w":0}"#
        );
    }
    #[test]
    fn created_identity_and_warning_boundaries() {
        for container in [false, true] {
            let mut v = json!({"Id":eid()});
            if container {
                v["Warnings"] = json!([]);
            }
            assert_eq!(
                decode_created(201, &serde_json::to_vec(&v).unwrap(), container).unwrap(),
                eid()
            );
            for status in [200, 204, 400, 404, 409, 500] {
                assert!(
                    decode_created(status, &serde_json::to_vec(&v).unwrap(), container).is_err()
                );
            }
            for bad in ["name".into(), "b".repeat(63), "B".repeat(64)] {
                v["Id"] = bad.into();
                assert!(decode_created(201, &serde_json::to_vec(&v).unwrap(), container).is_err());
            }
        }
        for w in [Value::Null, json!(["warning"]), json!(false)] {
            assert!(decode_created(
                201,
                &serde_json::to_vec(&json!({"Id":eid(),"Warnings":w})).unwrap(),
                true
            )
            .is_err());
        }
        assert!(decode_created(
            201,
            &serde_json::to_vec(&json!({"Id":eid()})).unwrap(),
            true
        )
        .is_err());
        assert!(decode_created(
            201,
            format!("{{\"Id\":\"{}\",\"Id\":\"{}\"}}", eid(), eid()).as_bytes(),
            false
        )
        .is_err());
    }
    #[test]
    fn start_ack_is_exact_not_health() {
        assert!(check_started(200, b"").is_ok());
        for status in [201, 204, 404, 409, 500] {
            assert!(check_started(status, b"").is_err());
        }
        for body in [b"\n".as_slice(), b"{}", b"null"] {
            assert!(check_started(200, body).is_err());
        }
    }
    #[test]
    fn unstarted_and_running_are_not_health_success() {
        let mut v = fixture();
        v["CanRemove"] = false.into();
        assert_eq!(inspect(&v).unwrap(), HealthObservation::Pending);
        v["Running"] = true.into();
        assert_eq!(inspect(&v).unwrap(), HealthObservation::Pending);
        v["CanRemove"] = true.into();
        assert!(inspect(&v).is_err());
        for code in [0, 1, 7, 255] {
            let mut v = fixture();
            v["ExitCode"] = code.into();
            assert_eq!(
                inspect(&v).unwrap(),
                HealthObservation::Completed(code as u8)
            );
        }
    }
    #[test]
    fn selected_fields_and_command_identity_are_required() {
        for key in [
            "ID",
            "ContainerID",
            "CanRemove",
            "Running",
            "ExitCode",
            "OpenStdin",
            "OpenStdout",
            "OpenStderr",
            "ProcessConfig",
        ] {
            let mut v = fixture();
            v.as_object_mut().unwrap().remove(key);
            assert!(inspect(&v).is_err());
        }
        for key in ["ID", "ContainerID"] {
            let mut v = fixture();
            v[key] = "c".repeat(64).into();
            assert!(inspect(&v).is_err());
        }
        for key in ["OpenStdin", "OpenStdout", "OpenStderr"] {
            let mut v = fixture();
            v[key] = true.into();
            assert!(inspect(&v).is_err());
        }
        for (key, value) in [
            ("entrypoint", json!("other")),
            ("arguments", json!([])),
            ("privileged", json!(true)),
            ("tty", json!(true)),
            ("user", json!("root")),
        ] {
            let mut v = fixture();
            v["ProcessConfig"][key] = value;
            assert!(inspect(&v).is_err());
        }
        for code in [-1, 256] {
            let mut v = fixture();
            v["ExitCode"] = code.into();
            assert!(inspect(&v).is_err());
        }
    }
    #[test]
    fn request_bounds_and_short_ids_refused() {
        assert!(HealthPlan::new("name", vec!["true".into()]).is_err());
        for argv in [
            vec![],
            vec!["".into()],
            vec!["bad\0arg".into()],
            vec!["x".repeat(8193)],
            vec!["x".into(); 129],
        ] {
            assert!(HealthPlan::new(&cid(), argv).is_err());
        }
        assert!(exec_route("short", true).is_err());
    }
    #[test]
    fn zero_arguments_accepts_explicit_null_but_not_missing() {
        let p = HealthPlan::new(&cid(), vec!["/bin/check".into()]).unwrap();
        let mut v = fixture();
        v["ProcessConfig"]["arguments"] = Value::Null;
        assert_eq!(
            p.inspect(&eid(), 200, &serde_json::to_vec(&v).unwrap())
                .unwrap(),
            HealthObservation::Completed(0)
        );
        v["ProcessConfig"]
            .as_object_mut()
            .unwrap()
            .remove("arguments");
        assert!(p
            .inspect(&eid(), 200, &serde_json::to_vec(&v).unwrap())
            .is_err());
    }
    #[test]
    fn health_duplicates_status_and_missing_process_fields_refused() {
        for key in ["entrypoint", "arguments", "privileged", "tty", "user"] {
            let mut v = fixture();
            v["ProcessConfig"].as_object_mut().unwrap().remove(key);
            assert!(inspect(&v).is_err());
        }
        let raw = serde_json::to_string(&fixture()).unwrap().replace(
            "\"privileged\":false",
            "\"privileged\":false,\"privileged\":true",
        );
        assert!(plan().inspect(&eid(), 200, raw.as_bytes()).is_err());
        for status in [201, 204, 404, 409, 500] {
            assert!(plan()
                .inspect(&eid(), status, &serde_json::to_vec(&fixture()).unwrap())
                .is_err());
        }
    }
}
