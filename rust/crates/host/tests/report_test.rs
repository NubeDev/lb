//! The report-builder + brand surface, headless (reports scope, "Testing plan"). Real store
//! (`mem://`), real seeded records (rule 9). Proves the mandatory categories: CRUD round-trip over
//! typed blocks, capability-deny **per verb** (save/export/brand.save), two-workspace isolation,
//! `panel_ref` hydration + dangling-ref rejection, brand-seed idempotence, and the block cap.
//!
//! Mirrors `panel_test.rs`'s principal builder and the real store setup.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    brand_delete, brand_get, brand_list, brand_save, panel_save, report_delete, report_export,
    report_get, report_list, report_save, seed_default_brand, BrandColors, BrandError, BrandFonts,
    Cell, PanelSpec, ReportBlock, ReportError, MAX_BLOCKS,
};
use lb_store::Store;
use serde_json::{json, Value};

/// A principal `sub` in workspace `ws` holding `caps`.
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

const R_GET: &str = "mcp:report.get:call";
const R_LIST: &str = "mcp:report.list:call";
const R_SAVE: &str = "mcp:report.save:call";
const R_DELETE: &str = "mcp:report.delete:call";
const R_EXPORT: &str = "mcp:report.export:call";
const B_GET: &str = "mcp:brand.get:call";
const B_LIST: &str = "mcp:brand.list:call";
const B_SAVE: &str = "mcp:brand.save:call";
const B_DELETE: &str = "mcp:brand.delete:call";
// A report that embeds panel refs needs the panel read/save caps too (hydrate/validate re-gate).
const P_SAVE: &str = "mcp:panel.save:call";
const P_GET: &str = "mcp:panel.get:call";

const ALL: &[&str] = &[
    R_GET, R_LIST, R_SAVE, R_DELETE, R_EXPORT, B_GET, B_LIST, B_SAVE, B_DELETE, P_SAVE, P_GET,
];

fn markdown_block(body: &str) -> ReportBlock {
    ReportBlock {
        kind: "markdown".into(),
        body: body.into(),
        ..Default::default()
    }
}

fn image_block(asset_id: &str, caption: &str) -> ReportBlock {
    ReportBlock {
        kind: "image".into(),
        asset_id: asset_id.into(),
        caption: caption.into(),
        ..Default::default()
    }
}

/// A panel block referencing library `panel:{id}` (a ref cell — layout + ref, no spec).
fn panel_ref_block(i: &str, panel_id: &str) -> ReportBlock {
    ReportBlock {
        kind: "panel".into(),
        cell: Cell {
            i: i.into(),
            x: 0,
            y: 0,
            w: 6,
            h: 4,
            panel_ref: format!("panel:{panel_id}"),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn series_spec() -> PanelSpec {
    PanelSpec {
        v: 3,
        widget_type: "chart".into(),
        title: "Cooler temp".into(),
        view: "timeseries".into(),
        ..Default::default()
    }
}

// -----------------------------------------------------------------------------------------------

#[tokio::test]
async fn crud_round_trip() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    // Seed a library panel so the panel block's ref resolves at save (validate) + get (hydrate).
    panel_save(&store, &ada, ws, "cooler", "Cooler", series_spec(), 1)
        .await
        .unwrap();

    let blocks = vec![
        markdown_block("# Summary\n\nHello."),
        image_block("photo-1", "Site photo"),
        panel_ref_block("p1", "cooler"),
    ];
    let saved = report_save(
        &store,
        &ada,
        ws,
        "q3",
        "Q3 Report",
        blocks,
        "brand:default",
        json!({ "range": "24h" }),
        10,
    )
    .await
    .unwrap();
    assert_eq!(saved.blocks.len(), 3);

    let got = report_get(&store, &ada, ws, "q3").await.unwrap();
    assert_eq!(got.title, "Q3 Report");
    assert_eq!(got.blocks.len(), 3);
    // Order preserved + kinds intact.
    assert_eq!(got.blocks[0].kind, "markdown");
    assert_eq!(got.blocks[0].body, "# Summary\n\nHello.");
    assert_eq!(got.blocks[1].kind, "image");
    assert_eq!(got.blocks[1].asset_id, "photo-1");
    assert_eq!(got.blocks[2].kind, "panel");
    // The panel block hydrated: the ref resolved to the library panel's spec (view carried over).
    assert_eq!(got.blocks[2].cell.panel_ref, "panel:cooler");
    assert_eq!(got.blocks[2].cell.view, "timeseries");
    assert!(!got.blocks[2].cell.panel_missing);

    // list shows the summary with a block count.
    let roster = report_list(&store, &ada, ws).await.unwrap();
    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].block_count, 3);

    // delete tombstones it.
    report_delete(&store, &ada, ws, "q3", 20).await.unwrap();
    assert!(matches!(
        report_get(&store, &ada, ws, "q3").await,
        Err(ReportError::NotFound)
    ));
    assert!(report_list(&store, &ada, ws).await.unwrap().is_empty());
}

