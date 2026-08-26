//! `spawn_mail_reactors` — the **driver**, without which none of this runs.
//!
//! This platform has shipped three features whose mechanism was complete and whose heartbeat was
//! missing: the ingest drain, series retention, and flow cron triggers. Each was reachable only from
//! a test or an on-demand verb, so a correctly-configured node did nothing. That history is why the
//! reactor is part of this slice rather than a follow-up, and why the poll cadence is per-source
//! state rather than a constant here.
//!
//! One detached owner per node ticks every workspace's sources. A source is polled when
//! `now - last_poll_ts >= poll_seconds`, so cadence is the *source's* config and the tick is only
//! the resolution at which it is honoured. Ticks never overlap (`MissedTickBehavior::Skip`), so a
//! slow mailbox cannot pile passes up behind it.
//!
//! Errors are logged, never fatal: one unreachable mailbox must not stop the node's heartbeat, and
//! must not stop the *other* sources in the same workspace from being polled.

use std::sync::Arc;
use std::time::Duration;

use crate::boot::Node;

use super::fetcher::{build_fetcher, http_client, token_cache};
use super::poll::poll_source;
use super::source::MIN_POLL_SECONDS;
use super::store::list_sources;

/// How often the reactor looks for a source that is due. Not the poll cadence — that is per-source.
/// A tick is one cheap scan of a small table per workspace.
pub const MAIL_TICK: Duration = Duration::from_secs(15);

/// Messages imported per pass. The remainder is not lost: `PollPass::more` makes the next tick
/// continue, so a large first sync drains at this rate rather than in one unbounded pass.
pub const MAIL_BATCH: usize = 25;

/// Spawn the detached mail-poll tick for `workspaces`. Returns immediately; the loop runs for the
/// life of the node.
pub fn spawn_mail_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        // The node's shared token cache + client (see `fetcher::token_cache`): an XOAUTH2 source
        // refreshes its bearer about hourly, and minting one per tick is what providers rate-limit.
        let tokens = token_cache();
        let http = http_client();
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let now = now_wall_ms();
            for ws in &workspaces {
                let sources = match list_sources(&node.store, ws).await {
                    Ok(sources) => sources,
                    Err(e) => {
                        tracing::warn!(ws = %ws, error = %e, "mail: could not list sources");
                        continue;
                    }
                };
                for mut source in sources {
                    if source.paused || !due(&source, now) {
                        continue;
                    }
                    let fetcher = match build_fetcher(&node.store, ws, &source, tokens, http).await
                    {
                        Ok(fetcher) => fetcher,
                        Err(e) => {
                            tracing::warn!(
                                ws = %ws, source = %source.id, error = %e,
                                "mail: could not build the fetcher (credential or config)"
                            );
                            // Record it on the source so the roster shows why, and stamp the
                            // poll time so a broken credential is retried on cadence rather
                            // than on every tick.
                            source.last_poll_ts = now;
                            source.last_error = Some(e.to_string());
                            let _ = super::store::save_source(&node.store, ws, &source).await;
                            continue;
                        }
                    };
                    match poll_source(
                        &node.store,
                        ws,
                        &mut source,
                        fetcher.as_ref(),
                        MAIL_BATCH,
                        now,
                    )
                    .await
                    {
                        Ok(pass) => {
                            // An idle mailbox ticks quietly; anything that happened is logged.
                            if pass.imported > 0 || pass.rejected > 0 || pass.failed > 0 {
                                tracing::info!(
                                    ws = %ws,
                                    source = %source.id,
                                    imported = pass.imported,
                                    duplicates = pass.duplicates,
                                    rejected = pass.rejected,
                                    failed = pass.failed,
                                    samples = pass.samples,
                                    series = pass.series.len(),
                                    more = pass.more,
                                    "mail source imported"
                                );
                            }
                        }
                        Err(e) => tracing::warn!(
                            ws = %ws, source = %source.id, error = %e, "mail source poll failed"
                        ),
                    }
                }
            }
        }
    });
}

/// Is this source due? `last_poll_ts == 0` (never polled) is always due, so a freshly-registered
/// source does not wait a cadence before its first pass.
fn due(source: &super::source::MailSource, now_ms: u64) -> bool {
    if source.last_poll_ts == 0 {
        return true;
    }
    let cadence_ms = source.poll_seconds.max(MIN_POLL_SECONDS) * 1000;
    now_ms.saturating_sub(source.last_poll_ts) >= cadence_ms
}

fn now_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::source::MailSource;
    use super::*;

    fn source(last_poll_ts: u64, poll_seconds: u64) -> MailSource {
        let json = serde_json::json!({
            "id": "s", "host": "h", "username": "u", "secretPath": "p",
            "pollSeconds": poll_seconds, "lastPollTs": last_poll_ts,
        });
        serde_json::from_value(json).expect("source")
    }

    #[test]
    fn a_never_polled_source_is_due_immediately() {
        assert!(due(&source(0, 3600), 1_000));
    }

    #[test]
    fn cadence_is_the_sources_own_setting() {
        let now = 1_000_000;
        assert!(
            !due(&source(now - 30_000, 60), now),
            "30s into a 60s cadence"
        );
        assert!(due(&source(now - 61_000, 60), now));
    }

    #[test]
    fn a_source_configured_below_the_floor_still_honours_the_floor() {
        let now = 1_000_000;
        // The floor exists because providers lock an account that connects too often; a record
        // written before the floor was enforced must not be able to bypass it at runtime.
        let mut fast = source(now - 2_000, 1);
        fast.poll_seconds = 1;
        assert!(
            !due(&fast, now),
            "2s since the last poll is inside the {MIN_POLL_SECONDS}s floor"
        );
    }
}
