//! `POST /packs/upload` end to end against a REAL node + gateway (pack-upload scope, U-pack-upload).
//!
//! No mocks: a real `Node::boot_as(Hub)`, the real router, real minted tokens, real archives built
//! in the test, and the real `pack.validate` / `pack.apply` verbs behind the real caps wall. The
//! multipart body is hand-built here rather than by a client crate on purpose — it pins the exact
//! wire a `curl -F pack=@ems.zip` produces, so this fails if the route stops speaking it.
//!
//! Proves:
//!   - **a zip installs**: upload → the engine's own dry-run report, then `?verb=apply` → a receipt,
//!     and a re-upload of the SAME archive is the idempotent no-op (the engine's decision, unchanged
//!     by the transport);
//!   - **the transport grants no authority (mandatory deny case):** a token holding
//!     `pack.validate` but NOT `pack.apply` gets its preview and a `403` on apply — the same wall
//!     `/mcp/call` enforces, because it IS the same chokepoint;
//!   - **workspace isolation (mandatory):** a pack applied with a ws-A token is invisible to a ws-B
//!     token, which the body never influences (the ws comes from the token);
//!   - **hostile archives die at the door:** zip-slip and a binary member are `400` naming the
//!     member, never a partially-applied pack;
//!   - **the safe default:** no `?verb=` never mutates.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use lb_role_gateway::router;
use std::io::Write;
use tower::ServiceExt; // for `oneshot`
use zip::write::SimpleFileOptions;

/// The caps a pack apply actually rides. The verb caps gate the DISPATCH; the applier then writes
/// each object through its ordinary host seam, which checks the ordinary object cap (a rule save is
/// `store:rule:write` — the pack path gets no shortcut). Both tiers are here because that is the
/// seeded-admin shape; the deny test below strips the apply verb to prove the wall.
const APPLY_CAPS: &[&str] = &[
    "mcp:pack.validate:call",
    "mcp:pack.apply:call",
    "mcp:pack.list:call",
    "mcp:pack.get:call",
    "store:rule:read",
    "store:rule:write",
];

/// A minimal, REAL pack: a manifest plus one rule it references. Kept tiny so the test is about the
/// transport, not about pack content — the pack engine's own suites cover the content.
fn pack_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "pack.yaml",
            "pack: demo\ntitle: Demo\nversion: 1\nrules:\n  - rules/a.rhai\n",
        ),
        ("rules/a.rhai", "// name: A\nlet x = 1;\n"),
    ]
}

fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in members {
        w.start_file(*name, SimpleFileOptions::default())
            .expect("start");
        w.write_all(body).expect("write");
    }
    w.finish().expect("finish").into_inner()
}

fn pack_zip() -> Vec<u8> {
    let members: Vec<(&str, &[u8])> = pack_files()
        .into_iter()
        .map(|(n, b)| (n, b.as_bytes()))
        .collect();
    zip_of(&members)
}

