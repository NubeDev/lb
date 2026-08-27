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
//!   - `nav.get_default` ([`nav_get_default`]) — read that same pointer (member-level, rides
//!     `nav.resolve`) — so a UI can show WHICH nav is the default instead of echoing its own write.
//!   - `nav.resolve` ([`nav_resolve`]) — THE composite read: pick + tag-expand + cap-strip (member).
//!   - `nav.pref.get`/`nav.pref.set` ([`nav_pref_get`]/[`nav_pref_set`]) — the member-owned active pick.
//!   - `nav.ext_boards.get`/`nav.ext_boards.set` ([`ext_nav_boards_get`]/[`ext_nav_boards_set`]) —
//!     the HOST-authored board rows merged into an extension's sidebar section, so placing a board
//!     under an extension needs no extension release (host-authored-ext-nav-boards scope).
//!   - the MCP bridge ([`call_nav_tool`]) — the one MCP contract over all of the above.

mod admin_lens;
mod authorize;
mod bounds;
mod default;
mod delete;
mod error;
mod ext_boards;
mod ext_boards_model;
mod ext_boards_pin;
mod get;
mod hidden;
mod list;
mod list_shares;
mod model;
mod pref;
mod reach;
mod reach_record;
mod resolve;
mod resolve_pins;
mod resolve_template_group;
mod resolved;
mod save;
mod share;
mod store;
mod surfaces;
mod tool;
mod unshare;
mod visibility;

pub use bounds::BUILTIN_PICK;
pub use default::{nav_get_default, nav_set_default};
pub use delete::nav_delete;
pub use error::NavError;
pub use ext_boards::{ext_nav_boards_get, ext_nav_boards_set};
pub use ext_boards_model::{ExtBoardRow, ExtNavBoards, MAX_EXT_BOARD_ROWS, MAX_EXT_BOARD_SLOTS};
pub use get::nav_get;
pub use hidden::{nav_hidden_get, nav_hidden_set, nav_order_set};
pub use list::nav_list;
pub use list_shares::nav_list_shares;
pub use model::{
    Nav, NavFacet, NavHidden, NavItem, NavPref, NavSummary, Visibility, MAX_GROUP_DEPTH,
    MAX_HIDDEN, MAX_ITEMS, MAX_ORDER, MAX_PINNED, MAX_TAG_GROUP, MAX_TITLE_TEMPLATE,
};
pub use pref::{nav_pref_get, nav_pref_set, nav_pref_set_force_builtin};
pub use reach::{reach_caps, reach_caps_for, reach_check, REACH_ALL};
pub use reach_record::{dashboard_reach_ok, reach_record_check, MAX_RECORD_REACH_CAPS};
pub use resolve::nav_resolve;
pub use resolved::{ResolvedItem, ResolvedNav, ResolvedSource};
pub use save::nav_save;
pub use share::nav_share;
pub use tool::call_nav_tool;
pub use unshare::nav_unshare;