#[tokio::test]
async fn capability_deny_per_verb() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    // Seed one report (with full caps) so export/get have a target.
    report_save(
        &store,
        &ada,
        ws,
        "r1",
        "R1",
        vec![markdown_block("# hi")],
        "",
        Value::Null,
        1,
    )
    .await
    .unwrap();

    // No report.save → Denied.
    let no_save = principal("user:ben", ws, &[R_GET, R_EXPORT, B_GET]);
    assert!(matches!(
        report_save(&store, &no_save, ws, "r2", "R2", vec![], "", Value::Null, 1).await,
        Err(ReportError::Denied)
    ));

    // No report.export → Denied (view-without-export).
    let no_export = principal("user:cid", ws, &[R_GET, R_SAVE]);
    assert!(matches!(
        report_export(&store, &no_export, ws, "r1", vec![], 1).await,
        Err(ReportError::Denied)
    ));

    // No brand.save → Denied.
    let no_brand = principal("user:dee", ws, &[R_GET, B_GET]);
    assert!(matches!(
        brand_save(
            &store,
            &no_brand,
            ws,
            "b1",
            "B1",
            "",
            BrandColors::default(),
            BrandFonts::default(),
            "",
            "",
            1
        )
        .await,
        Err(BrandError::Denied)
    ));
}

#[tokio::test]
async fn workspace_isolation() {
    let store = Store::memory().await.unwrap();
    let ws_a = "ws:aaa";
    let ws_b = "ws:bbb";
    let ada = principal("user:ada", ws_a, ALL);
    let ben = principal("user:ben", ws_b, ALL);

    report_save(
        &store,
        &ada,
        ws_a,
        "secret",
        "A's report",
        vec![markdown_block("# private")],
        "",
        Value::Null,
        1,
    )
    .await
    .unwrap();
    brand_save(
        &store,
        &ada,
        ws_a,
        "acme",
        "Acme",
        "",
        BrandColors::default(),
        BrandFonts::default(),
        "",
        "",
        1,
    )
    .await
    .unwrap();

    // ws B cannot get / list ws A's report or brand — the hard wall.
    assert!(matches!(
        report_get(&store, &ben, ws_b, "secret").await,
        Err(ReportError::NotFound)
    ));
    assert!(report_list(&store, &ben, ws_b).await.unwrap().is_empty());
    assert!(matches!(
        brand_get(&store, &ben, ws_b, "acme").await,
        Err(BrandError::NotFound)
    ));
    assert!(brand_list(&store, &ben, ws_b).await.unwrap().is_empty());
}

#[tokio::test]
async fn panel_ref_hydration_and_dangling_rejected() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    panel_save(&store, &ada, ws, "real", "Real", series_spec(), 1)
        .await
        .unwrap();

    // A real ref saves + hydrates.
    report_save(
        &store,
        &ada,
        ws,
        "ok",
        "OK",
        vec![panel_ref_block("p1", "real")],
        "",
        Value::Null,
        2,
    )
    .await
    .unwrap();
    let got = report_get(&store, &ada, ws, "ok").await.unwrap();
    assert_eq!(got.blocks[0].cell.view, "timeseries");

    // A dangling ref is rejected loudly on save (BadInput).
    let bad = report_save(
        &store,
        &ada,
        ws,
        "bad",
        "Bad",
        vec![panel_ref_block("p1", "does-not-exist")],
        "",
        Value::Null,
        3,
    )
    .await;
    assert!(matches!(bad, Err(ReportError::BadInput(_))), "got {bad:?}");
}

