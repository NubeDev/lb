//! `bus.watch(subject) -> stream` — subscribe to a workspace-walled subject (widget-config-vars scope,
//! "Platform fix"). Authorizes `mcp:bus.watch:call` FIRST (workspace-first), then walls the subject — so
//! a denied or cross-workspace caller never declares bus interest (the `403` the SSE route returns before
//! any stream opens). Backs the gateway's `GET /bus/{subject}/stream?token=` SSE route.

use lb_auth::Principal;
use lb_bus::{subscribe, Bus};

use super::authorize::{authorize_bus, wall_subject};
use super::error::BusError;
use super::subscribe::BusSub;

/// Subscribe to live payloads on `subject` in `ws` as `principal`. Gated `mcp:bus.watch:call`,
/// workspace-walled. Returns a [`BusSub`] the SSE route folds into an event stream.
pub async fn bus_watch(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    subject: &str,
) -> Result<BusSub, BusError> {
    authorize_bus(principal, ws, "bus.watch")?;
    let rel = wall_subject(subject)?;
    let inner = subscribe(bus, ws, &rel)
        .await
        .map_err(|e| BusError::Bus(e.to_string()))?;
    Ok(BusSub::new(inner))
}
