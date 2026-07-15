//! The verb library — what a rule body may call, and nothing else. **Ported from rubix-cube**
//! (`rules/verbs/`, MIT/Apache-2.0). `register` wires every verb closure into a fresh engine, each
//! closing over the run's pinned data seam / allowlist / collectors / AI meter — so tenancy and the
//! budget are enforced at every call (rule 5). One file per verb family (FILE-LAYOUT).

mod ai;
pub(crate) mod channel;
mod chart;
mod data;
pub(crate) mod duration;
mod emit;
#[cfg(feature = "frames")]
pub(crate) mod frame;
pub(crate) mod inbox;
pub(crate) mod insight;
pub(crate) mod job;
pub(crate) mod json;
pub(crate) mod mathx;
pub(crate) mod outbox;
pub(crate) mod stats;
pub(crate) mod time;
mod timeseries;
pub(crate) mod window;

pub use ai::AiHandle;
pub use channel::ChannelHandle;
pub use emit::Collectors;
pub use inbox::InboxHandle;
pub use insight::InsightHandle;
pub use job::JobHandle;
pub use outbox::OutboxHandle;
pub use time::TimeHandle;

use std::collections::HashSet;
use std::sync::Arc;

use rhai::Engine;

use crate::control::RunControl;
use crate::grid::{Col, Grid, GridCtx, GroupedGrid, Span};
use crate::meter::{AiMeter, WriteMeter};
use crate::sandbox::RuleLimits;
use crate::seam::{AiSeam, DataSeam, JobSeam, MessagingSeam};

/// The scope handles a run pushes as top-level variables: `ai`, `inbox`, `outbox`, `channel`,
/// `insight`, `time`, `job`. Returned so the engine can push them after registering the verbs.
pub struct RunHandles {
    pub ai: AiHandle,
    pub inbox: InboxHandle,
    pub outbox: OutboxHandle,
    pub channel: ChannelHandle,
    pub insight: InsightHandle,
    pub time: TimeHandle,
    pub job: JobHandle,
}

/// Everything one run's verb library closes over — the pinned seams, meters, and flags. Grouped as
/// a struct (the arg list outgrew positional form when the data stdlib + job handle landed).
pub struct RunWiring {
    pub ctx: Arc<GridCtx>,
    pub data: Arc<dyn DataSeam>,
    pub allow: Arc<HashSet<String>>,
    pub inputs: Arc<rhai::Map>,
    pub collectors: Arc<Collectors>,
    pub ai_seam: Arc<dyn AiSeam>,
    pub meter: Arc<AiMeter>,
    pub context_rows: usize,
    pub messaging: Arc<dyn MessagingSeam>,
    pub write_meter: Arc<WriteMeter>,
    /// The run's pinned logical clock (milliseconds — what messaging ids stamp).
    pub now: u64,
    pub route: bool,
    pub origin_ref: String,
    /// The cage limits (the frame verbs read `max_frame_rows`/`max_frame_cells`).
    pub limits: RuleLimits,
    /// Job-backed runs only: the durable checkpoint seam + the persisted state folded from the
    /// transcript + the shared control. `None` → the `job` handle is ephemeral (sync `rules.run`).
    pub job: Option<JobWiring>,
}

/// The durable half of a job-backed run (long-running-rules-scope).
pub struct JobWiring {
    pub id: String,
    pub seam: Arc<dyn JobSeam>,
    pub state: rhai::Map,
    pub control: Arc<RunControl>,
}

/// Register the grid value types + every verb family into `engine`. Returns the scope handles to
/// push (`ai`/`inbox`/`outbox`/`channel`/`insight`/`time`/`job`).
pub fn register(engine: &mut Engine, wiring: RunWiring) -> RunHandles {
    register_types(engine);
    register_grid_methods(engine);
    data::register(
        engine,
        wiring.ctx.clone(),
        wiring.allow.clone(),
        wiring.inputs,
    );
    timeseries::register(engine);
    chart::register(engine);
    emit::register(engine, wiring.collectors.clone());
    ai::register(engine);
    inbox::register(engine);
    outbox::register(engine);
    channel::register(engine);
    insight::register(engine);
    // The data stdlib (data-stdlib-scope): pure compute, no seam, no cap.
    time::register(engine);
    json::register(engine);
    stats::register(engine);
    window::register(engine);
    mathx::register(engine);
    duration::register(engine);
    #[cfg(feature = "frames")]
    frame::register(engine, wiring.ctx.clone(), &wiring.limits);
    job::register(engine);

    let job = match wiring.job {
        Some(j) => JobHandle::durable(j.id, j.seam, j.state, j.control),
        None => JobHandle::ephemeral(),
    };
    RunHandles {
        ai: AiHandle::new(
            wiring.ai_seam,
            wiring.ctx,
            wiring.data,
            wiring.allow,
            wiring.meter,
            wiring.context_rows,
        ),
        inbox: InboxHandle::new(
            wiring.messaging.clone(),
            wiring.write_meter.clone(),
            wiring.now,
        ),
        outbox: OutboxHandle::new(
            wiring.messaging.clone(),
            wiring.write_meter.clone(),
            wiring.now,
        ),
        channel: ChannelHandle::new(
            wiring.messaging.clone(),
            wiring.write_meter.clone(),
            wiring.now,
        ),
        insight: InsightHandle::new(
            wiring.messaging,
            wiring.write_meter,
            wiring.now,
            wiring.route,
            wiring.origin_ref,
            wiring.collectors,
        ),
        time: TimeHandle { now_ms: wiring.now },
        job,
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
