//! The document-store slice (scope `docs/scope/document-store/`): markdown docs, binary assets,
//! internal links/embeds, share-to-user, backlinks, and the save↔undo seam — all over the same
//! `assets.*` MCP surface S4 ships, extended additively. Real store, real dispatch (no mocks).
//!
//! Mandatory + load-bearing cases covered:
//!   - **link/embed never widens** (the new deny test): a reader of a doc who lacks access to a
//!     linked doc or an embedded asset sees the parent fine but is DENIED the target — honest
//!     no-access, never a leak;
//!   - **share-to-user + revoke** (the `user` subject): a Private doc shared to an individual is
//!     read by them, denied to others, and revoking the edge removes visibility immediately;
//!   - **binary round-trip**: put an asset, embed it from markdown, `get_asset` returns it
//!     byte-identical; backlinks/embed edges resolve;
//!   - **the save↔undo seam**: a markdown save rev1→rev2 auto-captures and **undo** restores
//!     rev1's body through the real journal (no app-side guessing).

use std::sync::Arc;

use lb_assets::ContentType;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    backlinks, call_tool, delete_doc, get_asset, get_doc, history_list, put_asset, put_doc,
    share_doc, undo, unshare_doc, AssetError, Node,
};
use serde_json::json;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const DOC_R: &str = "store:doc/*:read";
const DOC_W: &str = "store:doc/*:write";
const ASSET_R: &str = "store:asset/*:read";
const ASSET_W: &str = "store:asset/*:write";

/// A markdown save extracts `lb-doc://` / `lb-asset://` refs into `doclink`/`embed` edges, so
/// `backlinks` resolves "what links here" without a second write.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn markdown_save_seeds_backlinks_and_embeds() {
    let ws = "ds-backlinks";
    let store = lb_store::Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DOC_R, DOC_W, ASSET_W]);

    // Target doc + asset exist.
    put_doc(
        &store,
        &ada,
        ws,
        "alarm-matrix",
        "Alarms",
        "matrix",
        ContentType::Text,
        &[],
        1,
    )
    .await
    .unwrap();
    put_asset(&store, &ada, ws, "wiring", "image/png", vec![1, 2, 3], 2)
        .await
        .unwrap();

    // A runbook markdown body references both.
    put_doc(
        &store,
        &ada,
        ws,
        "runbook",
        "Runbook",
        "See lb-doc://alarm-matrix and ![w](lb-asset://wiring).",
        ContentType::Markdown,
        &[],
        3,
    )
    .await
    .unwrap();

    // backlinks(alarm-matrix) names the runbook; the embed edge lets the asset's embedder resolve.
    let bl = backlinks(&store, &ada, ws, "alarm-matrix").await.unwrap();
    assert!(
        bl.iter().any(|s| s == "runbook"),
        "backlink resolves: {bl:?}"
    );
}

/// The load-bearing deny test: a link/embed NEVER widens access. Ben reads the runbook (shared
/// to him) fine, but the alarm-matrix doc he was NOT shared to is DENIED via `get_doc`, and the
/// embedded asset he was NOT shared to is DENIED via `get_asset` — honest no-access, no leak.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn links_and_embeds_never_widen_access() {
    let ws = "ds-no-widen";
    let store = lb_store::Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DOC_R, DOC_W, ASSET_R, ASSET_W]);
    let ben = principal("user:ben", ws, &[DOC_R, ASSET_R]); // read-only

    put_doc(
        &store,
        &ada,
        ws,
        "alarm-matrix",
        "Alarms",
        "matrix",
        ContentType::Text,
        &[],
        1,
    )
    .await
    .unwrap();
    // Ada's private asset — embedded ONLY by a doc Ben canNOT read, so the embed path does not
    // gate Ben in (the load-bearing embed deny).
    put_asset(
        &store,
        &ada,
        ws,
        "secret-img",
        "image/png",
        vec![9, 9, 9],
        2,
    )
    .await
    .unwrap();
    put_doc(
        &store,
        &ada,
        ws,
        "private-notes",
        "Private",
        "![](lb-asset://secret-img)",
        ContentType::Markdown,
        &[],
        3,
    )
    .await
    .unwrap();
    // Ada's runbook links the alarm doc, then is shared to Ben.
    put_doc(
        &store,
        &ada,
        ws,
        "runbook",
        "Runbook",
        "See lb-doc://alarm-matrix.",
        ContentType::Markdown,
        &[],
        4,
    )
    .await
    .unwrap();
    share_doc(&store, &ada, ws, "runbook", "user:ben")
        .await
        .unwrap();

    // Ben reads the shared runbook — the parent doc gate passes.
    assert!(get_doc(&store, &ben, ws, "runbook").await.is_ok());

    // But the LINKED target Ben lacks is DENIED — the link is not a backdoor.
    assert!(matches!(
        get_doc(&store, &ben, ws, "alarm-matrix").await.unwrap_err(),
        AssetError::Denied
    ));
    // And an asset embedded only by a doc Ben lacks is DENIED — the embed is not a backdoor.
    assert!(matches!(
        get_asset(&store, &ben, ws, "secret-img").await.unwrap_err(),
        AssetError::Denied
    ));

    // Ada (owner) reads both the target doc and the embedded asset fine.
    assert!(get_doc(&store, &ada, ws, "alarm-matrix").await.is_ok());
    assert!(get_asset(&store, &ada, ws, "secret-img").await.is_ok());
}

