//! Pure native v6.1.4 inspect response checks. No transport or lease authority.
//! The caller must bind observations, protected configuration and the held socket.
use super::{native_json, observation::API};
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq)]
pub enum InspectOutcome {
    Present(State),
    /// Exact SQLite lookup error only; not permission to release a lease.
    Missing,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct State {
    pub status: String,
    pub running: bool,
    pub paused: bool,
    pub restarting: bool,
    pub dead: bool,
    pub exit_code: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Inspect {
    id: String,
    state: State,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    cause: String,
    message: String,
    response: u16,
}

/// Only full lowercase IDs: no names, prefixes, escaping, queries or discovery.
pub fn inspect_route(id: &str) -> Result<String, &'static str> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("full container ID required");
    }
    Ok(format!("/v{API}/libpod/containers/{id}/json"))
}

/// Validates selected state fields only. Image/labels/ports and mutation policy
/// remain separate integration gates; Present does not establish ownership.
pub fn decode_inspect(
    id: &str,
    method: &str,
    route: &str,
    status: u16,
    bytes: &[u8],
) -> Result<InspectOutcome, &'static str> {
    if method != "GET" || route != inspect_route(id)? {
        return Err("exact native inspect request required");
    }
    let value = native_json::decode(bytes)?;
    match status {
        200 => {
            let inspect: Inspect =
                serde_json::from_value(value).map_err(|_| "inspect fields rejected")?;
            if inspect.id != id {
                return Err("inspect target mismatch");
            }
            let s = inspect.state;
            // These supported states are deliberately narrower than every Libpod
            // state. Unknown/removing/bad states are unavailable, never absent.
            if !matches!(
                s.status.as_str(),
                "initialized" | "created" | "running" | "stopped" | "exited" | "paused"
            ) || s.running != (s.status == "running")
                || s.paused != (s.status == "paused")
                || s.restarting
                || s.dead
            {
                return Err("unsupported or inconsistent native state");
            }
            Ok(InspectOutcome::Present(s))
        }
        404 => {
            let error: Failure =
                serde_json::from_value(value).map_err(|_| "inspect error fields rejected")?;
            let expected =
                format!("no container with name or ID \"{id}\" found: no such container");
            if error.response != 404
                || error.cause != "no such container"
                || error.message != expected
            {
                return Err("inspect absence not proved");
            }
            Ok(InspectOutcome::Missing)
        }
        _ => Err("native inspect unavailable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn id() -> String {
        "a".repeat(64)
    }
    fn present(state: &str) -> Value {
        json!({"Id":id(),"State":{"Status":state,"Running":state=="running","Paused":state=="paused","Restarting":false,"Dead":false,"ExitCode":0},"Metric":{"float":1.25}})
    }
    fn missing() -> Value {
        json!({"cause":"no such container","message":format!("no container with name or ID \"{}\" found: no such container",id()),"response":404})
    }
    fn parse(status: u16, v: &Value) -> Result<InspectOutcome, &'static str> {
        decode_inspect(
            &id(),
            "GET",
            &inspect_route(&id()).unwrap(),
            status,
            &serde_json::to_vec(v).unwrap(),
        )
    }
    #[test]
    fn full_id_native_route_only() {
        for bad in [
            "fixture".into(),
            "a".repeat(63),
            "A".repeat(64),
            "g".repeat(64),
            format!("{}/", id()),
        ] {
            assert!(inspect_route(&bad).is_err());
        }
        let body = serde_json::to_vec(&missing()).unwrap();
        let route = inspect_route(&id()).unwrap();
        for wrong in [
            route.replace("/libpod", ""),
            format!("{route}?size=true"),
            route.replace("6.1.4", "6.1.3"),
            route.replace(&id(), &"b".repeat(64)),
        ] {
            assert!(decode_inspect(&id(), "GET", &wrong, 404, &body).is_err());
        }
        assert!(decode_inspect(&id(), "DELETE", &route, 404, &body).is_err());
    }
    #[test]
    fn supported_present_states_and_exact_missing() {
        for state in [
            "initialized",
            "created",
            "running",
            "paused",
            "stopped",
            "exited",
        ] {
            assert!(matches!(
                parse(200, &present(state)),
                Ok(InspectOutcome::Present(_))
            ));
        }
        assert_eq!(parse(404, &missing()).unwrap(), InspectOutcome::Missing);
    }
    #[test]
    fn missing_fields_wrong_target_and_inconsistent_state_refused() {
        let mut v = present("running");
        v["Id"] = "b".repeat(64).into();
        assert!(parse(200, &v).is_err());
        for field in [
            "Status",
            "Running",
            "Paused",
            "Restarting",
            "Dead",
            "ExitCode",
        ] {
            let mut v = present("running");
            v["State"].as_object_mut().unwrap().remove(field);
            assert!(parse(200, &v).is_err());
        }
        for field in ["Running", "Paused", "Restarting", "Dead"] {
            let mut v = present("running");
            v["State"][field] = (!v["State"][field].as_bool().unwrap()).into();
            assert!(parse(200, &v).is_err());
        }
        for state in [
            "configured",
            "unknown",
            "bad state",
            "removing",
            "stopping",
            "",
        ] {
            assert!(parse(200, &present(state)).is_err());
        }
    }
    #[test]
    fn generic_docker_ambiguous_wrong_id_and_status_errors_are_not_absence() {
        for field in ["cause", "message", "response"] {
            let mut v = missing();
            v.as_object_mut().unwrap().remove(field);
            assert!(parse(404, &v).is_err());
            let mut v = missing();
            v[field] = Value::Null;
            assert!(parse(404, &v).is_err());
        }
        for message in [
            "not found".into(),
            format!("No such container: {}", id()),
            missing()["message"]
                .as_str()
                .unwrap()
                .replace(&id(), &"b".repeat(64)),
        ] {
            let mut v = missing();
            v["message"] = message.into();
            assert!(parse(404, &v).is_err());
        }
        let mut v = missing();
        v["cause"] = "container already exists".into();
        assert!(parse(404, &v).is_err());
        let mut v = missing();
        v["response"] = 500.into();
        assert!(parse(404, &v).is_err());
        let mut v = missing();
        v["extra"] = true.into();
        assert!(parse(404, &v).is_err());
        for status in [200, 204, 301, 400, 401, 403, 409, 500, 503] {
            assert!(parse(status, &missing()).is_err());
        }
        assert!(parse(404, &present("running")).is_err());
    }
    #[test]
    fn duplicate_trailing_oversize_and_deep_bodies_refused() {
        let good = serde_json::to_string(&present("running")).unwrap();
        let bodies = vec![
            good.replace("\"Running\":true", "\"Running\":true,\"Running\":true")
                .into_bytes(),
            good.replace("1.25", "1.25,\"float\":2").into_bytes(),
            format!("{good} {{}}").into_bytes(),
            vec![b' '; native_json::LIMIT + 1],
            format!("{}0{}", "[".repeat(200), "]".repeat(200)).into_bytes(),
            Vec::new(),
        ];
        for bytes in bodies {
            assert!(
                decode_inspect(&id(), "GET", &inspect_route(&id()).unwrap(), 200, &bytes).is_err()
            );
        }
    }
}
