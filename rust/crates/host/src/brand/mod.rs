//! The brand service — the host's capability chokepoint for **brand profiles** (reports scope,
//! "Brand profiles"). A brand is a workspace-namespaced `brand:{id}` record holding reusable
//! document branding (name, logo `asset_id`, colors, fonts, header/footer). It is standalone and
//! cross-cutting — the report builder is its first consumer (a report stores a `brand_id`).
//!
//! Unlike a panel, a brand carries no visibility tiers: it is workspace-shared (any member with the
//! read cap reads it), still gated by `mcp:brand.<verb>:call`. The verbs (one per file, FILE-LAYOUT):
//!   - `brand.get` / `brand.list` — read / roster.
//!   - `brand.save` — idempotent UPSERT (owner-forced create; owner-only update, except the seeded
//!     default's `SYSTEM_OWNER` sentinel, which any writer with the cap adopts).
//!   - `brand.delete` — idempotent tombstone (owner-only + the same `SYSTEM_OWNER` exception).
//!   - [`seed_default_brand`] — the boot seeder (one neutral default so pickers are never empty).
//!   - the MCP bridge ([`call_brand_tool`]).

mod authorize;
mod delete;
mod error;
mod get;
mod list;
mod model;
mod save;
mod seed;
mod store;
mod tool;

pub use delete::brand_delete;
pub use error::BrandError;
pub use get::brand_get;
pub use list::brand_list;
pub use model::{Brand, BrandSummary, Colors as BrandColors, Fonts as BrandFonts};
pub use save::brand_save;
pub use seed::seed_default_brand;
pub use tool::call_brand_tool;
