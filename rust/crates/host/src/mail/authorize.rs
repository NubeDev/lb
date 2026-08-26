//! The mail-source capability gate.
//!
//! Every `mail.source.<verb>` is gated `mcp:mail.source.<verb>:call` through the shared
//! `lb_mcp::authorize_tool` chokepoint — workspace first (§7), then capability (§3.5). Nothing here
//! is special-cased.
//!
//! **Why the whole family is admin-tier.** Registering a mail source does two things a member must
//! not be able to do unilaterally: it opens an **external ingress** into the workspace (anyone who
//! can email the address can put documents and series data in front of the workspace's agents), and
//! it **spends storage and network** on a schedule nobody approved. The mail-source scope names the
//! mailbox-as-attack-surface risk explicitly; the allowlist is the containment and the admin gate is
//! who gets to set it.
//!
//! Separately: the caps the *poll* runs under are NOT these. See [`principal`](super::principal) —
//! the importer holds a deliberately narrow bundle that cannot read the corpus back.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::MailSourceError;

/// Authorize `mail.source.<verb>` in workspace `ws` for `principal`.
pub fn authorize_mail_source(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), MailSourceError> {
    let tool = format!("mail.source.{verb}");
    authorize_tool(principal, ws, &tool).map_err(|_| MailSourceError::Denied)
}
