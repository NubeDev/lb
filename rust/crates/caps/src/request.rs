//! The thing being authorized: a `(surface, resource, action)` request, plus the workspace
//! it targets. The workspace is on the request (not in a capability string) precisely so the
//! isolation gate can run *before* any capability is consulted (auth-caps scope).

/// The four enforcement surfaces (auth-caps grammar). A new surface is a deliberate grammar
/// change, not an ad-hoc string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Mcp,
    Store,
    Bus,
    Secret,
}

impl Surface {
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Mcp => "mcp",
            Surface::Store => "store",
            Surface::Bus => "bus",
            Surface::Secret => "secret",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mcp" => Some(Surface::Mcp),
            "store" => Some(Surface::Store),
            "bus" => Some(Surface::Bus),
            "secret" => Some(Surface::Secret),
            _ => None,
        }
    }
}

/// Surface-specific verbs. `Any` (`*`) matches every action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Call,
    Read,
    Write,
    Pub,
    Sub,
    Get,
    Any,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Call => "call",
            Action::Read => "read",
            Action::Write => "write",
            Action::Pub => "pub",
            Action::Sub => "sub",
            Action::Get => "get",
            Action::Any => "*",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "call" => Some(Action::Call),
            "read" => Some(Action::Read),
            "write" => Some(Action::Write),
            "pub" => Some(Action::Pub),
            "sub" => Some(Action::Sub),
            "get" => Some(Action::Get),
            "*" => Some(Action::Any),
            _ => None,
        }
    }
}

/// A concrete access request. `ws` is the target workspace — checked first, before `caps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub ws: String,
    pub surface: Surface,
    /// The `/`-segmented resource path WITHIN the surface (no workspace prefix — that's `ws`).
    pub resource: String,
    pub action: Action,
}

impl Request {
    pub fn new(
        ws: impl Into<String>,
        surface: Surface,
        resource: impl Into<String>,
        action: Action,
    ) -> Self {
        Self {
            ws: ws.into(),
            surface,
            resource: resource.into(),
            action,
        }
    }
}
