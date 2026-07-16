//! Shared wire shapes for the `/auth/*` front door (email-login scope). One place so `/auth/login`,
//! `/auth/select`, and `/auth/switch` reply with the same envelope the rubix-ai client decodes into a
//! discriminated result.
//!
//! The full-session reply reuses the legacy `LoginReply` fields (`token/principal/workspace/caps`) so
//! the client's session-store code is unchanged, PLUS a `workspaces` roster (`{ws, name}`) so the
//! client learns the switcher list in the same round trip. The N-branch reply carries a `select_token`
//! + the roster and NO full token. The client tells them apart by which fields are present.

use lb_host::IdentityWorkspace;
use serde::Serialize;

/// One workspace in the login/select roster — the switcher/picker row. Re-exported shape of the
/// host's `IdentityWorkspace` so the wire type is owned here, not leaked from the host crate.
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceRow {
    pub ws: String,
    pub name: String,
}

impl From<IdentityWorkspace> for WorkspaceRow {
    fn from(w: IdentityWorkspace) -> Self {
        Self {
            ws: w.ws,
            name: w.name,
        }
    }
}

/// The reply from `/auth/login` (and the shape `/auth/select`/`/auth/switch` return on success). One
/// of two states, distinguished by presence:
///   - **full session** — `token`, `principal`, `workspace`, `caps` set; `select_token` absent.
///   - **select needed (N>1)** — `select_token` set; `token`/`principal`/`workspace`/`caps` absent.
/// `workspaces` (the roster) is present in BOTH so the client always learns the switcher list.
#[derive(Debug, Serialize)]
pub struct AuthReply {
    /// The full signed session token — present iff this is the full-session branch (1-workspace login,
    /// or a completed select/switch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The resolved principal (`user:ada`) — present with `token`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// The workspace the token is scoped to — present with `token`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The caps the token carries (UI cap-gate convenience, never the boundary) — present with `token`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Vec<String>>,
    /// The short-lived select-token — present iff the person belongs to >1 workspace and must pick.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select_token: Option<String>,
    /// The workspaces the person may enter — present in both branches (the picker + the switcher roster).
    pub workspaces: Vec<WorkspaceRow>,
}

impl AuthReply {
    /// A full-session reply (1-branch, or a completed select/switch).
    pub fn session(
        token: String,
        principal: String,
        workspace: String,
        caps: Vec<String>,
        workspaces: Vec<WorkspaceRow>,
    ) -> Self {
        Self {
            token: Some(token),
            principal: Some(principal),
            workspace: Some(workspace),
            caps: Some(caps),
            select_token: None,
            workspaces,
        }
    }

    /// A select-needed reply (N-branch): a select-token + the roster, no full token.
    pub fn select(select_token: String, workspaces: Vec<WorkspaceRow>) -> Self {
        Self {
            token: None,
            principal: None,
            workspace: None,
            caps: None,
            select_token: Some(select_token),
            workspaces,
        }
    }
}
