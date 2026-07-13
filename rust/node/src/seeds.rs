//! The boot **seed** sequence (moved verbatim from `main.rs`): dev identity, core skills, agent
//! definitions, personas, the legacy active-persona migration, and the default core-skill grants for
//! the boot workspace. All idempotent — safe to run every boot. One function so the boot path reads
//! linearly; each individual seeder lives in `lb-host`.

use std::sync::Arc;

use lb_host::Node;

use crate::config::BootConfig;
use crate::seed_identity::seed_dev_identity;

/// Wall-clock seconds since the Unix epoch — the seed `now` at the binary boundary.
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Run the full boot seed sequence against `node` for `cfg.workspace`. Best-effort per step (a failure
/// is logged, not fatal — matching today's `main.rs`), so a partially-seeded store still boots.
pub async fn run(node: &Arc<Node>, cfg: &BootConfig) {
    let ws = &cfg.workspace;

    // global-identity seed: ensure the configured dev identity is a `workspace-admin` member. The login
    // gate still enforces membership; this just guarantees the dev user IS a member (provisioning, not a
    // login bypass). `seed_user: None` skips it (an embedder that provisions its own identities).
    if let Some(user) = &cfg.seed_user {
        if let Err(e) = seed_dev_identity(node, ws, user, cfg.seed_credential.as_deref()).await {
            eprintln!("boot seed for ws={ws} user={user} failed: {e}");
        }
    }

    // CORE-SKILL SEED: write the embedded `docs/skills/*/SKILL.md` corpus into the reserved system
    // namespace as immutable `skill:core.<name>@<node-version>` records. Idempotent — an already-seeded
    // version is a no-op. `env!("CARGO_PKG_VERSION")` is the node build version (keys the seeder).
    let node_version = env!("CARGO_PKG_VERSION");
    let boot_ts = unix_seconds();
    match lb_host::seed_core_skills(&node.store, node_version, boot_ts).await {
        Ok(ids) => println!(
            "boot: seeded {} core skills @{node_version} ({:?})",
            ids.len(),
            ids
        ),
        Err(e) => eprintln!("boot: core-skill seed failed: {e}"),
    }

    // AGENT-DEFINITION CATALOG seed: boot-seed the built-in agent definitions into the reserved
    // `_lb_agents` namespace. Idempotent (LWW UPSERT); the ONLY writer of that namespace.
    match lb_host::seed_agent_definitions(&node.store).await {
        Ok(ids) => println!("boot: seeded {} agent definitions ({:?})", ids.len(), ids),
        Err(e) => eprintln!("boot: agent-definition seed failed: {e}"),
    }

    // PERSONA CATALOG seed: boot-seed the built-in personas into the reserved `_lb_personas` namespace.
    // Idempotent (LWW UPSERT); the ONLY writer of that namespace.
    match lb_host::seed_personas(&node.store).await {
        Ok(ids) => println!("boot: seeded {} personas ({:?})", ids.len(), ids),
        Err(e) => eprintln!("boot: persona seed failed: {e}"),
    }

    // LEGACY active_persona MIGRATION: one-shot copy of the retired workspace-global toggle into the
    // ws-default prefs axis, then clear it. Idempotent; never overwrites an admin-set axis.
    match lb_host::migrate_active_persona(&node.store).await {
        Ok(migrated) if !migrated.is_empty() => {
            println!("boot: migrated legacy active_persona in {migrated:?}")
        }
        Ok(_) => {}
        Err(e) => eprintln!("boot: active_persona migration failed: {e}"),
    }

    // DEFAULT CORE-SKILL GRANTS for the boot workspace: the dev boot workspace is seeded directly (not
    // through `workspace_create`), so grant the resolved set here too. The set is config
    // (`default_core_skills`); empty ⇒ none. Best-effort + idempotent (revocable grant edges).
    let default_skills = lb_host::resolve_default_core_skills(cfg.default_core_skills.as_deref());
    lb_host::grant_default_core_skills(&node.store, ws, &default_skills).await;
    if !default_skills.is_empty() {
        println!("boot: default core-skill grants for ws={ws}: {default_skills:?}");
    }
}
