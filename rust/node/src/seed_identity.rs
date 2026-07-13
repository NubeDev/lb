//! Seed the dev identity as a workspace-admin member — the operator-provisioning boot verb (moved
//! verbatim from `main.rs`). Idempotent; the login gate still enforces membership, this just
//! guarantees the configured dev user IS a member so a fresh OR previously-seeded store logs in cleanly.

use lb_host::Node;

/// Seed the dev `user` as a `workspace-admin` member of `ws`: create the global identity (idempotent),
/// write the membership row (idempotent), and grant the built-in `member` + `workspace-admin` roles
/// (idempotent). Operator provisioning at boot — the login gate still enforces membership; this just
/// guarantees the dev user IS a member so a fresh OR previously-seeded store logs in cleanly.
///
/// `credential` (embedder-credential-mode scope): when `Some(non-empty)`, argon2-hash it into the
/// user's credential record so a `PasswordHash` node has a first admin who can log in (the bootstrap
/// paradox — `identity.set_credential` needs an admin token, unavailable before any credential
/// exists). Written raw here (no principal — this IS the provisioning seam), mirroring the
/// invite-accept onboarding write: the same `credential` table, in `ws`'s namespace, PHC only.
/// `None` seeds no credential (correct for password-less `DevTrustAny` nodes). Idempotent (upsert).
pub async fn seed_dev_identity(
    node: &Node,
    ws: &str,
    user: &str,
    credential: Option<&str>,
) -> anyhow::Result<()> {
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
    // Optionally seed the dev admin's login credential so a `PasswordHash` node lets it log in.
    // Same write the invite-accept onboarding runs: hash the plaintext (never store raw), write a
    // `credential` record keyed by the canonical `user:<name>` sub into `ws`'s namespace (the wall).
    if let Some(secret) = credential.filter(|s| !s.is_empty()) {
        let phc = lb_host::hash_secret(secret)
            .map_err(|e| anyhow::anyhow!("seed credential hash failed: {e}"))?;
        let record = serde_json::json!({
            "sub": user,
            "kind": "credential",
            "phc": phc,
            "set_ts": ts,
        });
        lb_store::write(store, ws, "credential", user, &record).await?;
        println!("boot seed: {user} login credential set (PasswordHash-ready)");
    }
    println!("boot seed: {user} is a workspace-admin member of {ws}");
    Ok(())
}
