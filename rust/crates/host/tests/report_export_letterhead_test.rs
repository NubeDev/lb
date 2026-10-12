//! The cover switch, the brand pick and the letterhead on `report.export` (report-letterhead),
//! headless against a real store and a real Typst compile.
//!
//! Each test asserts the OUTPUT CHANGES, for the reason `report_export_options_test.rs` gives: an
//! option that is parsed and never plumbed still "exports successfully".

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    brand_save, dashboard_save_meta, report_export, BrandColors, BrandFonts, Cell, ExportOptions,
    LetterheadOptions, PageMeta, RenderedPanel, ReportError,
};
use lb_store::Store;

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ALL: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.save:call",
    "mcp:dashboard.list:call",
    "mcp:report.export:call",
    "mcp:brand.get:call",
    "mcp:brand.save:call",
];

/// `cover: false` removes exactly one page, and an absent `cover` is the cover that always shipped.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cover_false_drops_exactly_the_cover_page() {
    let (store, p, ws) = fixture("energy", cells()).await;

    let shipped = export(&store, &p, ws, "energy", &ExportOptions::default()).await;
    let spelled = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            cover: Some(true),
            ..ExportOptions::default()
        },
    )
    .await;
    let coverless = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            cover: Some(false),
            ..ExportOptions::default()
        },
    )
    .await;

    assert_eq!(shipped, spelled, "cover: true is the shipped document");
    assert_eq!(page_count(&coverless) + 1, page_count(&shipped));
}

/// A letterhead reaches the page: the same export with and without one is a different document.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_letterhead_reaches_the_renderer() {
    let (store, p, ws) = fixture("energy", cells()).await;

    let plain = export(&store, &p, ws, "energy", &ExportOptions::default()).await;
    let lettered = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            letterhead: Some(LetterheadOptions {
                top_right: "120 Harbour Street - Sydney".into(),
                bottom_right: "Page ${__report.page} of ${__report.pages}".into(),
                ..LetterheadOptions::default()
            }),
            ..ExportOptions::default()
        },
    )
    .await;

    assert_ne!(plain, lettered);
    assert_eq!(page_count(&plain), page_count(&lettered));
}

/// `brandId` picks the brand. Before this an export always used the neutral default, so a saved
/// brand could not reach a report at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn brand_id_selects_the_brand_and_an_unknown_one_falls_back() {
    let (store, p, ws) = fixture("energy", cells()).await;
    brand_save(
        &store,
        &p,
        ws,
        "northside",
        "Northside",
        "",
        BrandColors {
            primary: "#b91c1c".into(),
            ..BrandColors::default()
        },
        BrandFonts::default(),
        "Northside Facilities",
        "",
        1,
    )
    .await
    .expect("brand save ok");

    let neutral = export(&store, &p, ws, "energy", &ExportOptions::default()).await;
    let branded = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            brand_id: "northside".into(),
            ..ExportOptions::default()
        },
    )
    .await;
    let missing = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            brand_id: "no-such-brand".into(),
            ..ExportOptions::default()
        },
    )
    .await;

    assert_ne!(
        neutral, branded,
        "the picked brand must change the document"
    );
    assert_eq!(
        neutral, missing,
        "an unreadable brand is the neutral default, never a failure"
    );
}

/// A brand's logo may be INLINE: the shell's brand editor stores a small logo as a `data:` URI in
/// `logoAssetId` rather than as an asset id. Looked up as an asset it resolves to nothing, so a logo
/// uploaded there never printed. It must reach the page like an asset-backed one does.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_inline_data_uri_logo_reaches_the_page() {
    use base64::Engine as _;
    let (store, p, ws) = fixture("energy", cells()).await;
    let uri = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(one_px_png())
    );
    for (id, logo) in [("plain", ""), ("logo", uri.as_str())] {
        brand_save(
            &store,
            &p,
            ws,
            id,
            id,
            logo,
            BrandColors::default(),
            BrandFonts::default(),
            "",
            "",
            1,
        )
        .await
        .expect("brand save ok");
    }
    let with = |brand: &str| ExportOptions {
        brand_id: brand.into(),
        letterhead: Some(LetterheadOptions {
            top_left: "${__logo}".into(),
            ..LetterheadOptions::default()
        }),
        ..ExportOptions::default()
    };

    let plain = export(&store, &p, ws, "energy", &with("plain")).await;
    let logo = export(&store, &p, ws, "energy", &with("logo")).await;
    assert_ne!(plain, logo, "the inline logo must be drawn");
}