/// An embedded asset IS reachable through the embed path: Ben reads the runbook (shared), and
/// the asset embedded by that runbook is readable to him — because the embedding doc gates him.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_embedded_asset_is_readable_through_the_embedding_doc() {
    let ws = "ds-embed-read";
    let store = lb_store::Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DOC_R, DOC_W, ASSET_R, ASSET_W]);
    let ben = principal("user:ben", ws, &[DOC_R, ASSET_R]);

    put_asset(&store, &ada, ws, "diag", "image/png", vec![1, 2, 3, 4], 1)
        .await
        .unwrap();
    put_doc(
        &store,
        &ada,
        ws,
        "runbook",
        "Runbook",
        "![](lb-asset://diag)",
        ContentType::Markdown,
        &[],
        2,
    )
    .await
    .unwrap();
    share_doc(&store, &ada, ws, "runbook", "user:ben")
        .await
        .unwrap();

    // Ben reads the asset because the runbook (which he may read) embeds it — embed re-gated.
    let a = get_asset(&store, &ben, ws, "diag").await.unwrap();
    assert_eq!(a.bytes, vec![1, 2, 3, 4]);
}

/// Share-to-user + revoke: a Private doc shared to an individual is read by them, denied to a
/// third party, and revoking the edge removes visibility on the next read (live relation).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn share_to_user_then_revoke_removes_visibility() {
    let ws = "ds-user-share";
    let store = lb_store::Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DOC_R, DOC_W]);
    let ben = principal("user:ben", ws, &[DOC_R]);
    let cleo = principal("user:cleo", ws, &[DOC_R]); // never shared to

    put_doc(
        &store,
        &ada,
        ws,
        "d",
        "D",
        "body",
        ContentType::Text,
        &[],
        1,
    )
    .await
    .unwrap();
    share_doc(&store, &ada, ws, "d", "user:ben").await.unwrap();

    // Ben (shared user) reads; Cleo (anyone else) is denied.
    assert_eq!(
        get_doc(&store, &ben, ws, "d").await.unwrap().content,
        "body"
    );
    assert!(matches!(
        get_doc(&store, &cleo, ws, "d").await.unwrap_err(),
        AssetError::Denied
    ));

    // Revoke → Ben is denied on the next read (the relation is re-resolved live).
    unshare_doc(&store, &ada, ws, "d", "user:ben")
        .await
        .unwrap();
    assert!(matches!(
        get_doc(&store, &ben, ws, "d").await.unwrap_err(),
        AssetError::Denied
    ));
}

/// Binary round-trip: a put asset is returned byte-identical, and the size bound rejects an
/// over-large payload with a clear error (never a silent truncation).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn asset_round_trips_byte_identical_and_bounds_size() {
    let ws = "ds-asset-rt";
    let store = lb_store::Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[ASSET_R, ASSET_W]);
    let payload = (0u8..255).collect::<Vec<u8>>();

    put_asset(&store, &ada, ws, "img", "image/png", payload.clone(), 1)
        .await
        .unwrap();
    let got = get_asset(&store, &ada, ws, "img").await.unwrap();
    assert_eq!(got.bytes, payload, "byte-identical round-trip");
    assert_eq!(got.mime, "image/png");

    // Over-bound payload is rejected with TooLarge — a clear, honest error.
    let too_big = vec![0u8; lb_host::MAX_ASSET_BYTES + 1];
    assert!(matches!(
        put_asset(&store, &ada, ws, "huge", "image/png", too_big, 2)
            .await
            .unwrap_err(),
        AssetError::TooLarge
    ));
}

