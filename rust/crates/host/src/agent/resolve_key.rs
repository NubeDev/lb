//! `resolve_endpoint_key` — the ONE place an endpoint's model key is resolved, with the decided
//! precedence **sealed secret → node env → unset** (agent-catalog test-and-secrets scope, open
//! question "where does key resolution live"). Both the in-house model build and the external-agent
//! env handoff route here, so a definition's key resolves identically whether a run or the
//! `agent.def.test` diagnostic asks for it — no divergence between "test passes" and "run works".
//!
//! **Names-only holds by construction.** The inputs are two NAMES — a secret `path` and an env-var
//! `name` — never a value. The value is produced here, at model-call time, and handed to the provider
//! transport; it is never written back to a record, a manifest, or a log. The secret read goes through
//! the shipped sealed `lb_secrets::get` (workspace-scoped, owner-stamped, visibility-gated), so a
//! ws-B caller can never resolve ws-A's sealed key — the hard wall (§7) is inherited, not re-invented.
//!
//! Best-effort on the secret: a `Denied`/`NotFound` for the sealed path falls through to the env
//! (then unset), NEVER an error — a workspace that referenced a path it can't read simply has no
//! sealed key and resolves the env, exactly the fallback the scope names. Only the SHAPE is decided
//! here; whether a real provider then USES the key is the adapter's job (deferred).

use lb_auth::Principal;
use lb_store::Store;

/// Resolve a model endpoint's API key value under `principal` in `ws`, with precedence
/// **`api_key_secret` (sealed) → `api_key_env` (node env) → `None`**.
///
/// - `secret_path` — an optional `lb-secrets` PATH (a name). Read via the sealed `lb_secrets::get`
///   (workspace-scoped, owner-stamped). A denied/absent path falls through to the env.
/// - `env_name` — an optional env-var NAME. Read from the node process env.
///
/// Returns the key VALUE (never logged by this function), or `None` when neither resolves — a clear
/// unconfigured path, not a panic. The value must be handed only to the provider transport.
pub async fn resolve_endpoint_key(
    store: &Store,
    principal: &Principal,
    ws: &str,
    secret_path: Option<&str>,
    env_name: Option<&str>,
) -> Option<String> {
    // (1) Sealed secret first — a workspace that set a key in the UI uses it. Best-effort: a
    // denied/absent path is NOT an error, it just falls through to the env below.
    if let Some(path) = secret_path.filter(|p| !p.is_empty()) {
        if let Ok(value) = lb_secrets::get(store, principal, ws, path).await {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    // (2) Node env var by NAME — the current behavior, the fallback so nothing that works today breaks.
    if let Some(name) = env_name.filter(|n| !n.is_empty()) {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    // (3) Neither → unset. The caller treats this as "no key configured" (honest), never a panic.
    None
}
