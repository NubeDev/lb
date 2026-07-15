//! Per-run limits + AI budget from node config (rules-engine-scope: "config, workspace-scoped
//! defaults" — the `env::rules::*` knobs rubix-cube uses). A per-workspace override record is additive
//! later, not v1. Read from `LB_RULES_*` env vars with conservative defaults; never an `if cloud`.

use std::time::Duration;

use lb_rules::{AiLimits, RuleLimits};

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The sandbox governors for a run.
pub fn rule_limits() -> RuleLimits {
    RuleLimits {
        max_operations: env_u64("LB_RULES_MAX_OPERATIONS", 5_000_000),
        max_call_levels: env_usize("LB_RULES_MAX_CALL_LEVELS", 64),
        max_string_bytes: env_usize("LB_RULES_MAX_STRING_BYTES", 256 * 1024),
        max_array_len: env_usize("LB_RULES_MAX_ARRAY_LEN", 100_000),
        max_map_len: env_usize("LB_RULES_MAX_MAP_LEN", 100_000),
        timeout: Duration::from_millis(env_u64("LB_RULES_TIMEOUT_MS", 10_000)),
        // New frame governors (rules crate WIP): take the sandbox's own defaults until the
        // datasources/rules slice wires host config for them.
        ..RuleLimits::default()
    }
}

/// The AI budget for a run.
pub fn ai_limits() -> AiLimits {
    AiLimits {
        max_calls: env_u32("LB_RULES_AI_MAX_CALLS", 8),
        max_tokens: env_u32("LB_RULES_AI_MAX_TOKENS", 20_000),
        context_rows: env_usize("LB_RULES_AI_CONTEXT_ROWS", 200),
    }
}

/// The per-run messaging write budget (`env::rules::MAX_WRITES`, default 32 — rules-messaging-scope
/// "Resolved decisions"). Every motion-producing messaging write (inbox record/resolve, outbox
/// enqueue, channel post/edit/delete) charges it; reads are free. A per-workspace override is additive.
pub fn max_writes() -> u32 {
    env_u32("LB_RULES_MAX_WRITES", 32)
}

/// The sandbox governors for a JOB-BACKED run (long-running-rules-scope) — still bounded, sized
/// for batch: 10 min wall-clock and 100× the op budget by default. Everything else inherits the
/// sync knobs.
pub fn job_rule_limits() -> RuleLimits {
    RuleLimits {
        max_operations: env_u64("LB_RULES_JOB_MAX_OPERATIONS", 500_000_000),
        timeout: Duration::from_millis(env_u64("LB_RULES_JOB_TIMEOUT_MS", 600_000)),
        ..rule_limits()
    }
}

/// The AI budget for a job-backed run (a batch classify wants more than 8 calls; still capped).
pub fn job_ai_limits() -> AiLimits {
    AiLimits {
        max_calls: env_u32("LB_RULES_JOB_AI_MAX_CALLS", 64),
        max_tokens: env_u32("LB_RULES_JOB_AI_MAX_TOKENS", 200_000),
        context_rows: env_usize("LB_RULES_AI_CONTEXT_ROWS", 200),
    }
}

/// The messaging write budget for a job-backed run.
pub fn job_max_writes() -> u32 {
    env_u32("LB_RULES_JOB_MAX_WRITES", 256)
}
