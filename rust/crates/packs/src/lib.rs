//! lb-packs — the pure half of the domain-pack engine (pack-core-scope).
//!
//! A **domain pack** is one versioned, declarative artifact — datasource schema + optional seed, the
//! semantic vocabulary, rules, pre-bound dashboards, channels, and the agent's domain context —
//! applied to ONE workspace. This crate owns everything about that which is a *decision*:
//!
//!   - [`manifest`] — the `pack.yaml` shape (`deny_unknown_fields`, line-numbered errors)
//!   - [`bundle`]   — a pack over the wire, resolved to a [`bundle::Pack`] (no filesystem)
//!   - [`plan`]     — the ordered object plan + the checksums drift is measured by
//!   - [`validate`] — the dry-run linter (errors gate, dialect warnings don't)
//!   - [`decision`] — the refusal matrix
//!   - [`receipt`]  — the record of an apply
//!
//! The I/O — bundle intake, the internal seams an apply drives, receipt persistence — belongs to the
//! `pack.*` verb handlers in `lb-host`. Nothing in this crate performs I/O, and nothing in it knows
//! a pack by name (rule 10): a pack is data.

pub mod binding;
pub mod bundle;
pub mod decision;
pub mod manifest;
pub mod plan;
pub mod receipt;
pub mod validate;

pub use bundle::{Bundle, LoadedDashboard, LoadedRule, Pack, MAX_BUNDLE_BYTES};
pub use decision::{decide, Decision};
pub use manifest::Manifest;
pub use plan::{checksum, content_checksum, plan, Kind, PlannedObject};
pub use receipt::{ObjectReceipt, Receipt};
pub use validate::{has_errors, validate, Finding};