/// Hand-build the `multipart/form-data` body — byte for byte what `curl -F pack=@demo.zip` sends.
fn multipart_upload(uri: &str, archive: &[u8]) -> Request<Body> {
    const BOUNDARY: &str = "----lbPackUploadTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"content-disposition: form-data; name=\"pack\"; filename=\"demo.zip\"\r\n",
    );
    body.extend_from_slice(b"content-type: application/zip\r\n\r\n");
    body.extend_from_slice(archive);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .header("content-length", body.len().to_string())
        .body(Body::from(body))
        .expect("request")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_uploaded_zip_validates_applies_and_re_applies_as_a_noop() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", APPLY_CAPS);

    // 1) The default verb is the DRY RUN — the engine's own report, over an archive it has never
    //    seen, so the decision is `apply` and the plan is non-empty.
    let resp = router(gw.clone())
        .oneshot(bearer(multipart_upload("/packs/upload", &pack_zip()), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = json_body(resp).await;
    assert_eq!(v["pack"], "demo");
    assert_eq!(v["valid"], true);
    assert_eq!(v["decision"], "apply");
    assert!(!v["plan"].as_array().expect("plan").is_empty());

    // 2) `?verb=apply` writes the receipt — the same verb `/mcp/call` would dispatch.
    let resp = router(gw.clone())
        .oneshot(bearer(
            multipart_upload("/packs/upload?verb=apply&ts=1000", &pack_zip()),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let applied: serde_json::Value = json_body(resp).await;
    assert_eq!(applied["pack"], "demo");
    assert_eq!(applied["outcome"], "applied");
    assert_eq!(applied["objects"][0]["outcome"], "applied");

    // 3) The SAME archive again is the engine's idempotent no-op. The transport changed nothing
    //    about the decision — which is the entire claim this route makes.
    let resp = router(gw)
        .oneshot(bearer(multipart_upload("/packs/upload", &pack_zip()), &tok))
        .await
        .unwrap();
    let again: serde_json::Value = json_body(resp).await;
    assert_eq!(again["decision"], "noop");
}

/// MANDATORY deny case. The route is transport, not authority: a caller who may preview but not
/// apply is refused by the SAME wall, at the same chokepoint, with the opaque `403`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caller_without_the_apply_cap_previews_but_cannot_apply() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:mem", "nube", &["mcp:pack.validate:call"]);

    let resp = router(gw.clone())
        .oneshot(bearer(multipart_upload("/packs/upload", &pack_zip()), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "validate is granted");

    let resp = router(gw)
        .oneshot(bearer(
            multipart_upload("/packs/upload?verb=apply", &pack_zip()),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "apply is NOT granted — uploading does not confer it"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unauthenticated_upload_is_refused() {
    let (gw, _key) = gateway().await;
    let resp = router(gw)
        .oneshot(multipart_upload("/packs/upload", &pack_zip()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// MANDATORY isolation case. The workspace comes from the TOKEN; nothing in the archive or the
/// query can reach across. A pack applied in ws A is not in ws B's roster.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pack_applied_in_one_workspace_is_invisible_in_another() {
    let (gw, key) = gateway().await;
    let test = token(&key, "user:test", "nube", APPLY_CAPS);
    let bob = token(&key, "user:bob", "other", APPLY_CAPS);

    let resp = router(gw.clone())
        .oneshot(bearer(
            multipart_upload("/packs/upload?verb=apply&ts=1000", &pack_zip()),
            &test,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // ws B uploads NOTHING and asks the engine what it has: an empty roster.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                serde_json::json!({"tool": "pack.list", "args": {}}),
            ),
            &bob,
        ))
        .await
        .unwrap();
    let roster: serde_json::Value = json_body(resp).await;
    assert_eq!(
        roster["packs"].as_array().map(|a| a.len()),
        Some(0),
        "ws B sees none of ws A's packs"
    );

    // …and the same archive uploaded by ws B is a FIRST apply there, not a no-op — proof the
    // receipt it was compared against is per-workspace.
    let resp = router(gw)
        .oneshot(bearer(multipart_upload("/packs/upload", &pack_zip()), &bob))
        .await
        .unwrap();
    let v: serde_json::Value = json_body(resp).await;
    assert_eq!(v["decision"], "apply");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_zip_slip_archive_is_refused_before_any_verb_runs() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", APPLY_CAPS);

    let hostile = zip_of(&[
        ("pack.yaml", b"pack: demo".as_slice()),
        ("../escape.rhai", b"pwned"),
    ]);
    let resp = router(gw.clone())
        .oneshot(bearer(
            multipart_upload("/packs/upload?verb=apply", &hostile),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(
        body_text(resp).await.contains("safe relative path"),
        "the refusal names the reason"
    );

    // Nothing was applied: the roster is still empty.
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                serde_json::json!({"tool": "pack.list", "args": {}}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    let roster: serde_json::Value = json_body(resp).await;
    assert_eq!(roster["packs"].as_array().map(|a| a.len()), Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_binary_member_is_refused_by_name() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", APPLY_CAPS);

    let bad = zip_of(&[
        ("pack.yaml", b"pack: demo".as_slice()),
        ("logo.png", &[0xff, 0xfe, 0x00]),
    ]);
    let resp = router(gw)
        .oneshot(bearer(multipart_upload("/packs/upload", &bad), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let msg = body_text(resp).await;
    assert!(msg.contains("logo.png"), "{msg}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_request_with_no_archive_says_how_to_send_one() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", APPLY_CAPS);

    let empty = Request::builder()
        .method("POST")
        .uri("/packs/upload")
        .header(
            "content-type",
            "multipart/form-data; boundary=----lbEmptyBoundary",
        )
        .body(Body::from("------lbEmptyBoundary--\r\n"))
        .expect("request");

    let resp = router(gw).oneshot(bearer(empty, &tok)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(resp).await.contains("curl -F pack="));
}
