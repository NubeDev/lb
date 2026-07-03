//! The verb library — what a rule body may call, and nothing else. **Ported from rubix-cube**
//! (`rules/verbs/`, MIT/Apache-2.0). `register` wires every verb closure into a fresh engine, each
//! closing over the run's pinned data seam / allowlist / collectors / AI meter — so tenancy and the
//! budget are enforced at every call (rule 5). One file per verb family (FILE-LAYOUT).

mod ai;
mod data;
mod duration;
mod emit;
pub(crate) mod inbox;
pub(crate) mod outbox;
mod timeseries;

pub use ai::AiHandle;
pub use emit::Collectors;
pub use inbox::InboxHandle;
pub use outbox::OutboxHandle;

use std::collections::HashSet;
use std::sync::Arc;

use rhai::Engine;

use crate::grid::{Col, Grid, GridCtx, GroupedGrid, Span};
use crate::meter::{AiMeter, WriteMeter};
use crate::seam::{AiSeam, DataSeam, MessagingSeam};

/// The three scope handles a run pushes as top-level variables: `ai`, `inbox`, `outbox` (the
/// `channel` handle is slice 3). Returned so the engine can push them after registering the verbs.
pub struct RunHandles {
    pub ai: AiHandle,
    pub inbox: InboxHandle,
    pub outbox: OutboxHandle,
}

/// Register the grid value types + every verb family into `engine`. Returns the scope handles to push
/// (`ai`/`inbox`/`outbox`).
#[allow(clippy::too_many_arguments)]
pub fn register(
    engine: &mut Engine,
    ctx: Arc<GridCtx>,
    data: Arc<dyn DataSeam>,
    allow: Arc<HashSet<String>>,
    inputs: Arc<rhai::Map>,
    collectors: Arc<Collectors>,
    ai_seam: Arc<dyn AiSeam>,
    meter: Arc<AiMeter>,
    context_rows: usize,
    messaging: Arc<dyn MessagingSeam>,
    write_meter: Arc<WriteMeter>,
    now: u64,
) -> RunHandles {
    register_types(engine);
    register_grid_methods(engine);
    data::register(engine, ctx.clone(), allow.clone(), inputs);
    timeseries::register(engine);
    emit::register(engine, collectors);
    ai::register(engine);
    inbox::register(engine);
    outbox::register(engine);
    RunHandles {
        ai: AiHandle::new(ai_seam, ctx, data, allow, meter, context_rows),
        inbox: InboxHandle::new(messaging.clone(), write_meter.clone(), now),
        outbox: OutboxHandle::new(messaging, write_meter, now),
    }
}

/// Register the opaque grid value types so rhai can pass them around.
fn register_types(engine: &mut Engine) {
    engine.register_type_with_name::<Grid>("Grid");
    engine.register_type_with_name::<Col>("Col");
    engine.register_type_with_name::<GroupedGrid>("GroupedGrid");
    engine.register_type_with_name::<Span>("Span");
}

/// Register the lazy grid plan-builders + `Col` reductions (the chainable surface).
fn register_grid_methods(engine: &mut Engine) {
    engine.register_fn("filter", |g: &mut Grid, expr: &str| g.filter(expr));
    engine.register_fn("select", |g: &mut Grid, cols: rhai::Array| g.select(cols));
    engine.register_fn("add_col", |g: &mut Grid, name: &str, expr: &str| {
        g.add_col(name, expr)
    });
    engine.register_fn("rename", |g: &mut Grid, from: &str, to: &str| {
        g.rename(from, to)
    });
    engine.register_fn("group_by", |g: &mut Grid, keys: rhai::Array| {
        g.group_by(keys)
    });
    engine.register_fn("join", |g: &mut Grid, other: Grid, on: &str, how: &str| {
        g.join(other, on, how)
    });
    engine.register_fn("col", |g: &mut Grid, name: &str| g.col(name));
    engine.register_fn("head", |g: &mut Grid, n: i64| g.head(n));
    engine.register_fn("size", |g: &mut Grid| g.size());
    engine.register_fn("columns", |g: &mut Grid| {
        g.columns().map(|c| {
            c.into_iter()
                .map(rhai::Dynamic::from)
                .collect::<rhai::Array>()
        })
    });
    engine.register_fn("records", |g: &mut Grid| g.records());

    engine.register_fn("agg", |g: &mut GroupedGrid, exprs: rhai::Array| {
        g.agg(exprs)
    });

    engine.register_fn("max", |c: &mut Col| c.max());
    engine.register_fn("min", |c: &mut Col| c.min());
    engine.register_fn("avg", |c: &mut Col| c.avg());
    engine.register_fn("mean", |c: &mut Col| c.avg());
    engine.register_fn("sum", |c: &mut Col| c.sum());
    engine.register_fn("count", |c: &mut Col| c.count());
    engine.register_fn("std", |c: &mut Col| c.std());
    engine.register_fn("first", |c: &mut Col| c.first());
    engine.register_fn("last", |c: &mut Col| c.last());
    engine.register_fn("p", |c: &mut Col, pct: i64| c.p(pct));
}
