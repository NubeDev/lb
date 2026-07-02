//! Scope resolution — the **member wall** (agent-memory scope: "`member:{user}` sub-scope resolved
//! from the authenticated principal, never from an argument"). This is the one place a principal
//! becomes a set of scopes, so a run under user U can ONLY ever touch `workspace` + `member:U`:
//!
//!   - **read set** (`list`/`get`): `[workspace, member:self]` — the two scopes U's runs see.
//!   - **write target** (`set`/`delete`): the caller passes `scope: "workspace" | "member"` (a
//!     TIER, not a user), which resolves to `Workspace` or `Member(self)`. A caller can NEVER name
//!     another member — `member` always binds to the authenticated principal. An unknown tier is a
//!     `BadInput` (never silently a different member).
//!
//! The principal's `sub` is the member key (`user:ada` → `member:user:ada`). Because the write
//! target is derived here and the list query is walled to the read set, bob's resolution never
//! returns `member:ada` rows even if bob knows ada's slugs — the member wall is structural.

use lb_auth::Principal;

use super::model::MemoryScope;

/// The scopes a principal's runs may READ, in injection order (workspace first, then the member's
/// own). Never includes another member's scope — that key is simply never produced here.
pub fn read_scopes(principal: &Principal) -> Vec<MemoryScope> {
    vec![
        MemoryScope::Workspace,
        MemoryScope::Member(principal.sub().to_string()),
    ]
}

/// Resolve the WRITE target scope from the caller's `scope` tier argument + the principal. `None`/
/// `"member"` → the caller's own `member:{self}` (the default: a run remembers for its member);
/// `"workspace"` → the shared scope. `Some(other)` → `None` here (the verb maps that to `BadInput`).
/// Crucially, `"member"` binds to the AUTHENTICATED principal — a caller cannot ask for `member:V`.
pub fn write_scope(principal: &Principal, scope_arg: Option<&str>) -> Option<MemoryScope> {
    match scope_arg {
        None | Some("member") => Some(MemoryScope::Member(principal.sub().to_string())),
        Some("workspace") => Some(MemoryScope::Workspace),
        Some(_) => None,
    }
}

/// Resolve the scope to GET/DELETE a slug from: same rule as the write target (`member` → self,
/// `workspace` → shared). A `get` for `member` can only ever read the caller's own member scope.
pub fn addressed_scope(principal: &Principal, scope_arg: Option<&str>) -> Option<MemoryScope> {
    write_scope(principal, scope_arg)
}
