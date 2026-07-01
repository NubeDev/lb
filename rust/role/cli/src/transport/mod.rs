//! The `Transport` seam (operator-cli scope, decision #2): "two modes share one command tree; the
//! only difference is the transport." Every command is `transport.call(tool, args)`; the two impls —
//! [`Remote`](remote::Remote) (a reqwest client over the gateway) and [`Local`](local::Local) (an
//! in-process `Node` + a minted `Principal`) — both end at `lb_host::call_tool`. This is the
//! symmetric-nodes rule made literal: the same binary, the same verbs, the same auth path, differing
//! only by where the host runs. There is no `if privileged` branch; both are exactly as authorized as
//! the token/principal they carry.

mod local;
mod publish;
mod remote;

pub use local::Local;
pub use publish::{ExtPublish, PublishOutcome};
pub use remote::Remote;

use serde_json::Value;

use crate::error::CliResult;
use crate::header::Header;

/// Either transport, as one concrete value — so the dispatcher builds one `AnyTransport` (a config/flag
/// choice, not a code branch on role) and hands it to a command as `impl Transport`. An enum rather
/// than `Box<dyn>` because the trait's `async fn call` is not object-safe-with-Send cleanly, and the
/// two-variant enum is smaller and faster anyway.
pub enum AnyTransport {
    Remote(Remote),
    Local(Local),
}

impl Transport for AnyTransport {
    fn header(&self) -> Header {
        match self {
            AnyTransport::Remote(t) => t.header(),
            AnyTransport::Local(t) => t.header(),
        }
    }

    fn caps(&self) -> Vec<String> {
        match self {
            AnyTransport::Remote(t) => t.caps(),
            AnyTransport::Local(t) => t.caps(),
        }
    }

    async fn call(&self, tool: &str, args: Value) -> CliResult<Value> {
        match self {
            AnyTransport::Remote(t) => t.call(tool, args).await,
            AnyTransport::Local(t) => t.call(tool, args).await,
        }
    }
}

/// A client transport: authenticate, then call one MCP tool and return its JSON result. Async over the
/// `async_trait`-free `impl Future` style is avoided (object safety) — this uses `async_trait`'s hand
/// pattern via a boxed future is not needed since we dispatch on a concrete enum at the call site. To
/// keep it object-safe and simple, the trait exposes an async method through the 2024 native
/// `async fn in trait` (the workspace's edition supports it for our own crate's private trait).
#[allow(async_fn_in_trait)]
pub trait Transport {
    /// The identity header this transport runs under (`ws`/`user`/`role`), for the legibility line
    /// every command prints. Remote decodes it from the token; local reads the minted principal.
    fn header(&self) -> Header;

    /// The capabilities the session carries — `whoami` renders these so an operator sees which verbs
    /// they may reach before running one. Remote decodes them from the token; local reads the minted
    /// principal. Never includes the token itself (caps are not secret; the bearer is).
    fn caps(&self) -> Vec<String>;

    /// Call `tool` with JSON `args`, returning the tool's JSON result. A server/host DENY becomes
    /// [`CliError::Denied`](crate::error::CliError::Denied); a down gateway becomes
    /// [`CliError::Transport`](crate::error::CliError::Transport) — never a fabricated success.
    async fn call(&self, tool: &str, args: Value) -> CliResult<Value>;
}
