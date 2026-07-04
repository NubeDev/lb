//! The Frame input governors (scope "Governors"). Native polars calls can't be interrupted by the
//! rhai deadline, so the bound moves to the *inputs*: [`FrameLimits`] is checked at
//! `frame()`/`frame(records)`/`vstack`/`join`/`pivot` OUTPUT in Phase 2 — polars runs only on inputs
//! already proven bounded.
//!
//! Phase 0 carries the type so the workspace-wide default (`max_frame_rows` = 200 000,
//! `max_frame_cells` = 2 000 000 — scope Open question "max_frame_rows default") is named in one place;
//! the checks wire on in Phase 2.

/// The input governors a run enforces on every Frame-producing op (scope "Governors"). Native polars
/// calls can't be interrupted by the rhai deadline, so the bound moves to the *inputs*: this is
/// checked at `frame()`/`frame(records)`/`vstack`/`join`/`pivot` OUTPUT in Phase 2.
#[derive(Debug, Clone, Copy)]
pub struct FrameLimits {
    /// Max rows in a materialized Frame (a join/vstack that would exceed this aborts BEFORE polars
    /// runs the uninterruptible op).
    pub max_frame_rows: usize,
    /// Max total cells (rows × cols) — the true memory bound (a 1-row × billion-col frame is also
    /// a DoS).
    pub max_frame_cells: usize,
    /// Max bytes in a `to_csv_string`/`to_json_string` export — mirrors the cage's
    /// `RuleLimits::max_string_bytes`.
    pub max_string_bytes: usize,
}

impl Default for FrameLimits {
    fn default() -> Self {
        Self {
            max_frame_rows: 200_000,
            max_frame_cells: 2_000_000,
            max_string_bytes: 256 * 1024,
        }
    }
}
