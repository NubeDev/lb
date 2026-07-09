//! The nav service — the host's capability chokepoint for the **nav builder** surface (nav scope;
//! README §6.5, the S4 asset model). A nav is an **asset**: a workspace-namespaced `nav:{id}` record
//! holding an ordered menu (`items[]`), wrapped with the three-gate read check (workspace → capability
//! → membership/visibility), reusing the shipped S4 `share`/`member` edges rather than a new ACL —
//! exactly the `dashboard` pattern.
//!
//! **The nav is a LENS, never a grant.** It shapes which pages appear in a member's sidebar; it grants
//! nothing. An item carries no caps; `nav.resolve` strips every item the caller can't reach; the
//! gateway re-checks every verb on click regardless (nav scope, "the lens grants nothing" — the
//! headline "nav never widens" test).
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `nav.get` ([`nav_get`]) — three-gate read of one nav (full `items[]`).
//!   - `nav.list` ([`nav_list`]) — the membership-filtered roster (summaries, no items).
//!   - `nav.save` ([`nav_save`]) — idempotent UPSERT for create+update (owner-only update; bounded).
//!   - `nav.delete` ([`nav_delete`]) — idempotent tombstone (owner-only).
//!   - `nav.share` ([`nav_share`]) — set visibility / write the S4 `share` edge.
//!   - `nav.unshare` ([`nav_unshare`]) — revoke one S4 `share` edge (the inverse write).
//!   - `nav.list_shares` ([`nav_list_shares`]) — enumerate the live team shares (the builder roster).
//!   - `nav.set_default` ([`nav_set_default`]) — set the one workspace-default pointer (admin-ish).
//!   - `nav.resolve` ([`nav_resolve`]) — THE composite read: pick + tag-expand + cap-strip (member).
//!   - `nav.pref.get`/`nav.pref.set` ([`nav_pref_get`]/[`nav_pref_set`]) — the member-owned active pick.
//!   - the MCP bridge ([`call_nav_tool`]) — the one MCP contract over all of the above.

mod admin_lens;
mod authorize;
mod bounds;
mod default;
mod delete;
mod error;
mod get;
mod hidden;
mod list;
mod list_shares;
mod model;
mod pref;
mod reach;
mod resolve;
mod resolve_template_group;
mod save;
mod share;
mod store;
mod surfaces;
mod tool;
mod unshare;
mod visibility;

pub use bounds::BUILTIN_PICK;
pub use default::nav_set_default;
pub use delete::nav_delete;
pub use error::NavError;
pub use get::nav_get;
pub use hidden::{nav_hidden_get, nav_hidden_set};
pub use list::nav_list;
pub use list_shares::nav_list_shares;
pub use model::{
    Nav, NavFacet, NavHidden, NavItem, NavPref, NavSummary, ResolvedItem, ResolvedNav,
    ResolvedSource, Visibility, MAX_HIDDEN, MAX_ITEMS, MAX_PINNED, MAX_TAG_GROUP,
};
pub use pref::{nav_pref_get, nav_pref_set};
pub use reach::{reach_caps, reach_check, REACH_ALL};
pub use resolve::nav_resolve;
pub use save::nav_save;
pub use share::nav_share;
pub use tool::call_nav_tool;
pub use unshare::nav_unshare;
