//! The `pack.*` verb family — domain packs in core (pack-core-scope).
//!
//! A **domain pack** is one versioned, declarative artifact — datasource schema + optional seed, the
//! semantic vocabulary, rules, pre-bound dashboards, channels, and the agent's domain context —
//! applied to ONE workspace. A node boots blank and generic; a *deployment* never is. This family is
//! what turns the first into the second in one call:
//!
//!   - `pack.validate` — parse + plan + lint + "what would an apply decide" (the CI validator, and
//!     the dry run). Read-tier.
//!   - `pack.apply`    — idempotent apply with the refusal matrix, per-object outcomes, loud
//!     clobber listing, receipt written at the end. Admin-tier — it writes through every object
//!     family.
//!   - `pack.list` / `pack.get` — first-class receipts. Member-read: a receipt is operator
//!     documentation.
//!
//! **Rule 10.** Core owns the mechanism and knows NO pack by name. Every branch in this module is on
//! an object KIND (`rule`, `dashboard`, …), which is data; `bas` and `ems` differ only in bytes.
//! Packs are authored and shipped by embedders and third parties, never in this repo.
//!
//! **No cap smuggling.** `mcp:pack.apply:call` gets a caller into the orchestration and nothing
//! more: every object is driven through the same internal seam the equivalent public verb calls, and
//! each of those re-checks its own capability under the caller's principal (see `apply.rs`).
//!
//! The pure half — manifest shape, plan, checksums, refusal matrix — lives in `lb-packs`, so it is
//! unit-testable without a node. This module owns the I/O: bundle intake, the seams, the receipt.
//!
//! *(Naming: `lb-pack` / `rust/tools/pack` is the unrelated extension-artifact packager. Nothing
//! here touches that toolchain.)*

mod apply;
mod authorize;
mod error;
mod read;
mod run;
mod sqlite;
mod store;
mod store_seed;
mod tool;
mod validate;
mod verb;

pub use error::PackError;
pub use read::{pack_get, pack_list};
pub use store::{read_receipt, scan_receipts, TABLE as PACK_RECEIPT_TABLE};
pub use tool::call_pack_tool;
pub use validate::pack_validate;
pub use verb::pack_apply;
