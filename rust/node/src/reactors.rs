//! The boot **reactor** spawns (moved verbatim from `main.rs`): the flow / channel-agent / approval /
//! insight-digest background scan loops, plus the one-shot insight-timestamp heal. Gated by
//! [`BootConfig::reactors`] — the `node` binary spawns them (today's behaviour); an embedder wanting
//! store+auth+MCP only sets `reactors: false` and no background scans run.

use std::sync::Arc;
use std::time::Duration;

use lb_host::Node;

use crate::config::OutboxProviders;
use crate::mail::EmailTransport;

/// Spawn the background reactor loops for `ws` on `node`, and run the one-shot insight-ts heal. One
/// detached owner per reactor per node, each scanning the configured workspace on its own cadence.
/// `providers` is the boot provider-injection seam (release scope, gap 1): the relay reactor
/// delivers email/push effects through them; unset providers fall back to the logging no-ops.
/// `email_transport` is the config-selected mailer (email-transport scope, issue #118) used when the
/// embedder supplied no `EmailProvider` of its own — so a host gets real email from configuration alone.
/// `store_budget_bytes` is [`BootConfig::store_budget_bytes`] — the node's disk allowance. `None`
/// (the default, and what `LB_STORE_MAX_BYTES` unset means) leaves the store-compact reactor
/// warn-only, exactly as it shipped.
/// `retention_period` is [`BootConfig::retention_period`] — the retention-GC cadence. `None` (the
/// default, and what `LB_RETENTION_PERIOD_SECS` unset means) ⇒ [`lb_host::RETENTION_PERIOD`], 300 s.
pub async fn spawn(
    node: &Arc<Node>,
    ws: &str,
    providers: &OutboxProviders,
    email_transport: Option<&EmailTransport>,
    store_budget_bytes: Option<u64>,
    profile: Option<crate::config::ProfileConfig>,
    retention_period: Option<Duration>,
) {
    // FLOW REACTOR TICK: drive cron/reconcile scans so a `mode:"cron"` trigger actually fires. A
    // few-second period catches a minute-granularity cron promptly; each tick is a cheap ws scan.
    lb_host::spawn_flow_reactors(
        node.clone(),
        vec![ws.to_string()],
        lb_host::Role::Solo,
        Duration::from_secs(5),
    );

    // CHANNEL AGENT REACTOR TICK: drain durable `channel-agent-run` enqueue jobs and drive each run off
    // the reactor, so an in-channel agent run survives the tab closing and (durable + idempotent) a
    // node restart. One detached owner per node on a few-second cadence.
    lb_host::spawn_agent_reactors(node.clone(), vec![ws.to_string()], Duration::from_secs(2));

    // APPROVAL-RELEASE REACTOR TICK: release a rule's `held` gated effect the moment its
    // `needs:approval` item is approved (or discard on reject). Cheap ws-scoped scan; guarded transition.
    lb_host::spawn_approval_reactors(node.clone(), vec![ws.to_string()], Duration::from_secs(2));

    // INSIGHT TS HEAL (one-shot, idempotent): rewrite historical insights whose `ts` landed in the
    // seconds-band `[1e9, 1e12)` ×1000. A no-op once healed, so safe every boot.
    let _ = lb_host::heal_insight_timestamps(&node.store, ws).await;

    // OUTBOX RELAY REACTOR TICK (release scope, gap 1 — previously never booted): drain staged
    // outbox effects through the registered delivery adapters. The RouterTarget dispatches on the
    // effect's opaque `target` string (rule 10): `email` → EmailTarget, `push` → PushTarget. A
    // provider the embedder didn't configure falls back to the logging no-op — the relay still
    // drains (never crash boot, never strand effects); the send is logged, not performed.
    // The email provider: embedder-supplied → the config-selected transport (smtp/postmark) → the
    // logging no-op WITH A LOUD WARNING. Before issue #118 was fixed there was no third option: the
    // logging provider was the only non-test impl, so every email a node "sent" was logged and dropped.
    let email_provider =
        crate::mail::build_email_provider(providers.email.as_ref(), email_transport, &node.store);
    let push_provider: Box<dyn lb_host::PushProvider> = match &providers.push {
        Some(p) => Box::new(p.clone()),
        None => Box::new(lb_host::LoggingPushProvider),
    };
    let mut router = lb_host::RouterTarget::new()
        .route(
            lb_host::EMAIL_TARGET,
            lb_host::EmailTarget::new(email_provider, node.store.clone()),
        )
        .route(
            lb_host::PUSH_TARGET,
            lb_host::PushTarget::new(push_provider, node.store.clone()),
        );
    // Embedder-registered targets, folded in LAST so a host can replace a built-in as well as add to
    // it. The core still routes on the opaque string and knows nothing about what it just registered.
    for (name, target) in &providers.targets {
        router = router.route_dyn(name, target.clone());
    }
    lb_host::spawn_relay_reactors(
        node.clone(),
        vec![ws.to_string()],
        router,
        Duration::from_secs(2),
    );

    // INGEST DRAIN REACTOR TICK (drain-backpressure scope — previously never booted): commit staged
    // samples → the `series` tables off every caller's request path. The ingest scope always named a
    // "commit worker mounted by the ingest role" and `drain.rs` said outright there was no
    // background drain worker — so every CALLER was the worker, draining the whole workspace backlog
    // inside its own call (one sample against a 4,671-row backlog measured 18.5s vs 21ms at backlog
    // 0, and it never recovered: a caller that timed out abandoned only the wait). Callers now drain
    // only their own batch; this tick owns the backlog. A few seconds is ample — a writer's own
    // samples already commit inline, so nothing here is latency-critical.
    lb_host::spawn_ingest_reactors(node.clone(), vec![ws.to_string()], Duration::from_secs(2));

    // RETENTION GC REACTOR TICK (series-sample-cap scope, issue #65 — previously never booted):
    // execute the retention policies an admin already wrote. `run_gc` was reachable only from tests
    // and the on-demand `series.retention.gc` verb, so a correctly-configured horizon evicted
    // NOTHING on a real node unless someone called the verb by hand — the same missing-driver class
    // as the ingest drain above. Without this tick the `max_samples` cap is decorative and a series
    // grows until the disc is full. Slow cadence on purpose: a pass counts rows per series behind
    // the store's global session mutex, and nothing waits on an eviction.
    //
    // The cadence is config (`BootConfig::retention_period`), defaulting to `RETENTION_PERIOD`. It
    // is configurable so it can be EXERCISED — a hardcoded const meant the reactor's own loop could
    // not be observed on a dev box without editing lb — not so it can be run fast; the default is
    // unchanged and the reasons above are why it should stay that way.
    lb_host::spawn_retention_reactors(
        node.clone(),
        vec![ws.to_string()],
        retention_period.unwrap_or(lb_host::RETENTION_PERIOD),
    );

    // STORE-COMPACT REACTOR TICK (online-compaction scope, issue #67): drain `store.compact`
    // jobs off the request path (a pass is whole-log I/O with no upper bound — never inline)
    // and log the log-size advisory past the threshold. The tick itself is cheap (an indexed
    // pending scan + one file stat); a pass runs ONLY when an authorized admin enqueued one —
    // threshold-driven visibility, operator-triggered execution, never compaction-on-a-tick.
    // …and, when the operator set `LB_STORE_MAX_BYTES`, drive the disk budget (disk-budget scope,
    // issue #122): past the soft mark the same reactor enqueues ONE `store.compact` job in this
    // workspace. Unset ⇒ `None` ⇒ inert, exactly the release-1 behaviour. The budget is config,
    // never a code branch (rule 1).
    lb_host::spawn_store_compact_reactors(
        node.clone(),
        vec![ws.to_string()],
        lb_host::STORE_COMPACT_PERIOD,
        store_budget_bytes,
        ws.to_string(),
    );

    // REMINDER REACTOR TICK (previously NEVER BOOTED): fire due reminders on their cron. Without it
    // `react_to_reminders` was reachable only from a test and the manual `reminder.fire` verb, so an
    // authored schedule listed a "next run" it would never reach — the same missing-driver class as
    // the ingest drain and retention GC above. Cadence must be well under a minute: cron granularity
    // is one minute and a missed slot is SKIPPED, not backfilled.
    lb_host::spawn_reminder_reactors(node.clone(), vec![ws.to_string()], lb_host::REMINDER_PERIOD);

    // INSIGHT DIGEST REACTOR TICK: digest the anti-spam ladder — one message per (sub, window), decay
    // quiet keys, post under each sub's stored principal. 30s cadence (windows are hours/days).
    lb_host::spawn_insight_digest_reactors(
        node.clone(),
        vec![ws.to_string()],
        Duration::from_secs(30),
    );

    // DATASOURCE PROFILE REACTOR TICK (datasource-profile scope): keep each source's discovery
    // profile younger than `refresh_after_secs` by enqueueing + draining bounded profiling passes.
    // OFF on two axes — the `datasource-profile` cargo feature must be compiled in AND the embedder
    // must have filled `BootConfig::profile` with `enabled: true`. This one spends work on an
    // EXTERNAL database, so opting in is deliberately explicit twice over. The tick is lazy
    // (minutes): a profile's freshness contract is hours, and a tight loop here is exactly the
    // reactor-rescan CPU burn that pegged a Pi.
    spawn_profile_reactor(node, ws, profile);
}

