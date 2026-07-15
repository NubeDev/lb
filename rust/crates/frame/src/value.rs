//! The `Frame` value type — the one opaque value the rhai cage passes around (Clone; wraps a
//! polars `DataFrame` + the run's [`FrameLimits`]). Every constructor and every frame-producing
//! verb goes through [`Frame::new`], which checks the OUTPUT against the row/cell caps — the
//! deadline cannot interrupt a native polars call, so the bound lives on the values themselves
//! (scope "Governors").

use polars::prelude::{DataFrame, LazyFrame, PolarsError};
use rhai::{Dynamic, EvalAltResult};

use crate::limits::FrameLimits;

/// An in-memory, post-collect dataframe inside the cage. Cloning clones the underlying columns'
/// `Arc`s (cheap); the limits ride along so every derived frame is capped the same way.
#[derive(Clone)]
pub struct Frame {
    pub(crate) df: DataFrame,
    pub(crate) limits: FrameLimits,
}

impl Frame {
    /// The ONE cap chokepoint: every Frame that ever exists passed this shape check.
    pub fn new(df: DataFrame, limits: FrameLimits) -> Result<Self, Box<EvalAltResult>> {
        limits.check_frame(df.height(), df.width()).map_err(rerr)?;
        Ok(Self { df, limits })
    }

    /// Derive a new Frame from this one (same limits, re-checked on the new shape).
    pub(crate) fn with_df(&self, df: DataFrame) -> Result<Self, Box<EvalAltResult>> {
        Self::new(df, self.limits)
    }

    /// Run a lazy plan eagerly and wrap the result (cap-checked like every other output).
    pub(crate) fn collect(&self, lf: LazyFrame) -> Result<Self, Box<EvalAltResult>> {
        self.with_df(lf.collect().map_err(perr)?)
    }

    /// Borrow the wrapped dataframe (read-only; the tests inspect shapes through this).
    pub fn df(&self) -> &DataFrame {
        &self.df
    }
}

/// Make a rhai eval error from any string (the author-facing error shape).
pub(crate) fn rerr(msg: impl Into<String>) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from(msg.into()),
        rhai::Position::NONE,
    ))
}

/// Surface a polars error verbatim as an author error (`f.sql` syntax errors etc. — the scope's
/// testing plan wants them readable, not swallowed).
pub(crate) fn perr(e: PolarsError) -> Box<EvalAltResult> {
    rerr(format!("frame: {e}"))
}
