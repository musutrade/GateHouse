//! Read-only info/inspect/info composition over an injected GET exchange.
//! The exchange must supply bounded, deadline-controlled HTTP transport. This
//! module does not prove a connection's identity, authentication or lifetime.
use super::{
    native_image::{decode_image_for, image_route_for, ImageFacts},
    native_info::decode_info_read_only,
    native_status::{decode_inspect, inspect_route, InspectOutcome, State},
    observation::{info_route, read_only_api, API},
    ownership::ExpectedContainer,
    protected_context::ProtectedContext,
};

/// Evidence of the protected observation sequence, not transport authentication,
/// freshness, restart continuity, installation verification or dispatch authority.
/// There is deliberately no public constructor or deserializer. The injected
/// exchange remains a trust boundary and can supply fabricated HTTP responses.
pub struct CapturedImage {
    facts: ImageFacts,
    image_bytes: Vec<u8>,
    config_bytes: Vec<u8>,
    epoch: String,
}
impl std::fmt::Debug for CapturedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapturedImage").finish_non_exhaustive()
    }
}
impl CapturedImage {
    pub fn facts(&self) -> &ImageFacts {
        &self.facts
    }
    pub fn image_bytes(&self) -> &[u8] {
        &self.image_bytes
    }
    pub fn config_bytes(&self) -> &[u8] {
        &self.config_bytes
    }
    pub fn operator_epoch(&self) -> &str {
        &self.epoch
    }
    /// Same exact context only; socket recreation/restart is not adopted.
    pub fn match_context(&self, context: &ProtectedContext) -> Result<(), &'static str> {
        let (expected, bytes) = context.snapshot()?;
        if bytes != self.config_bytes || expected.operator_epoch != self.epoch {
            return Err("captured image context mismatch");
        }
        Ok(())
    }
}

impl ProtectedContext {
    /// Capture exact bounded evidence only after protected info/image/info.
    /// An eventual record constructor must accept this type, never ImageFacts.
    pub fn capture_image_for_plan<F>(&self, id: &str, get: F) -> Result<CapturedImage, &'static str>
    where
        F: FnMut(&str) -> Result<(u16, Vec<u8>), &'static str>,
    {
        let (expected, bytes) = self.snapshot()?;
        let config_bytes = bytes.to_vec();
        let epoch = expected.operator_epoch.clone();
        let api = self.selected_read_only_api()?;
        let route = image_route_for(&api, id)?;
        let (facts, image_bytes) =
            self.inspect_checked_with_api(&api, &route, get, |route, status, bytes| {
                Ok((
                    decode_image_for(&api, id, "GET", route, status, bytes)?,
                    bytes.to_vec(),
                ))
            })?;
        let captured = CapturedImage {
            facts,
            image_bytes,
            config_bytes,
            epoch,
        };
        captured.match_context(self)?;
        Ok(captured)
    }

    /// No retry, mutation or lease authority. Even an exact Missing result is
    /// withheld until the final info observation and filesystem check succeed.
    pub fn inspect_with<F>(&self, id: &str, get: F) -> Result<InspectOutcome, &'static str>
    where
        F: FnMut(&str) -> Result<(u16, Vec<u8>), &'static str>,
    {
        let api = self.selected_read_only_api()?;
        if api != API {
            return Err("5.7 container inspect is not supported");
        }
        self.inspect_checked_with_api(&api, &inspect_route(id)?, get, |route, status, bytes| {
            decode_inspect(id, "GET", route, status, bytes)
        })
    }
    /// Selected ownership facts under the same protected observation sequence.
    /// The expected plan still requires durable installation/slot binding.
    pub fn inspect_owned_with<F>(
        &self,
        owner: &ExpectedContainer,
        get: F,
    ) -> Result<State, &'static str>
    where
        F: FnMut(&str) -> Result<(u16, Vec<u8>), &'static str>,
    {
        owner.validate()?;
        let api = self.selected_read_only_api()?;
        if api != API {
            return Err("5.7 ownership inspect is not supported");
        }
        self.inspect_checked_with_api(
            &api,
            &inspect_route(&owner.id)?,
            get,
            |route, status, bytes| owner.match_inspect("GET", route, status, bytes),
        )
    }
    /// Image facts are returned only after matching both protected observations.
    pub fn inspect_image_with<F>(&self, id: &str, get: F) -> Result<ImageFacts, &'static str>
    where
        F: FnMut(&str) -> Result<(u16, Vec<u8>), &'static str>,
    {
        let api = self.selected_read_only_api()?;
        let route = image_route_for(&api, id)?;
        self.inspect_checked_with_api(&api, &route, get, |route, status, bytes| {
            decode_image_for(&api, id, "GET", route, status, bytes)
        })
    }
    fn selected_read_only_api(&self) -> Result<String, &'static str> {
        let (expected, _) = self.snapshot()?;
        let api = expected.observation.selected_api.clone();
        if !read_only_api(&api) || expected.observation.server_version != api {
            return Err("unsupported read-only Podman API");
        }
        Ok(api)
    }

    fn inspect_checked_with_api<F, T, D>(
        &self,
        api: &str,
        route: &str,
        mut get: F,
        decode: D,
    ) -> Result<T, &'static str>
    where
        F: FnMut(&str) -> Result<(u16, Vec<u8>), &'static str>,
        D: FnOnce(&str, u16, &[u8]) -> Result<T, &'static str>,
    {
        let (expected, _) = self.snapshot()?;
        if expected.observation.selected_api != api {
            return Err("native route API differs from protected context");
        }
        expected.validate_local_socket_policy_read_only()?;
        let info = info_route(api)?;
        let exchange = |get: &mut F, path: &str| {
            self.revalidate()?;
            let response = get(path)?;
            self.revalidate()?;
            Ok::<_, &'static str>(response)
        };
        let (status, bytes) = exchange(&mut get, &info)?;
        if status != 200 {
            return Err("initial native info unavailable");
        }
        let before = decode_info_read_only(&bytes, expected.observation.socket.clone(), api)?;
        expected.match_current_operation_read_only(expected, &before, &before)?;
        let (status, bytes) = exchange(&mut get, route)?;
        let result = decode(route, status, &bytes)?;
        let (status, bytes) = exchange(&mut get, &info)?;
        if status != 200 {
            return Err("final native info unavailable");
        }
        let after = decode_info_read_only(&bytes, expected.observation.socket.clone(), api)?;
        expected.match_current_operation_read_only(expected, &before, &after)?;
        self.revalidate()?;
        Ok(result)
    }
}
