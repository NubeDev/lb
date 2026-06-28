//! `bus.publish(subject, payload) -> { ok }` — fire-and-forget workspace-walled motion (widget-config-
//! vars scope, "Platform fix"). State vs motion (rule 3): this is NOT durable — a must-deliver effect
//! still goes through the outbox. The subject is walled to `ws/{id}/ext/{subject}` host-side from the
//! token; the caller can never name another workspace's subject nor a reserved platform prefix.

use lb_auth::Principal;
use lb_bus::{publish, Bus};

use super::authorize::{authorize_bus, wall_subject};
use super::error::BusError;

/// Publish `payload` (an opaque JSON value, serialized) onto `subject` in `ws` as `principal`.
/// Authorizes `mcp:bus.publish:call` FIRST (workspace-first), then walls the subject. Best-effort —
/// the bus is fire-and-forget; success means "handed to the bus", never "delivered".
pub async fn bus_publish(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    subject: &str,
    payload: &[u8],
) -> Result<(), BusError> {
    authorize_bus(principal, ws, "bus.publish")?;
    let rel = wall_subject(subject)?;
    publish(bus, ws, &rel, payload)
        .await
        .map_err(|e| BusError::Bus(e.to_string()))
}
