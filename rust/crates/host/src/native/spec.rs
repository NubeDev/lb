//! Build a `lb_supervisor::Spec` from a native manifest, injecting the child's **scoped identity**
//! (native-tier scope). This is the host-side bridge between the manifest's `[native]` block and the
//! supervisor's recipe, and the one place the child's credential is minted.
//!
//! The injected env (`LB_EXT_WS`/`LB_EXT_ID`/`LB_EXT_TOKEN`/`LB_GATEWAY_URL`) is the child's identity
//! + callback address: a token minted carrying exactly `granted = requested ∩ admin_approved` (the
//! same intersection the wasm tier grants), so a compromised child is bounded by its scoped key — it
//! can do nothing the grant forbids when it calls back through the routed MCP namespace
//! (`POST /mcp/call`). The token is per-spawn and never logged or stored (it lives only in the
//! child's env).
//!
//! **native-callback-transport scope:** the token is minted with the **node's signing key** (passed
//! in), NOT a throwaway — so the gateway can VERIFY it on the callback (`session::authenticate`),
//! closing the co-trust gap the native-tier scope deferred. The minter here and the verifier there
//! now share one key (`Node::key`). `LB_GATEWAY_URL` tells the child where to POST its callbacks;
//! unset (no gateway fronting this node) → the child simply has no callback address and its
//! `lb-sidecar-client` calls fail cleanly rather than guessing.

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_ext_loader::{Manifest, Native};
use lb_supervisor::{RestartPolicy, Spec};

/// Build the supervisor spec for `manifest`'s native block, resolving `exec` against `install_dir`
/// and injecting the scoped identity for workspace `ws` with capability set `granted`, signed by the
/// node's `key` so the callback token verifies. The `exec` is joined to `install_dir` unless it is
/// already absolute, so a manifest carries a relative binary name (platform-targets/registry: the
/// artifact's binary lands under the install dir). `gateway_url` (if any) is injected as
/// `LB_GATEWAY_URL` so the child's callback client knows where to POST `/mcp/call`.
pub fn build_spec(
    native: &Native,
    install_dir: &str,
    ws: &str,
    ext_id: &str,
    granted: &[String],
    key: &SigningKey,
    gateway_url: Option<&str>,
) -> Spec {
    let exec = resolve_exec(&native.exec, install_dir);
    let restart = match native.restart.as_str() {
        "never" => RestartPolicy::Never,
        _ => RestartPolicy::OnCrash,
    };

    let token = mint_child_token(key, ws, ext_id, granted);
    let mut spec = Spec::new(exec).with_args(native.args.clone());
    spec.restart = restart;
    spec = spec
        .with_env("LB_EXT_WS", ws)
        .with_env("LB_EXT_ID", ext_id)
        .with_env("LB_EXT_TOKEN", token);
    if let Some(url) = gateway_url {
        spec = spec.with_env("LB_GATEWAY_URL", url);
    }
    spec
}

/// Resolve a manifest `exec` against the install dir (absolute paths pass through). Kept tiny and
/// platform-neutral — a relative name like `echo-sidecar` becomes `<install_dir>/echo-sidecar`.
fn resolve_exec(exec: &str, install_dir: &str) -> String {
    if exec.starts_with('/') || install_dir.is_empty() {
        exec.to_string()
    } else {
        format!("{}/{}", install_dir.trim_end_matches('/'), exec)
    }
}

/// Mint the child's scoped token: a Member principal in `ws` holding exactly `granted`, signed with
/// the **node's** signing `key` so the gateway verifies it on the callback (`POST /mcp/call`). The
/// `sub` is `ext:{ext_id}` (the child acts as itself); `iat=0`/`exp=MAX` like the other in-process
/// tokens (the clock is injected, never wall-clock — the child token is not time-bounded, it is
/// bounded by the process lifetime and its `granted` set).
fn mint_child_token(key: &SigningKey, ws: &str, ext_id: &str, granted: &[String]) -> String {
    let claims = Claims {
        sub: format!("ext:{ext_id}"),
        ws: ws.to_string(),
        role: Role::Member,
        caps: granted.to_vec(),
        iat: 0,
        exp: u64::MAX,
    };
    mint(key, &claims)
}

/// Pull the validated `[native]` block out of a parsed manifest, or `None` if not a native ext.
pub fn native_of(manifest: &Manifest) -> Option<&Native> {
    if manifest.tier == "native" {
        manifest.native.as_ref()
    } else {
        None
    }
}

/// Convenience: collect a manifest's declared tool names (for registering the child's MCP tools).
pub fn tool_names(manifest: &Manifest) -> Vec<String> {
    manifest.tools.iter().map(|t| t.name.clone()).collect()
}
