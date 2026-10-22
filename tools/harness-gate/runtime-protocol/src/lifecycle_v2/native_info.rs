//! Bounded native Libpod info body decoder, pinned to v6.1.4 Go JSON fields.
//! HTTP route/status/framing and held socket verification are caller obligations.
use super::observation::{Observation, RemoteSocketObservation, SocketObservation, API};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;

const LIMIT: usize = 1024 * 1024;
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded native JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Strict, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| de::Error::custom("nonfinite number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(Strict(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom("duplicate field"));
                    }
                    let Strict(v) = a.next_value()?;
                    out.insert(k, v);
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}

// Native info carries many metrics unrelated to engine context. Accept extra
// fields but reject duplicate keys everywhere, including ignored metric objects.
#[derive(Deserialize)]
struct Info {
    host: Host,
    store: Store,
    version: Version,
}
#[derive(Deserialize)]
struct Host {
    hostname: String,
    security: Security,
    #[serde(rename = "remoteSocket")]
    remote_socket: Option<Remote>,
}
#[derive(Deserialize)]
struct Security {
    rootless: bool,
}
#[derive(Deserialize)]
struct Store {
    #[serde(rename = "graphRoot")]
    graph_root: String,
    #[serde(rename = "runRoot")]
    run_root: String,
}
#[derive(Deserialize)]
struct Version {
    #[serde(rename = "APIVersion")]
    api_version: String,
    #[serde(rename = "Version")]
    version: String,
}
#[derive(Deserialize)]
struct Remote {
    path: String,
    exists: bool,
}

/// Socket metadata is a separate locally observed input, never read from JSON.
/// A successful parse does not match protected expected context or authorize I/O.
pub fn decode_info(bytes: &[u8], socket: SocketObservation) -> Result<Observation, &'static str> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err("native info size rejected");
    }
    let Strict(value) = serde_json::from_slice(bytes).map_err(|_| "native info JSON rejected")?;
    if ["ID", "daemon_id", "operator_epoch"]
        .iter()
        .any(|key| value.get(key).is_some())
    {
        return Err("native info cannot supply identity authority");
    }
    let info: Info = serde_json::from_value(value).map_err(|_| "native info fields rejected")?;
    if info.version.api_version != API {
        return Err("native API version rejected");
    }
    let observation = Observation {
        engine: "podman".into(),
        api_surface: "libpod".into(),
        selected_api: API.into(),
        server_version: info.version.version,
        hostname: info.host.hostname,
        rootless: info.host.security.rootless,
        graph_root: info.store.graph_root,
        run_root: info.store.run_root,
        remote_socket: info.host.remote_socket.map(|r| RemoteSocketObservation {
            path: r.path,
            exists: r.exists,
        }),
        socket,
    };
    observation.validate()?;
    Ok(observation)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn socket() -> SocketObservation {
        SocketObservation {
            path: "/run/podman/podman.sock".into(),
            device: 1,
            inode: 2,
            uid: 0,
            mode: 0o140660,
        }
    }
    fn fixture() -> Value {
        serde_json::json!({"host":{"hostname":"fixture","security":{"rootless":false},"cpuUtilization":{"userPercent":1.25}},
            "store":{"graphRoot":"/var/lib/containers/storage","runRoot":"/run/containers/storage"},
            "version":{"APIVersion":"6.1.4","Version":"6.1.4","Built":0}})
    }
    #[test]
    fn native_fields_and_nonidentity_float_metrics() {
        let v = fixture();
        let o = decode_info(&serde_json::to_vec(&v).unwrap(), socket()).unwrap();
        assert_eq!(o.hostname, "fixture");
        assert!(!o.rootless);
        assert_eq!(o.selected_api, API);
    }
    #[test]
    fn selected_fields_missing_wrong_type_or_wrong_version_rejected() {
        for (section, field) in [
            ("host", "hostname"),
            ("host", "security"),
            ("store", "graphRoot"),
            ("store", "runRoot"),
            ("version", "APIVersion"),
            ("version", "Version"),
        ] {
            for remove in [true, false] {
                let mut v = fixture();
                if remove {
                    v[section].as_object_mut().unwrap().remove(field);
                } else {
                    v[section][field] = Value::Null;
                }
                assert!(decode_info(&serde_json::to_vec(&v).unwrap(), socket()).is_err());
            }
        }
        let mut v = fixture();
        v["version"]["Version"] = "6.1.5".into();
        assert!(decode_info(&serde_json::to_vec(&v).unwrap(), socket()).is_err());
    }
    #[test]
    fn duplicate_keys_in_selected_and_ignored_objects_rejected() {
        let text = serde_json::to_string(&fixture()).unwrap();
        for raw in [
            text.replace("\"hostname\":", "\"hostname\":\"other\",\"hostname\":"),
            text.replace("1.25", "1.25,\"userPercent\":2.5"),
        ] {
            assert!(decode_info(raw.as_bytes(), socket()).is_err());
        }
    }
    #[test]
    fn compat_and_fabricated_authority_rejected() {
        assert!(decode_info(
            br#"{"ID":"random","Name":"host","ServerVersion":"6.1.4"}"#,
            socket()
        )
        .is_err());
        for key in ["ID", "daemon_id", "operator_epoch"] {
            let mut v = fixture();
            v[key] = "random".into();
            assert!(decode_info(&serde_json::to_vec(&v).unwrap(), socket()).is_err());
        }
    }
    #[test]
    fn remote_socket_must_match_local_socket() {
        let mut v = fixture();
        v["host"]["remoteSocket"] = serde_json::json!({"path":socket().path,"exists":true});
        decode_info(&serde_json::to_vec(&v).unwrap(), socket()).unwrap();
        v["host"]["remoteSocket"]["path"] = "/run/other.sock".into();
        assert!(decode_info(&serde_json::to_vec(&v).unwrap(), socket()).is_err());
    }
    #[test]
    fn bounds_trailing_data_utf8_and_nonfinite_rejected() {
        for raw in [
            vec![b' '; LIMIT + 1],
            vec![0xff],
            b"{}{}".to_vec(),
            b"{\"metric\":NaN}".to_vec(),
        ] {
            assert!(decode_info(&raw, socket()).is_err());
        }
    }
}
