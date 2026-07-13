//! `docs.extract` at the host layer (doc-extraction scope) — the capability chokepoint, the
//! per-item outcomes, the extraction ledger's idempotency, version-bump re-derivation into the
//! SAME doc ids, per-item failure containment while the job completes, unsupported-mime honesty,
//! and the mandatory workspace-isolation + capability-deny tests.
//!
//! Real store, real caps, real media bytes seeded through the REAL upload path (rule 9 — no mocks,
//! no fakes; extraction has no external so there is nothing to fake at all). Fixtures are the same
//! committed binaries the pure `lb-extract` crate snapshot-tests, read off disk here and uploaded
//! as media so the whole media→doc path is exercised end to end.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_docs_tool, chunk_write, docs_extract, get_doc, get_extraction, media_upload_begin,
    media_upload_commit, ExtractRequest, ItemOutcome,
};
use lb_store::Store;
use sha2::{Digest, Sha256};

// The full grant an extraction caller needs: the verb, media read reach, and doc write.
const CAPS: &[&str] = &[
    "mcp:docs.extract:call",
    "mcp:media.upload:call",
    "mcp:media.get:call",
    "store:media/*:read",
    "store:doc/*:write",
    "store:doc/*:read",
];

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

fn checksum(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(hash.len() * 2);
    for b in &hash {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../extract/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {path}: {e}"))
}

/// Seed a media record with real bytes through the real upload path (begin → chunk → commit), so
/// the extraction reads genuine chunked media. Returns the media id.
async fn seed_media(store: &Store, p: &Principal, ws: &str, mime: &str, bytes: &[u8]) -> String {
    let cs = checksum(bytes);
    let begin = media_upload_begin(store, p, ws, mime, bytes.len() as u64, &cs, None, 100)
        .await
        .expect("begin upload");
    let id = begin["id"].as_str().unwrap().to_string();
    let chunks = begin["chunks"].as_u64().unwrap() as u32;
    let chunk_size = begin["chunk_size"].as_u64().unwrap_or(bytes.len() as u64) as usize;
    for n in 0..chunks {
        let start = n as usize * chunk_size;
        let end = ((n as usize + 1) * chunk_size).min(bytes.len());
        chunk_write(store, ws, &id, n, &bytes[start..end])
            .await
            .expect("chunk write");
    }
    media_upload_commit(store, p, ws, &id, 200)
        .await
        .expect("commit upload");
    id
}

const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

fn req(media: Vec<String>) -> ExtractRequest {
    ExtractRequest {
        media,
        title: None,
        tags: vec!["report".into()],
        split: lb_extract::SplitPolicy::Whole,
        force_version: None,
    }
}

// ── Happy path: PDF media → a derived markdown doc, edged + tagged + ledgered ─────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn extracts_pdf_media_to_markdown_doc() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(
        &store,
        &p,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;

    let result = docs_extract(&store, &p, "acme", &req(vec![media_id.clone()]), 300)
        .await
        .unwrap();
    assert_eq!(result.items.len(), 1);
    let doc_ids = match &result.items[0] {
        ItemOutcome::Extracted {
            doc_ids, reused, ..
        } => {
            assert!(!reused, "first run is a fresh derivation");
            doc_ids.clone()
        }
        other => panic!("expected extracted, got {other:?}"),
    };
    assert_eq!(doc_ids.len(), 1);

    // The derived doc is a real, readable markdown doc carrying the caller's tag.
    let doc = get_doc(&store, &p, "acme", &doc_ids[0]).await.unwrap();
    assert!(doc.content.contains("Quarterly Report"), "{}", doc.content);
    assert!(doc.tags.contains(&"report".to_string()));

    // The provenance ledger records the derivation.
    let led = get_extraction(&store, "acme", &media_id, "pdf-text")
        .await
        .unwrap()
        .expect("ledger record");
    assert_eq!(led.doc_ids, doc_ids);
    assert_eq!(led.extractor_version, 1);
}