/// `options.logo` is the logo of LAST RESORT: drawn when the brand supplies none (or no brand is
/// picked), and never instead of a brand's own. A shell that keeps its workspace logo somewhere lb
/// does not read can hand it over this way, so a letterhead is not logo-less until someone re-uploads
/// the same image into a brand.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_fallback_logo_fills_in_for_a_brand_without_one_and_never_overrides_one() {
    use base64::Engine as _;
    let (store, p, ws) = fixture("energy", cells()).await;
    let b64 = base64::engine::general_purpose::STANDARD.encode(one_px_png());
    let png_uri = format!("data:image/png;base64,{b64}");
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#1d4ed8"/></svg>"##;
    let svg_uri = format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    );
    brand_save(
        &store,
        &p,
        ws,
        "has-logo",
        "Has logo",
        &png_uri,
        BrandColors::default(),
        BrandFonts::default(),
        "",
        "",
        1,
    )
    .await
    .expect("brand save ok");
    let with = |brand: &str, logo: &str| ExportOptions {
        brand_id: brand.into(),
        logo: logo.into(),
        letterhead: Some(LetterheadOptions {
            top_left: "${__logo}".into(),
            ..LetterheadOptions::default()
        }),
        ..ExportOptions::default()
    };

    let none = export(&store, &p, ws, "energy", &with("", "")).await;
    let fallback = export(&store, &p, ws, "energy", &with("", &svg_uri)).await;
    assert_ne!(none, fallback, "with no brand, the fallback logo is drawn");

    let brand_only = export(&store, &p, ws, "energy", &with("has-logo", "")).await;
    let brand_and_fallback = export(&store, &p, ws, "energy", &with("has-logo", &svg_uri)).await;
    assert_eq!(
        brand_only, brand_and_fallback,
        "a brand's own logo always wins"
    );
}

/// A fallback logo that is not a decodable image `data:` URI, or is unreasonably large, is the
/// caller's mistake and is named — not swallowed into a PDF with a silent gap where the logo was.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bad_or_oversized_fallback_logo_is_a_named_bad_input() {
    let (store, p, ws) = fixture("energy", cells()).await;
    let huge = format!("data:image/png;base64,{}", "A".repeat(3 * 1024 * 1024));
    for logo in [
        "https://example.com/logo.png",
        "data:image/png;base64,***",
        huge.as_str(),
    ] {
        let err = report_export(
            &store,
            &p,
            ws,
            "energy",
            vec![],
            &ExportOptions {
                logo: logo.into(),
                ..ExportOptions::default()
            },
            1,
        )
        .await
        .expect_err("a bad fallback logo is refused");
        assert!(
            matches!(&err, ReportError::BadInput(m) if m.contains("options.logo")),
            "expected options.logo named, got {err:?}"
        );
    }
}

// ---- helpers ------------------------------------------------------------------------------------

fn cell(i: &str, x: u32, y: u32, w: u32, h: u32, title: &str, break_before: bool) -> Cell {
    Cell {
        i: i.into(),
        x,
        y,
        w,
        h,
        title: title.into(),
        view: "stat".into(),
        page_break_before: break_before,
        ..Cell::default()
    }
}

fn cells() -> Vec<Cell> {
    vec![
        cell("p1", 0, 0, 6, 5, "Energy", false),
        cell("p2", 6, 0, 6, 5, "Demand", false),
    ]
}

async fn fixture(id: &str, cells: Vec<Cell>) -> (Store, Principal, &'static str) {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);
    save(&store, &p, ws, id, cells).await;
    (store, p, ws)
}

async fn save(store: &Store, p: &Principal, ws: &str, id: &str, cells: Vec<Cell>) {
    dashboard_save_meta(
        store,
        p,
        ws,
        id,
        "Report",
        PageMeta {
            kind: Some("report".into()),
            ..PageMeta::default()
        },
        cells,
        vec![],
        1,
    )
    .await
    .expect("dashboard save ok");
}

async fn export(
    store: &Store,
    p: &Principal,
    ws: &str,
    id: &str,
    options: &ExportOptions,
) -> Vec<u8> {
    let pdf = report_export(
        store,
        p,
        ws,
        id,
        vec![RenderedPanel {
            cell_id: "p1".into(),
            png: one_px_png(),
            ..RenderedPanel::default()
        }],
        options,
        1,
    )
    .await
    .expect("export ok");
    assert!(pdf.starts_with(b"%PDF"), "expected PDF magic bytes");
    pdf
}

/// How many pages the PDF has. `/Type /Page` but never `/Type /Pages` (the tree node).
fn page_count(pdf: &[u8]) -> usize {
    let s = String::from_utf8_lossy(pdf);
    s.match_indices("/Type/Page")
        .filter(|(i, _)| !s[*i..].starts_with("/Type/Pages"))
        .count()
        + s.match_indices("/Type /Page")
            .filter(|(i, _)| !s[*i..].starts_with("/Type /Pages"))
            .count()
}

fn one_px_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc,
        0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}
