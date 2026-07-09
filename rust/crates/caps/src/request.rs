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
    /// Outbound network — a native (Tier-2) extension opening a socket to an admin-approved
    /// `host:port` (datasources scope: `net:tls:tsdb.acme:5432`). Enforced pre-connect by the
    /// supervisor (`requested ∩ admin_approved`); core crates never open sockets.
    Net,
    /// Page **reach** — may this subject OPEN a core surface (page)? `reach:<surface>:view`
    /// (nav-reach scope). Orthogonal to the data caps that gate a page's tiles/reads: a subject may
    /// hold `series.read` (so their dashboard tile renders) yet lack `reach:ingest:view` (so the
    /// Ingest page 403s). The reach caps are DERIVED from the subject's resolved nav at login — a
    /// curated nav yields one `reach:<surface>:view` per menu surface; a fallback (no nav) yields the
    /// wildcard `reach:*:view` (reaches all, so a default member/admin is never locked out). The nav
    /// gates reach without widening: reach is only emitted for surfaces the resolver already kept.
    Reach,
}

impl Surface {
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Mcp => "mcp",
            Surface::Store => "store",
            Surface::Bus => "bus",
            Surface::Secret => "secret",
            Surface::Net => "net",
            Surface::Reach => "reach",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mcp" => Some(Surface::Mcp),
            "store" => Some(Surface::Store),
            "bus" => Some(Surface::Bus),
            "secret" => Some(Surface::Secret),
            "net" => Some(Surface::Net),
            "reach" => Some(Surface::Reach),
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
    /// Open an outbound connection (the `net` surface): `net:tls:host:5432:connect`.
    Connect,
    /// Open (reach) a core surface (the `reach` surface): `reach:rules:view` (nav-reach scope).
    View,
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
            Action::Connect => "connect",
            Action::View => "view",
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
            "connect" => Some(Action::Connect),
            "view" => Some(Action::View),
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