// ── Multi-sheet workbook: per-part split yields one doc per sheet, all from one source ────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workbook_per_part_yields_one_doc_per_sheet() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(&store, &p, "acme", XLSX, &fixture("workbook.xlsx")).await;

    let mut request = req(vec![media_id.clone()]);
    request.split = lb_extract::SplitPolicy::PerPart;
    let result = docs_extract(&store, &p, "acme", &request, 300)
        .await
        .unwrap();

    match &result.items[0] {
        ItemOutcome::Extracted { doc_ids, .. } => {
            assert_eq!(doc_ids.len(), 2, "two sheets → two docs");
            // Both derived docs are readable and distinct.
            let a = get_doc(&store, &p, "acme", &doc_ids[0]).await.unwrap();
            let b = get_doc(&store, &p, "acme", &doc_ids[1]).await.unwrap();
            assert!(a.content.contains("Month") || b.content.contains("Month"));
            assert!(a.content.contains("Region") || b.content.contains("Region"));
        }
        other => panic!("expected extracted, got {other:?}"),
    }
}

// ── Idempotency: a second run with the same (checksum, version) is a ledger no-op ─────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rerun_is_idempotent_via_the_ledger() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(
        &store,
        &p,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;

    let first = docs_extract(&store, &p, "acme", &req(vec![media_id.clone()]), 300)
        .await
        .unwrap();
    let first_ids = match &first.items[0] {
        ItemOutcome::Extracted { doc_ids, .. } => doc_ids.clone(),
        other => panic!("{other:?}"),
    };

    let second = docs_extract(&store, &p, "acme", &req(vec![media_id.clone()]), 400)
        .await
        .unwrap();
    match &second.items[0] {
        ItemOutcome::Extracted {
            doc_ids, reused, ..
        } => {
            assert!(*reused, "second run hits the ledger → no-op");
            assert_eq!(doc_ids, &first_ids, "same doc ids reused");
        }
        other => panic!("expected reused extracted, got {other:?}"),
    }
}

// ── Version bump re-derives into the SAME doc ids (derived-data migration) ────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn version_bump_rederives_into_same_doc_ids() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(
        &store,
        &p,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;

    let first = docs_extract(&store, &p, "acme", &req(vec![media_id.clone()]), 300)
        .await
        .unwrap();
    let first_ids = match &first.items[0] {
        ItemOutcome::Extracted { doc_ids, .. } => doc_ids.clone(),
        other => panic!("{other:?}"),
    };

    // Force a version floor ABOVE the current extractor version → the ledger entry is stale →
    // re-derive. The derived doc id is stable, so it lands on the SAME doc (backlinks survive).
    let mut bumped = req(vec![media_id.clone()]);
    bumped.force_version = Some(99);
    let again = docs_extract(&store, &p, "acme", &bumped, 500)
        .await
        .unwrap();
    match &again.items[0] {
        ItemOutcome::Extracted {
            doc_ids, reused, ..
        } => {
            assert!(
                !reused,
                "a version floor above current forces re-derivation"
            );
            assert_eq!(
                doc_ids, &first_ids,
                "re-derivation reuses the same stable doc ids"
            );
        }
        other => panic!("expected re-derived extracted, got {other:?}"),
    }
}

// ── Per-item failure on corrupt input while the job completes ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn corrupt_item_fails_while_job_completes() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    let good = seed_media(
        &store,
        &p,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;
    let bad = seed_media(
        &store,
        &p,
        "acme",
        "application/pdf",
        &fixture("corrupt.pdf"),
    )
    .await;

    let result = docs_extract(
        &store,
        &p,
        "acme",
        &req(vec![good.clone(), bad.clone()]),
        300,
    )
    .await
    .unwrap(); // the JOB completes (Ok), even though one item fails

    assert!(matches!(result.items[0], ItemOutcome::Extracted { .. }));
    match &result.items[1] {
        ItemOutcome::Failed { media_id, .. } => assert_eq!(media_id, &bad),
        other => panic!("expected failed for corrupt item, got {other:?}"),
    }
}

