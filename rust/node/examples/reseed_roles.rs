//! THROWAWAY dev maintenance one-shot — refresh the built-in role rows in a persistent store.
//!
//! Why this exists: `ensure_builtin_authz_roles` is idempotent (writes a role row ONLY when
//! absent), so a workspace seeded before a new built-in cap was added keeps the stale role record
//! forever — and `resolve_caps` reads that stored record, so a member/admin never gets the new cap.
//! This deletes the three built-in role rows (`viewer`/`member`/`workspace-admin`) in the named
//! workspace(s) and immediately re-seeds them from the CURRENT code (`ensure_builtin_authz_roles`).
//!
//! The store is embedded SurrealKV and is LOCKED by a running node — kill the node first.
//!
//! Usage: `LB_STORE_PATH=... cargo run -p node --example reseed_roles -- acme [other-ws ...]`
//! Delete this file once the affected dev stores are refreshed.

use lb_authz::ROLE_TABLE;
use lb_store::{delete, Store};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var("LB_STORE_PATH")
        .expect("set LB_STORE_PATH to the persistent dev store directory");
    let workspaces: Vec<String> = std::env::args().skip(1).collect();
    if workspaces.is_empty() {
        eprintln!("usage: reseed_roles -- <workspace> [<workspace> ...]");
        std::process::exit(2);
    }

    let store = Store::open(&path).await?;
    for ws in &workspaces {
        for role in ["viewer", "member", "workspace-admin"] {
            // Idempotent: deleting an absent row is fine; we just want them gone before the reseed.
            let _ = delete(&store, ws, ROLE_TABLE, role).await;
        }
        lb_host::ensure_builtin_authz_roles(&store, ws).await?;
        println!("reseeded built-in roles in workspace {ws}");
    }
    Ok(())
}
