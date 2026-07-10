//! Seed the dev identity as a workspace-admin member — the operator-provisioning boot verb (moved
//! verbatim from `main.rs`). Idempotent; the login gate still enforces membership, this just
//! guarantees the configured dev user IS a member so a fresh OR previously-seeded store logs in cleanly.

use lb_host::Node;

/// Seed the dev `user` as a `workspace-admin` member of `ws`: create the global identity (idempotent),
/// write the membership row (idempotent), and grant the built-in `member` + `workspace-admin` roles
/// (idempotent). Operator provisioning at boot — the login gate still enforces membership; this just
/// guarantees the dev user IS a member so a fresh OR previously-seeded store logs in cleanly.
pub async fn seed_dev_identity(node: &Node, ws: &str, user: &str) -> anyhow::Result<()> {
    use lb_authz as raw;
    let store = &node.store;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Seed the built-in `member`/`workspace-admin` role records so the role grants below resolve to
    // caps (login-hardening scope). Idempotent; the same seam every login/create path runs.
    lb_host::ensure_builtin_authz_roles(store, ws).await?;
    raw::identity_create(store, user, None, ts).await?;
    raw::membership_add_raw(store, ws, user, ts).await?;
    if let Some(name) = user.strip_prefix("user:") {
        let subject = lb_authz::Subject::User(name.to_string());
        raw::grant_assign(store, ws, &subject, "role:member").await?;
        raw::grant_assign(store, ws, &subject, "role:workspace-admin").await?;
    }
    println!("boot seed: {user} is a workspace-admin member of {ws}");
    Ok(())
}
