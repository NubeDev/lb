//! The extension loader — parse `extension.toml` and compute the granted capability set
//! (extensions scope, the §13 manifest decision).
//!
//! Two jobs, deliberately separate (the blast-radius rule, §11.5):
//! 1. [`Manifest::parse`] reads what an extension *requests* and checks its WIT world major.
//! 2. [`grant`] computes `granted = requested ∩ admin_approved` — nothing requested is live
//!    until an admin approved it. "Public" never means "more privileged" (§6.4).

mod grant;
mod manifest;
/// The reference-name extractor for the one template grammar (nav-context-builtins scope). Shared so
/// the manifest path and the host's nav-builder write path validate a template through ONE function.
pub mod template_refs;

pub use grant::grant;
pub use manifest::{
    slug, Manifest, ManifestError, Native, NavItem, Tool, UiPage, Visibility, Widget, WidgetOption,
    NAV_MAX_TITLE_TEMPLATE,
};
