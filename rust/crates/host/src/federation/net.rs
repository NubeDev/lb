//! Pre-connect `net:*` enforcement (datasources scope). A source connects only to a `host:port` the
//! admin approved at install (`net:tls:host:5432:connect`). The grant is the federation extension's
//! **install grant** (`requested ∩ admin_approved`, persisted as the `Install` record). A source
//! whose endpoint the grant omits is refused — opaque, even with the binary present (the headline
//! reference-extension deny). Core never opens the socket; this is the host gate IN FRONT of the
//! sidecar that does.
//!
//! Net caps are matched on their CANONICAL colon form (`net:tls:HOST:PORT:connect`), split on `:`
//! with a per-part `*` wildcard. This is dot-safe — the generic caps grammar splits a resource on
//! `.` too (for `ext.tool`), which would shred an IP/hostname; net matching must not.

use lb_assets::read_install;
use lb_store::Store;

use super::error::FederationError;

/// The federation extension id — its install grant carries the admin-approved `net:*` set.
pub const FEDERATION_EXT: &str = "federation";

/// Refuse unless `endpoint` (`host:port`) is permitted by the federation install's `net:*` grant in
/// `ws`. No install / no grant → refused (opaque).
pub async fn enforce_endpoint(
    store: &Store,
    ws: &str,
    endpoint: &str,
) -> Result<(), FederationError> {
    let (host, port) = split_endpoint(endpoint)?;
    let install = read_install(store, ws, FEDERATION_EXT)
        .await?
        .ok_or(FederationError::EndpointRefused)?;

    if install
        .granted
        .iter()
        .any(|g| net_grant_permits(g, host, port))
    {
        Ok(())
    } else {
        Err(FederationError::EndpointRefused)
    }
}

/// Does the `net:tls:<host>:<port>:connect` grant permit a TLS connect to `host:port`? Splits on
/// `:` (dot-safe) and matches each part literally or via a `*` wildcard. A non-net / malformed grant
/// permits nothing.
pub fn net_grant_permits(grant: &str, host: &str, port: &str) -> bool {
    // Expected shape: net : tls : <host> : <port> : connect  (5 colon-parts).
    let parts: Vec<&str> = grant.split(':').collect();
    if parts.len() != 5 {
        return false;
    }
    let [surface, scheme, ghost, gport, action] =
        [parts[0], parts[1], parts[2], parts[3], parts[4]];
    surface == "net"
        && (scheme == "tls" || scheme == "*")
        && (action == "connect" || action == "*")
        && wild(ghost, host)
        && wild(gport, port)
}

/// A single-part wildcard match: `*` matches anything, else exact.
fn wild(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern == value
}

/// Split an `host:port` endpoint into its parts. Rejects a malformed endpoint (no port).
fn split_endpoint(endpoint: &str) -> Result<(&str, &str), FederationError> {
    endpoint
        .rsplit_once(':')
        .filter(|(h, p)| !h.is_empty() && !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .ok_or_else(|| FederationError::BadInput(format!("bad endpoint: {endpoint}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_grant_permits_its_endpoint() {
        assert!(net_grant_permits(
            "net:tls:127.0.0.1:49019:connect",
            "127.0.0.1",
            "49019"
        ));
    }

    #[test]
    fn specific_grant_refuses_other_endpoint() {
        assert!(!net_grant_permits(
            "net:tls:127.0.0.1:49019:connect",
            "127.0.0.1",
            "59999"
        ));
        assert!(!net_grant_permits(
            "net:tls:tsdb.acme:5432:connect",
            "evil.example",
            "5432"
        ));
    }

    #[test]
    fn wildcard_grant_permits_any() {
        assert!(net_grant_permits("net:tls:*:*:connect", "any.host", "1234"));
    }

    #[test]
    fn non_net_grant_permits_nothing() {
        assert!(!net_grant_permits("secret:federation/*:get", "h", "1"));
    }
}
