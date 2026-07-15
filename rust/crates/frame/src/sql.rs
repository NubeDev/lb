//! `f.sql("SELECT … FROM self")` — the long-tail method: polars' own `SQLContext` with exactly
//! ONE table registered (`self`). In-memory only by construction: no other table resolves, and
//! `read_csv`/`read_parquet` are not registered as SQL table functions on the pinned feature set
//! (proven at runtime by `tests/sql_security_test.rs` — the polars version is a SECURITY pin).
//! Errors (syntax and otherwise) surface verbatim as author feedback; the result frame passes
//! the same row/cell caps as every other frame-producing verb.

use polars::prelude::IntoLazy;
use polars::sql::SQLContext;
use rhai::{Engine, EvalAltResult};

use crate::value::{perr, Frame};

/// Register the sql verb.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn(
        "sql",
        |f: &mut Frame, query: &str| -> Result<Frame, Box<EvalAltResult>> {
            let mut ctx = SQLContext::new();
            ctx.register("self", f.df.clone().lazy());
            let lf = ctx.execute(query).map_err(perr)?;
            f.collect(lf)
        },
    );
}
