//! The extension loader — parse `extension.toml` and compute the granted capability set
//! (extensions scope, the §13 manifest decision).
//!
//! Two jobs, deliberately separate (the blast-radius rule, §11.5):
//! 1. [`Manifest::parse`] reads what an extension *requests* and checks its WIT world major.
//! 2. [`grant`] computes `granted = requested ∩ admin_approved` — nothing requested is live
//!    until an admin approved it. "Public" never means "more privileged" (§6.4).

mod grant;
mod manifest;

pub use grant::grant;
pub use manifest::{Manifest, ManifestError, Tool, Visibility};
