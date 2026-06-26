//! Capabilities — the actual core product (README §11.1, auth-caps scope).
//!
//! One identity → one capability set that projects onto all three enforcement surfaces
//! (store, bus, mcp) plus secrets. The single chokepoint is [`check`]: it runs two gates in
//! order — **workspace isolation first** (the hard wall, §3.6), then the **capability**
//! pattern match (§3.5). There is no other path to a resource.

mod check;
mod grammar;
mod request;

pub use check::{check, Decision, Denied};
pub use grammar::{matches, Capability, ParseError};
pub use request::{Action, Request, Surface};
