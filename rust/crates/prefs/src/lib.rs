//! `lb-prefs` — canonical-in, localized-out preferences + units + formatting (prefs scope, README
//! §6.5/§3.7). The platform stores domain data **canonically** (UTC instants, base units, locale-
//! neutral enums) and **never** a formatted string; this crate is the boundary that renders a
//! canonical value per a principal's *resolved* preferences.
//!
//! Three pieces, kept apart:
//!   1. **The record + resolution** (`axis`, `prefs`, `resolve`, `store`) — the nullable per-(ws,user)
//!      and per-(ws) preference records and the pure fold over the chain
//!      `request override → user → workspace default → built-in fallback`.
//!   2. **Conversion** (`convert`) — `uom`-backed dimensional correctness (affine °C↔°F, m/s↔knots);
//!      a cross-dimension convert is rejected structurally.
//!   3. **Formatting** (`format`) — locale-aware number/date rendering and tz application over a UTC
//!      instant (DST-correct via `chrono-tz`).
//!
//! Everything here is **pure** (no auth, no bus, no clock): the same code runs on edge and cloud,
//! fully offline. Authorization is the host's job — the `format.*`/`convert.*` verbs are a grant-free
//! utility tier (no tenant data), while `prefs.get/set/resolve/set_default` are gated there.

pub mod axis;
pub mod catalog;
mod convert;
mod error;
mod format;
mod prefs;
mod resolve;
mod store;

pub use axis::{DateStyle, Dimension, FirstDay, NumberFormat, TimeStyle, Unit, UnitSystem};
pub use catalog::{
    lint as lint_catalog, merged_catalog, render as render_message, Rendered as RenderedMessage,
};
pub use convert::{convert, to_display, DisplayQuantity};
pub use error::PrefsError;
pub use format::{format_datetime, format_number, format_quantity, FormattedQuantity, NumberOpts};
pub use prefs::{Prefs, ResolvedPrefs};
pub use resolve::{builtin, resolve};
pub use store::{
    define_prefs_schema, get_catalog_override, get_user_prefs, get_workspace_prefs, resolve_chain,
    set_catalog_override, set_user_prefs, set_workspace_prefs, CATALOG_TABLE, USER_PREFS_TABLE,
    WORKSPACE_PREFS_TABLE,
};

/// The enabled-language slice this build compiled in (en/es). Re-exported for the host's bootstrap
/// locale path and the generated client constants.
pub use axis::language::{
    is_enabled as language_enabled, ENABLED as ENABLED_LANGUAGES, FALLBACK as FALLBACK_LANGUAGE,
};
