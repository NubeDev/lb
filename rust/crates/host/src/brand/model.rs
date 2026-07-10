//! The brand-profile record (reports scope, "Brand profiles"). A `brand:{id}` is a
//! workspace-namespaced, reusable document-branding resource — name, logo (an `asset_id` ref or
//! empty), color palette, fonts, and header/footer text. It is **standalone and cross-cutting**:
//! the report builder is merely its first consumer (a report stores a `brand_id`). Unlike a panel
//! it carries no visibility tiers — a brand is workspace-shared (any member with the read cap sees
//! it), still gated by `mcp:brand.<verb>:call`. The closed-struct discipline holds: additive
//! serde-defaulted fields, typed nested objects, no app-side JSON parsing.

use serde::{Deserialize, Serialize};

/// The table brands live in. Record id is `brand:{id}` (a stable slug, unique per workspace).
pub const TABLE: &str = "brand";

/// Our brand-model document version, pinned on [`Brand::schema_version`] at save.
pub const SCHEMA_VERSION: u32 = 1;

/// The brand color palette. CSS-style `#rrggbb` strings; empty fields fall back to neutral
/// defaults at render time (mirrors `lb_render::Colors`, the render target).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Colors {
    /// The dominant brand color (headings, title, rules).
    #[serde(default)]
    pub primary: String,
    /// The highlight/accent color (links).
    #[serde(default)]
    pub accent: String,
    /// Default body-text color.
    #[serde(default)]
    pub text: String,
    /// Page/background color.
    #[serde(default)]
    pub background: String,
}

/// The brand typography. Only the embeddable fonts render in the PDF (lesson 4); the brand editor
/// surfaces the allowed list — the host stores whatever it is handed, opaquely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Fonts {
    /// Font family for headings.
    #[serde(default)]
    pub heading: String,
    /// Font family for body text.
    #[serde(default)]
    pub body: String,
}

/// A brand-profile record. Reusable branding (colors/fonts/logo/header/footer) for reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brand {
    /// Stable slug, unique per workspace (the record id `brand:{id}`).
    pub id: String,
    /// The profile name (free to rename; the slug is forever).
    pub name: String,
    /// The principal who created it.
    pub owner: String,
    /// The logo — an `asset_id` into the shipped `assets.*` store (empty = no logo).
    #[serde(default, rename = "logoAssetId")]
    pub logo_asset_id: String,
    /// The color palette.
    #[serde(default)]
    pub colors: Colors,
    /// The typography.
    #[serde(default)]
    pub fonts: Fonts,
    /// Header text rendered on every page (supports `{page}`/`{title}`/`{date}` tokens — data).
    #[serde(default, rename = "headerText")]
    pub header_text: String,
    /// Footer text rendered on every page.
    #[serde(default, rename = "footerText")]
    pub footer_text: String,
    /// OUR brand-model document version — pinned at save (`SCHEMA_VERSION`).
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent).
    #[serde(default)]
    pub deleted: bool,
}

impl Default for Brand {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            owner: String::new(),
            logo_asset_id: String::new(),
            colors: Colors::default(),
            fonts: Fonts::default(),
            header_text: String::new(),
            footer_text: String::new(),
            schema_version: SCHEMA_VERSION,
            updated_ts: 0,
            deleted: false,
        }
    }
}

/// The cheap roster row `brand.list` returns — id/name/updated_ts (no logo bytes, no palette body).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrandSummary {
    pub id: String,
    pub name: String,
    pub updated_ts: u64,
}

impl From<&Brand> for BrandSummary {
    fn from(b: &Brand) -> Self {
        Self {
            id: b.id.clone(),
            name: b.name.clone(),
            updated_ts: b.updated_ts,
        }
    }
}
