//! `ops` — the **pure transform logic** behind the data/JSON built-in node pack (data-nodes scope,
//! Tier A). Every function here is a pure function of a JSON `payload` (+ the node's validated
//! `config`), with **no store, bus, or host seam** — so the transforms are unit-tested *in this
//! crate*, next to the descriptors (data-nodes Testing plan: "Tier A … live in `crates/flows`"). The
//! host's `execute_node` dispatch arm is a thin wrapper: resolve inputs → call one `ops::*` fn → wrap
//! the result as a `{ payload }` envelope (Decision 6). The engine change (state, gating, parking)
//! stays in the host; the *shape* work lives here.
//!
//! ## The op contract
//!
//! A Tier-A op returns `Result<Value, String>`: `Ok(new_payload)` (the node emits `{ payload }`) or
//! `Err(msg)` (the node **fails**, surfaced under the flow's `FailurePolicy` — the `json`-node
//! failure parity a parse op inherits for malformed input). Ops never mutate shared state and never
//! read the clock.
//!
//! Shared helpers (Risk 5 — one field-path + one predicate, never four bespoke matchers):
//! - [`path`] — the dot-path get/set/delete (`change`/`select` addressing; the exact binding walker).
//! - [`predicate`] — the `{op,value}` evaluator (`switch` routing + `filter` deadband share it).
//!
//! The category files:
//! - [`data`] — `change`/`select`/`merge`/`map`/`flatten`/`sort`/`range`/`aggregate` (Data).
//! - [`template`] — `template` (mustache-lite text render; no templating engine — Risk 4).
//! - [`parse`] — `csv`/`xml`/`yaml`/`base64` (Parse; malformed input FAILS the node).
//! - [`sequence`] — `split`/`join` array-carry + the `parts` sequence contract (Decision 15).

pub mod data;
pub mod parse;
pub mod path;
pub mod predicate;
pub mod sequence;
pub mod template;