/// Spawn the discovery-profile reactor when the feature is compiled AND the config enables it.
/// Split into a `#[cfg]` pair so `spawn` above reads the same either way — the feature-off build
/// compiles a no-op, never a branch inside the caller.
#[cfg(feature = "datasource-profile")]
fn spawn_profile_reactor(
    node: &Arc<Node>,
    ws: &str,
    profile: Option<crate::config::ProfileConfig>,
) {
    let Some(cfg) = profile.filter(|c| c.enabled) else {
        return;
    };
    lb_host::spawn_profile_reactors(
        node.clone(),
        vec![ws.to_string()],
        lb_host::PROFILE_PERIOD,
        lb_host::ProfileReactorConfig {
            refresh_after_secs: cfg.refresh_after_secs,
            bounds: lb_host::ProfileBounds {
                max_tables: cfg.max_tables,
                max_values: cfg.max_values,
            },
        },
    );
}

/// Feature-off: the config field is accepted and honoured as a no-op (the `page-cache` posture), so
/// an embedder's config code is identical on a build that left the feature out.
#[cfg(not(feature = "datasource-profile"))]
fn spawn_profile_reactor(
    _node: &Arc<Node>,
    _ws: &str,
    _profile: Option<crate::config::ProfileConfig>,
) {
}
