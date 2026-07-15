//! `stats` — array statistics over plain rhai arrays (data-stdlib-scope): center/spread, quantiles,
//! shape, normalization, correlation/regression, sequences, missing-value handling, outliers,
//! selection. Pure compute — no seam, no cap, no I/O. Windowed functions live in `../window.rs`
//! (one family, two files); the shared numeric plumbing lives in [`numeric`].
//!
//! # Input policy (uniform across the family)
//!
//! Inputs are plain rhai arrays. A **numeric** element is an `INT` or a finite `FLOAT`; `()`,
//! non-numeric entries, and non-finite floats (`NaN`/`inf`) are **missing**. Missing values are
//! - **skipped by aggregations** — `mean([1, (), 3])` is `2.0`;
//! - **preserved positionally as `()`** by windowed/sequence/normalize functions, whose outputs
//!   have the input's length with a number or `()` in every slot.
//! Numeric results are always `FLOAT`. `variance`/`std_dev`/`sem` (and `rolling_std`) are
//! **sample** statistics (n-1 denominator). `percentile` uses linear interpolation. The two
//! element-order verbs (`sample`/`shuffle`) pass elements through **verbatim** (they reorder, they
//! don't compute) and take a **mandatory seed** — a deterministic in-crate PRNG, no `rand`, no
//! ambient randomness. Undefined cases (empty input, mismatched lengths, zero variance, bad
//! window/seed arguments) are clear author-facing errors, never silent numbers.

mod center;
mod missing;
pub(crate) mod numeric;
mod relate;
mod rows;
mod select;
mod sequence;
mod shape;

use rhai::Engine;

/// Register the array-stats verbs (free functions — no handle).
pub fn register(engine: &mut Engine) {
    center::register(engine);
    shape::register(engine);
    relate::register(engine);
    sequence::register(engine);
    missing::register(engine);
    select::register(engine);
}

/// Catalog rows for the `stats.rs` half of the family (the windowed rows live in
/// `window.rs::CATALOG` — one family, two files).
pub(crate) use rows::CATALOG;
