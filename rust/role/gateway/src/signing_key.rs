//! Resolve the node's **token-signing key** at boot so a session survives a node restart against
//! the same persistent store.
//!
//! The bug this closes: [`Gateway::boot`] used to call `SigningKey::generate()` unconditionally, so
//! every process start minted a *fresh random* key. The SurrealKV store is durable
//! (`LB_STORE_PATH`), but the signing key was not — so after any restart the browser's rehydrated
//! session token (signed with the previous key) failed `verify` with `401`, and every read silently
//! fell back to empty (e.g. the agent catalog rendered "No agent definitions available" even though
//! the store still held the 6 seeded defs). See
//! `debugging/auth/signing-key-not-persisted-invalidates-sessions.md`.
//!
//! The fix mirrors how the store itself persists: when `LB_STORE_PATH` names a durable store, keep a
//! 32-byte seed in `<store>.signing-seed` beside it — load it if present, else generate once and
//! write it (owner-only `0600`). With no store path (in-memory dev/test nodes) there is nothing
//! durable to pair a key with, so we keep the ephemeral `generate()` — tests that forge/expire tokens
//! construct their own key via [`Gateway::new`] and are unaffected.
//!
//! This is dev-grade custody (a seed file beside the store), not the deployment secret store —
//! README §13's key-custody question is still open. It is the smallest change that makes the
//! persistent-store dev loop behave like a persistent store.

use std::io::Write;
use std::path::PathBuf;

use lb_auth::SigningKey;

/// The env var selecting a durable on-disk store; when set we pair a durable signing seed with it.
/// Mirrors `lb_host`'s boot-wiring read of the same var (the one config seam §3.1 permits).
const STORE_PATH_ENV: &str = "LB_STORE_PATH";

/// Resolve the node's signing key for this boot. A durable store (`LB_STORE_PATH` set) gets a durable
/// seed beside it so tokens outlive a restart; an ephemeral store keeps a fresh per-process key.
pub fn resolve() -> SigningKey {
    match std::env::var(STORE_PATH_ENV) {
        Ok(path) if !path.is_empty() => persistent(seed_path(&path)),
        // No durable store → nothing to pair a stable key with. Keep the S1 ephemeral behaviour.
        _ => SigningKey::generate(),
    }
}

/// The seed file paired with a store path: `<store>.signing-seed` (a sibling, so wiping the store
/// dir wipes the seed too — a fresh store correctly issues fresh, matching tokens).
fn seed_path(store_path: &str) -> PathBuf {
    let mut p = PathBuf::from(store_path);
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "store".into());
    p.set_file_name(format!("{name}.signing-seed"));
    p
}

/// Load the 32-byte seed at `path` if it exists and is well-formed, else generate one and persist it.
/// Any IO/format problem degrades to a fresh ephemeral key rather than failing boot — a node that
/// cannot persist its key still runs (sessions just won't survive a restart, the old behaviour).
fn persistent(path: PathBuf) -> SigningKey {
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(seed) = <[u8; 32]>::try_from(bytes.as_slice()) {
            return SigningKey::from_seed(&seed);
        }
        // A short/corrupt file: fall through and re-seed (overwrites) rather than run un-persisted.
    }
    let seed = random_seed();
    if let Err(e) = write_seed(&path, &seed) {
        eprintln!(
            "gateway: could not persist signing seed ({e}); sessions won't survive a restart"
        );
        return SigningKey::from_seed(&seed);
    }
    SigningKey::from_seed(&seed)
}

/// 32 random bytes for a fresh signing seed (`rand`'s thread CSPRNG — same source as `generate`).
fn random_seed() -> [u8; 32] {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

/// Write the seed owner-only (`0600` on unix) — the private key half must not be world-readable.
fn write_seed(path: &PathBuf, seed: &[u8; 32]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(seed)?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_auth::{mint, Claims, Role};

    /// A minimal claims set for a mint/verify round-trip (the fields `verify` reads: sub/ws/exp).
    fn claims() -> Claims {
        Claims {
            sub: "user:ada".into(),
            ws: "acme".into(),
            role: Role::Member,
            caps: vec![],
            iat: 0,
            exp: 9_999_999_999,
        }
    }

    /// The regression: a token minted BEFORE a restart must still verify AFTER — i.e. `persistent`
    /// returns the SAME key across two calls for the same seed path (a durable store), which is what
    /// was broken (a fresh random key per boot 401'd every rehydrated browser session).
    #[test]
    fn seed_persists_so_a_pre_restart_token_verifies_after_restart() {
        let dir = std::env::temp_dir().join(format!("lb-signkey-test-{}", std::process::id()));
        let store = dir.join("dev-store");
        let seed = seed_path(store.to_str().unwrap());
        let _ = std::fs::remove_file(&seed);

        // "Boot 1": resolve the key, mint a session token, then drop the key (process exit).
        let key1 = persistent(seed.clone());
        let tok = mint(&key1, &claims());

        // "Boot 2": a brand-new process resolves the key again from the SAME store path.
        let key2 = persistent(seed.clone());

        // The token from boot 1 verifies under boot 2's key — the session survives the restart.
        let principal =
            lb_auth::verify(&key2, &tok, 0).expect("pre-restart token must still verify");
        assert_eq!(principal.sub(), "user:ada");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A different store path is a different node identity → a different key (tokens don't cross
    /// stores). Guards against a global/shared seed.
    #[test]
    fn a_different_store_gets_a_different_key() {
        let dir = std::env::temp_dir().join(format!("lb-signkey-test-b-{}", std::process::id()));
        let a = persistent(seed_path(dir.join("store-a").to_str().unwrap()));
        let b = persistent(seed_path(dir.join("store-b").to_str().unwrap()));
        let tok = mint(&a, &claims());
        assert!(
            lb_auth::verify(&b, &tok, 0).is_err(),
            "a token from store A must NOT verify against store B's key",
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
