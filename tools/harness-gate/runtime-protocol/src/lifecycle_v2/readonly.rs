//! Read-only info/inspect/info composition over an injected GET exchange.
//! The exchange must supply bounded, deadline-controlled HTTP transport. This
//! module does not prove a connection's identity, authentication or lifetime.
use super::{
    native_info::decode_info,
    native_status::{decode_inspect, inspect_route, InspectOutcome},
    observation::INFO_ROUTE,
    protected_context::ProtectedContext,
};

impl ProtectedContext {
    /// No retry, mutation or lease authority. Even an exact Missing result is
    /// withheld until the final info observation and filesystem check succeed.
    pub fn inspect_with<F>(&self, id: &str, mut get: F) -> Result<InspectOutcome, &'static str>
    where
        F: FnMut(&str) -> Result<(u16, Vec<u8>), &'static str>,
    {
        let route = inspect_route(id)?;
        let (expected, _) = self.snapshot()?;
        expected.validate_local_socket_policy()?;
        let exchange = |get: &mut F, path: &str| {
            self.revalidate()?;
            let response = get(path)?;
            self.revalidate()?;
            Ok::<_, &'static str>(response)
        };
        let (status, bytes) = exchange(&mut get, INFO_ROUTE)?;
        if status != 200 {
            return Err("initial native info unavailable");
        }
        let before = decode_info(&bytes, expected.observation.socket.clone())?;
        expected.match_current_operation(expected, &before, &before)?;
        let (status, bytes) = exchange(&mut get, &route)?;
        let result = decode_inspect(id, "GET", &route, status, &bytes)?;
        let (status, bytes) = exchange(&mut get, INFO_ROUTE)?;
        if status != 200 {
            return Err("final native info unavailable");
        }
        let after = decode_info(&bytes, expected.observation.socket.clone())?;
        expected.match_current_operation(expected, &before, &after)?;
        self.revalidate()?;
        Ok(result)
    }
}
