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
///
/// `email` (email-login scope): when `Some(non-empty)`, also set the dev admin's GLOBAL email login
/// handle (`identity_set_email`) AND — when a `credential` is present — the GLOBAL password
/// (`identity_credential_set`), so the new `POST /auth/login {email, password}` front door has a first
/// admin who can sign in on a fresh store. The same `credential` plaintext backs both the legacy per-ws
/// `/login` credential above and the global credential here, so one seeded password works on both doors
/// while they coexist. `None` email ⇒ no global email/credential seeded (the identity still logs in via
/// the legacy `/login` or the dev form). Idempotent (upsert); the global email index is race-safe.
pub async fn seed_dev_identity(
    node: &Node,
    ws: &str,
    user: &str,
    credential: Option<&str>,
    email: Option<&str>,
) -> anyhow::Result<()> {
    use lb_authz as raw;
    let store = &node.store;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Register the workspace in the node DIRECTORY so it is listable (email-login scope). Without this
    // `/auth/login`'s `login_workspaces` scan finds no directory row and returns "not a member of any
    // workspace" on a freshly seeded node — the membership exists but the directory entry did not (it
    // used to be created lazily on the first legacy `/login`). Un-gated provisioning write; idempotent.
    if let Err(e) = lb_host::workspace_register(store, ws, ws, ts).await {
        eprintln!("boot seed: workspace directory register for {ws} failed: {e}");
    }
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
    let secret = credential.filter(|s| !s.is_empty());
    if let Some(secret) = secret {
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
    // email-login scope: seed the GLOBAL email handle + (if a password was given) the GLOBAL
    // credential, so the new `/auth/login {email, password}` front door has a first admin. Raw writes
    // (no principal — this IS the provisioning seam), the same records the mediated verbs produce.
    if let Some(email) = email.filter(|e| !e.trim().is_empty()) {
        // Race-safe global-unique email index on the identity (idempotent when already owned by `user`).
        match raw::identity_set_email(store, user, email).await {
            Ok(_) => {}
            // A conflict means the email is already owned by a DIFFERENT identity — a real misconfig,
            // not a re-seed. Log and continue (best-effort seed) rather than abort the whole boot.
            Err(lb_store::StoreError::Conflict) => {
                eprintln!("boot seed: email {email} already owned by another identity — skipped");
            }
            Err(e) => return Err(anyhow::anyhow!("seed email failed: {e}")),
        }
        if let Some(secret) = secret {
            let phc = lb_host::hash_secret(secret)
                .map_err(|e| anyhow::anyhow!("seed global credential hash failed: {e}"))?;
            raw::identity_credential_set(store, user, &phc, ts).await?;
            println!("boot seed: {user} global email {email} + password set (/auth/login-ready)");
        } else {
            println!("boot seed: {user} global email {email} set (no password — dev-login only)");
        }
    }
    println!("boot seed: {user} is a workspace-admin member of {ws}");
    Ok(())
}