/// Soft-delete: a deleted doc reads as NotFound to a caller who passed the gates (the tombstone
/// is invisible), and is idempotent. Only the owner may delete.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_doc_tombstones_and_is_owner_gated() {
    let ws = "ds-delete";
    let store = lb_store::Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DOC_R, DOC_W]);
    let mallory = principal("user:mallory", ws, &[DOC_R, DOC_W]);

    put_doc(
        &store,
        &ada,
        ws,
        "d",
        "D",
        "body",
        ContentType::Text,
        &[],
        1,
    )
    .await
    .unwrap();

    // A non-owner writer cannot delete someone else's doc.
    assert!(matches!(
        delete_doc(&store, &mallory, ws, "d").await.unwrap_err(),
        AssetError::Denied
    ));

    // The owner deletes; the doc is gone (tombstone reads as NotFound).
    delete_doc(&store, &ada, ws, "d").await.unwrap();
    assert!(matches!(
        get_doc(&store, &ada, ws, "d").await.unwrap_err(),
        AssetError::NotFound
    ));
    // Idempotent: deleting again is still Ok (the tombstone upsert is a no-op).
    delete_doc(&store, &ada, ws, "d").await.unwrap();
}

/// The save↔undo seam (document-store scope + undo scope): a markdown save through the dispatch
/// seam auto-captures a before-image; **undo** restores the prior body via the real journal.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn markdown_save_participates_in_undo() {
    let ws = "ds-undo";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:assets.put_doc:call",
            "mcp:assets.get_doc:call",
            "mcp:undo:call",
            "mcp:history.list:call",
            DOC_R,
            DOC_W,
        ],
    );

    // Save rev1.
    call_tool(
        &node,
        &p,
        ws,
        "assets.put_doc",
        r#"{"id":"notes","title":"Notes","content":"rev1","content_type":"markdown","tags":[],"ts":1}"#,
    )
    .await
    .expect("put_doc rev1");

    // Save rev2.
    call_tool(
        &node,
        &p,
        ws,
        "assets.put_doc",
        r#"{"id":"notes","title":"Notes","content":"rev2","content_type":"markdown","tags":[],"ts":2}"#,
    )
    .await
    .expect("put_doc rev2");

    assert_eq!(
        get_doc(&node.store, &p, ws, "notes").await.unwrap().content,
        "rev2"
    );

    // The save was auto-captured as an undoable step.
    let items = history_list(&node.store, &p, ws, "user:ada", "")
        .await
        .expect("history reads");
    assert!(
        items
            .iter()
            .any(|i| i.tool == "assets.put_doc" && i.undoable),
        "the markdown save is journaled undoable: {items:?}"
    );

    // Undo restores rev1's body through the real journal (no app-side guessing).
    undo(&node.store, &p, ws, "user:ada", "")
        .await
        .expect("undo");
    assert_eq!(
        get_doc(&node.store, &p, ws, "notes").await.unwrap().content,
        "rev1",
        "undo restored the prior markdown body"
    );
}

/// Reusability over the unified MCP contract: an `assets.put_doc` call routed through `call_tool`
/// (the same dispatch path a guest extension's host-callback re-enters) persists markdown into
/// the one store under the caller's capability — proving the substrate is reached the same way
/// the first-party shell reaches it (document-store scope: "one store, many consumers").
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assets_put_doc_is_reachable_over_the_mcp_contract() {
    let ws = "ds-reuse";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal(
        "user:ada",
        ws,
        &[
            "mcp:assets.put_doc:call",
            "mcp:assets.get_doc:call",
            DOC_R,
            DOC_W,
        ],
    );

    let out = call_tool(
        &node,
        &p,
        ws,
        "assets.put_doc",
        r##"{"id":"release-notes","title":"Rel","content":"# v1","content_type":"markdown","tags":["rel"],"ts":1}"##,
    )
    .await
    .expect("assets.put_doc over the bridge");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["id"], "release-notes");

    let got = call_tool(&node, &p, ws, "assets.get_doc", r#"{"id":"release-notes"}"#)
        .await
        .expect("assets.get_doc over the bridge");
    let g: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(g["content"], "# v1");
    assert_eq!(g["content_type"], "markdown");
    assert_eq!(g["tags"], json!(["rel"]));
}

/// Workspace isolation across the new verbs: a ws-B principal cannot read a ws-A asset through
/// `get_asset` (gate 1, structural) — the mandatory isolation test extended to binaries.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_cannot_read_a_ws_a_asset() {
    let ws_a = "ds-iso-a";
    let ws_b = "ds-iso-b";
    let store = lb_store::Store::memory().await.unwrap();
    let ada_a = principal("user:ada", ws_a, &[ASSET_R, ASSET_W]);
    let ada_b = principal("user:ada", ws_b, &[ASSET_R]); // same sub, OTHER workspace

    put_asset(&store, &ada_a, ws_a, "img", "image/png", vec![1, 2, 3], 1)
        .await
        .unwrap();

    // The same principal in ws-B cannot reach ws-A's asset — the workspace wall holds.
    assert!(matches!(
        get_asset(&store, &ada_b, ws_a, "img").await.unwrap_err(),
        AssetError::Denied
    ));
}