// ── Unsupported mime → honest Unsupported, never an empty doc ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unsupported_mime_is_honest_not_empty() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    // A PNG has no text extractor in v1.
    let media_id = seed_media(&store, &p, "acme", "image/png", &fixture("report.pdf")).await;

    let result = docs_extract(&store, &p, "acme", &req(vec![media_id.clone()]), 300)
        .await
        .unwrap();
    match &result.items[0] {
        ItemOutcome::Unsupported { media_id: m, .. } => assert_eq!(m, &media_id),
        other => panic!("expected unsupported, got {other:?}"),
    }
}

// ── Mandatory: capability deny (no docs.extract cap; and per-item denied read reach) ─────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_extract_without_cap() {
    let store = Store::memory().await.unwrap();
    let owner = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(
        &store,
        &owner,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;

    // Authenticated but without mcp:docs.extract:call → the whole request is denied.
    let mallory = principal("user:mallory", "acme", &["mcp:media.get:call"]);
    let err = docs_extract(&store, &mallory, "acme", &req(vec![media_id]), 300)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_host::ExtractSvcError::Denied), "got {err}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn per_item_denied_when_media_unreadable() {
    let store = Store::memory().await.unwrap();
    let owner = principal("user:alice", "acme", CAPS);
    let readable = seed_media(
        &store,
        &owner,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;
    let mut cannot_read =
        seed_media(&store, &owner, "acme", "text/csv", &fixture("table.csv")).await;

    // A caller with docs.extract + doc write but WITHOUT media.get read reach: every item that
    // needs a media read is denied per-item, the job still completes.
    let no_read = principal(
        "user:bob",
        "acme",
        &["mcp:docs.extract:call", "store:doc/*:write"],
    );
    let result = docs_extract(
        &store,
        &no_read,
        "acme",
        &req(vec![readable.clone(), std::mem::take(&mut cannot_read)]),
        300,
    )
    .await
    .unwrap();
    for item in &result.items {
        assert!(matches!(item, ItemOutcome::Denied { .. }), "got {item:?}");
    }
}

// ── Mandatory: workspace isolation — ws B cannot extract ws A's media ─────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn media_in_other_workspace_is_per_item_denied() {
    let store = Store::memory().await.unwrap();
    let alice = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(
        &store,
        &alice,
        "acme",
        "application/pdf",
        &fixture("report.pdf"),
    )
    .await;

    // Carol in globex, holding the caps IN HER OWN workspace, names ws-A's media id: it is invisible
    // in globex, so the item is denied (no existence leak) and no cross-workspace doc is derived.
    let carol = principal("user:carol", "globex", CAPS);
    let result = docs_extract(&store, &carol, "globex", &req(vec![media_id.clone()]), 300)
        .await
        .unwrap();
    assert!(
        matches!(result.items[0], ItemOutcome::Denied { .. }),
        "got {:?}",
        result.items[0]
    );

    // And nothing was written into globex.
    assert!(get_extraction(&store, "globex", &media_id, "pdf-text")
        .await
        .unwrap()
        .is_none());
}

// ── The MCP bridge routes and shapes the result (the tested seam the gateway/UI call) ─────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_bridge_extracts_and_returns_items() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);
    let media_id = seed_media(&store, &p, "acme", "text/html", &fixture("page.html")).await;

    let input = serde_json::json!({
        "media": media_id,
        "tags": ["kb"],
        "ts": 300
    });
    let out = call_docs_tool(&store, &p, "acme", "docs.extract", &input)
        .await
        .unwrap();
    assert!(out["job_id"].as_str().unwrap().starts_with("docs-extract-"));
    assert_eq!(out["items"][0]["status"], "extracted");
    let doc_id = out["items"][0]["doc_ids"][0].as_str().unwrap();
    let doc = get_doc(&store, &p, "acme", doc_id).await.unwrap();
    assert!(
        doc.content.contains("Cooler Maintenance"),
        "{}",
        doc.content
    );
}
