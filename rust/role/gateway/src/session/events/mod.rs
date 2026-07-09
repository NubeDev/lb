//! The **unified event stream** — one multiplexed SSE connection per browser session, every live feed a
//! *subject* riding it (unified-event-stream scope). Fixes the SSE-pool-exhaustion defect
//! (`debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md`): instead of one
//! `EventSource` per run/channel/series/… spending one of the browser's ~6 HTTP/1.1 slots, ALL feeds
//! multiplex onto one connection, freeing the rest for REST.
//!
//! Two files, one responsibility each (FILE-LAYOUT §4):
//!   - [`subject`] — the registry mapping an opaque `kind:id` subject to the SAME `lb_host` gate +
//!     snapshot + live feed its dedicated route uses (the gate is reused, never re-implemented).
//!   - [`hub`] — the connection-scoped, ephemeral subscription registry (mint sid, subscribe/unsubscribe
//!     tasks, drop-on-close). No durable state.
//!
//! The HTTP surface (`GET /events/stream`, `POST /events/{sid}/{subscribe,unsubscribe}`) lives in
//! `routes/events.rs`, thin glue over [`EventHub`].

pub mod hub;
pub mod subject;

pub use hub::{EventHub, NoSuchConn};
