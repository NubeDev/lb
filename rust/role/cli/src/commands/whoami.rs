//! `lb whoami` — who am I, in which workspace, with what role and caps. There is no `/whoami` route
//! (the CLI holds no authority the server must confirm); the answer is the transport's own session:
//! remote decodes the stored token's (unverified) claims, local reports the minted principal. The
//! token is NEVER shown — only the identity it carries.
//!
//! `whoami` never touches the network in remote mode (it reads the token it already holds), so it
//! works offline and cannot leak the secret. The body lists the caps the token/principal carries so an
//! operator can see, before running anything, which verbs they may reach.

use serde_json::{json, Value};

use crate::error::CliResult;
use crate::output::Format;

use super::Printed;

/// Render the caller's identity from a decoded/minted claim set: `sub`, `ws`, `role`, and the caps.
/// `header` is the transport's header (already `ws/user/role`); `caps` is the held capability list.
/// The token is not a parameter here — by construction it cannot be printed.
pub fn render(header: &crate::header::Header, caps: &[String], format: Format) -> CliResult<Printed> {
    let body: Value = json!({
        "user": header.user,
        "workspace": header.workspace,
        "role": header.role_label(),
        "caps": caps,
    });
    Printed::from_value(header, &body, format)
}
