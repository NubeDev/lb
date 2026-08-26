//! `report.export(id, snapshots)` — the branded-PDF export (reports-as-dashboards scope, "PDF export
//! follows the record"). Gated by its **own** `mcp:report.export:call` (an admin can grant
//! view-but-not-export; the PDF embeds data as pixels under the *exporter's* caps). This is a
//! **gateway route**, not the JSON MCP bridge (binary response + snapshot payload don't fit the JSON
//! envelope) — but it still authorizes through the one chokepoint here.
//!
//! **`id` addresses a report-kind DASHBOARD**, not the retired `report:{id}` notebook. A report is a
//! dashboard whose record says `kind: "report"`, so the exporter reads it with `dashboard_get` — the
//! same three gates, the same `hydrate_cells`, the same lens — and lays its A4 pages out from the
//! cell grid ([`super::compose`]). The wire contract is unchanged: the same route, the same cap, the
//! same `{ snapshots: [{ cellId, png }] }` body, keyed on `cell.i` exactly as before.
//!
//! An ordinary dashboard is **refused**, not exported. A 12-column board authored for a wide screen
//! laid onto a 166 mm page is not a report, it is a broken PDF; a loud refusal sends the author to
//! "New report" instead of to a bug report.

use lb_auth::Principal;
use lb_render::{render_pdf, Assembled, Brand as RenderBrand, Colors, Fonts, ImageAsset};
use lb_store::Store;

use super::authorize::authorize_report;
use super::compose::compose_pages;
use super::error::ReportError;
use super::rendered::RenderedPanel;
use crate::brand::{brand_get, Brand};
use crate::dashboard::{dashboard_get, DashboardError};

/// Export the report-kind dashboard `id` in `ws` as `principal` to branded PDF bytes.
///
/// `panels` is what the client actually RENDERED — one entry per panel on the page, each with the grid
/// rect it was drawn at and the PNG it captured (empty when it could not be rasterised, in which case an
/// error tile is placed in its rect rather than the panel being dropped). That list, not the stored
/// cells, is the page: see [`super::compose`] for the four ways the two differ and what each looked like
/// in the PDF. A client that sends no geometry falls back to the record, unchanged.
///
/// `now` is unused today (kept for signature symmetry / future cover-date). Returns `%PDF`-prefixed
/// bytes.
pub async fn report_export(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    panels: Vec<RenderedPanel>,
    _now: u64,
) -> Result<Vec<u8>, ReportError> {
    // The export-specific gate (its own cap — view-without-export is a real posture). Checked FIRST,
    // before the record read, so a caller without it learns nothing about what exists.
    authorize_report(principal, ws, "report.export")?;

    // Read + hydrate through the dashboard verb — its own three gates re-run under this principal,
    // so export grants no read it did not already have (the exporter also needs `dashboard.get`).
    let report = dashboard_get(store, principal, ws, id)
        .await
        .map_err(dashboard_err)?;
    if !report.is_report() {
        return Err(ReportError::BadInput(format!(
            "dashboard {id:?} is not a report (kind is {:?}) — only report-kind dashboards export to A4",
            if report.kind.is_empty() { "dashboard" } else { &report.kind }
        )));
    }

    // Reports carry no brand id of their own, so the workspace's default brand applies. `resolve_brand`
    // already treats an empty id as "the default", and a report's branding is a workspace decision,
    // not a per-page one.
    let brand = resolve_brand(store, principal, ws, "").await;

    let mut assembled = Assembled::default();
    assembled.title.clone_from(&report.title);
    assembled.brand = render_brand(&brand);

    // Brand logo bytes → the render logo (best-effort; a missing/unreadable logo just drops it).
    if !brand.logo_asset_id.is_empty() {
        if let Ok(asset) = crate::get_asset(store, principal, ws, &brand.logo_asset_id).await {
            assembled.logo = Some(ImageAsset::new(
                "logo",
                logo_filename(&asset.mime),
                asset.bytes,
            ));
        }
    }

    let (pages, images) = compose_pages(&report.cells, &panels);
    for (src, filename, bytes) in images {
        assembled.images.push(ImageAsset::new(src, filename, bytes));
    }
    // `pages` (markdown) stays parallel to `placements`; a placed page's markdown is never read, but
    // the renderer iterates `pages`, so one empty entry per page is what makes the page exist.
    assembled.pages = vec![String::new(); pages.len()];
    assembled.page_titles = pages.iter().map(|p| p.title.clone()).collect();
    assembled.placements = pages.into_iter().map(|p| p.placements).collect();

    render_pdf(&assembled).map_err(|e| ReportError::Render(e.to_string()))
}

/// Map a dashboard read failure onto the report error. `Denied`/`NotFound` stay opaque exactly as
/// they were — the exporter must not become a probe for which dashboards exist.
fn dashboard_err(e: DashboardError) -> ReportError {
    match e {
        DashboardError::NotFound => ReportError::NotFound,
        DashboardError::BadInput(m) => ReportError::BadInput(m),
        DashboardError::Store(e) => ReportError::Store(e),
        _ => ReportError::Denied,
    }
}

/// Resolve the report's brand, falling back to the neutral default when the id is empty or the
/// record is missing/unreadable (export never fails on a bad brand ref).
async fn resolve_brand(store: &Store, principal: &Principal, ws: &str, brand_id: &str) -> Brand {
    if brand_id.is_empty() {
        return Brand::default();
    }
    brand_get(store, principal, ws, brand_id)
        .await
        .unwrap_or_default()
}

/// Map our stored [`Brand`] onto the render crate's pure [`RenderBrand`].
fn render_brand(b: &Brand) -> RenderBrand {
    RenderBrand {
        colors: Colors {
            primary: b.colors.primary.clone(),
            secondary: String::new(),
            accent: b.colors.accent.clone(),
            text: b.colors.text.clone(),
            background: b.colors.background.clone(),
        },
        fonts: Fonts {
            heading: b.fonts.heading.clone(),
            body: b.fonts.body.clone(),
        },
        header_text: b.header_text.clone(),
        footer_text: b.footer_text.clone(),
    }
}

fn logo_filename(mime: &str) -> String {
    format!("logo.{}", ext_for_mime(mime))
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/svg+xml" => "svg",
        "image/gif" => "gif",
        _ => "png",
    }
}