#[tokio::test]
async fn brand_seed_idempotent() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    seed_default_brand(&store, ws, 1).await.unwrap();
    seed_default_brand(&store, ws, 2).await.unwrap();

    let brands = brand_list(&store, &ada, ws).await.unwrap();
    assert_eq!(
        brands.len(),
        1,
        "seed must be idempotent (one default brand)"
    );
    assert_eq!(brands[0].id, "default");
}

/// The seeded default carries the `SYSTEM_OWNER` sentinel; a save/delete against it ADOPTS it (the
/// writer becomes owner) instead of denying — so the workspace default is brandable in place. Once
/// adopted, the ordinary owner-only wall applies (a different member can no longer overwrite it).
#[tokio::test]
async fn system_owned_seed_is_adopted_on_write() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);
    let ben = principal("user:ben", ws, ALL);

    seed_default_brand(&store, ws, 1).await.unwrap();
    // The seed is system-owned, not Ada's — but she adopts it on save (no Denied).
    let saved = brand_save(
        &store,
        &ada,
        ws,
        "default",
        "Acme Brand",
        "",
        BrandColors::default(),
        BrandFonts::default(),
        "",
        "",
        2,
    )
    .await
    .expect("adopt-on-save must not deny the system-owned seed");
    assert_eq!(saved.name, "Acme Brand");
    assert_eq!(
        brand_get(&store, &ada, ws, "default").await.unwrap().owner,
        "user:ada",
        "the writer adopts ownership of the seed"
    );

    // Now that Ada owns it, Ben (a different member) hits the ordinary owner-only wall.
    let denied = brand_save(
        &store,
        &ben,
        ws,
        "default",
        "Ben Brand",
        "",
        BrandColors::default(),
        BrandFonts::default(),
        "",
        "",
        3,
    )
    .await;
    assert!(matches!(denied, Err(BrandError::Denied)), "got {denied:?}");
}

/// Delete mirrors the save exception: any member with the cap may delete the system-owned seed.
#[tokio::test]
async fn system_owned_seed_can_be_deleted() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    seed_default_brand(&store, ws, 1).await.unwrap();
    brand_delete(&store, &ada, ws, "default", 2)
        .await
        .expect("the system-owned seed is deletable by any writer");
    assert!(matches!(
        brand_get(&store, &ada, ws, "default").await,
        Err(BrandError::NotFound)
    ));
}

#[tokio::test]
async fn max_blocks_enforced() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    let too_many: Vec<ReportBlock> = (0..=MAX_BLOCKS).map(|_| markdown_block("x")).collect();
    let res = report_save(&store, &ada, ws, "big", "Big", too_many, "", Value::Null, 1).await;
    assert!(matches!(res, Err(ReportError::BadInput(_))), "got {res:?}");
}

/// Export a markdown-only report → `%PDF`-prefixed bytes (the assembly path + a real Typst compile).
/// If `lb-render` fails to compile in a given sandbox this test compiles but is the one to check.
#[tokio::test]
async fn export_markdown_only_yields_pdf() {
    let store = Store::memory().await.unwrap();
    let ws = "ws:acme";
    let ada = principal("user:ada", ws, ALL);

    report_save(
        &store,
        &ada,
        ws,
        "doc",
        "Doc",
        vec![
            markdown_block("# Intro\n\nBody text."),
            markdown_block("# Second\n\nMore."),
        ],
        "",
        Value::Null,
        1,
    )
    .await
    .unwrap();

    let pdf = report_export(&store, &ada, ws, "doc", vec![], 1)
        .await
        .expect("export ok");
    assert!(pdf.starts_with(b"%PDF"), "expected PDF magic bytes");
}
