//! `report.export(id, snapshots)` — the branded-PDF export (reports scope, "Branded PDF export").
//! Gated by its **own** `mcp:report.export:call` (an admin can grant view-but-not-export; the PDF
//! embeds data as pixels under the *exporter's* caps). This is a **gateway route**, not the JSON MCP
//! bridge (binary response + snapshot payload don't fit the JSON envelope) — but it still authorizes
//! through the one chokepoint here.
//!
//! Assembly (keeping the `lb-render` crate pure): read + hydrate the report, resolve its brand, and
//! turn each block IN ORDER into an `Assembled` page or image:
//!   - **markdown** → its body markdown, each block its own page (the lazybones page semantics);
//!   - **image** → resolve the `asset:{id}` bytes (`lb_host::get_asset`), add as an `ImageAsset`,
//!     reference via a markdown image;
//!   - **panel** → look up the client-supplied PNG snapshot for that block; add as an `ImageAsset`
//!     and reference it, or render an honest titled placeholder line when no snapshot was supplied.
//! Then `lb_render::render_pdf`.

use lb_auth::Principal;
use lb_render::{render_pdf, Assembled, Brand as RenderBrand, Colors, Fonts, ImageAsset};
use lb_store::Store;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::get::report_get;
use crate::brand::{brand_get, Brand};

/// Export report `id` in `ws` as `principal` to branded PDF bytes. `snapshots` are the client's
/// per-panel-block PNG captures, each `(key, png_bytes)` where `key` is the block's `cell.i` (the
/// stable cell key) — panel blocks whose key has no snapshot degrade to a titled placeholder. `now`
/// is unused today (kept for signature symmetry / future cover-date). Returns `%PDF`-prefixed bytes.
pub async fn report_export(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    snapshots: Vec<(String, Vec<u8>)>,
    _now: u64,
) -> Result<Vec<u8>, ReportError> {
    // The export-specific gate (its own cap — view-without-export is a real posture).
    authorize_report(principal, ws, "report.export")?;

    // Read + hydrate (panel refs resolved under the exporter's gates — report_get re-gates on
    // `report.get`, which the exporter also holds; export is a superset posture in the role bundle).
    let report = report_get(store, principal, ws, id).await?;

    // Resolve the brand (fall back to the neutral default when empty/missing/unreadable).
    let brand = resolve_brand(store, principal, ws, &report.brand_id).await;

    let mut assembled = Assembled::default();
    assembled.title = report.title.clone();
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

    let mut page_titles: Vec<String> = Vec::new();
    for (idx, block) in report.blocks.iter().enumerate() {
        match block.kind.as_str() {
            "markdown" => {
                assembled.pages.push(block.body.clone());
                page_titles.push(first_heading(&block.body));
            }
            "image" => {
                let src = format!("asset:{}", block.asset_id);
                // Resolve the bytes under the exporter's caps; a denied/missing asset drops the
                // image but keeps a caption line so the page is honest, never a crash.
                if let Ok(asset) = crate::get_asset(store, principal, ws, &block.asset_id).await {
                    assembled.images.push(ImageAsset::new(
                        src.clone(),
                        asset_filename(&block.asset_id, &asset.mime),
                        asset.bytes,
                    ));
                    assembled.pages.push(image_markdown(&src, &block.caption));
                } else {
                    assembled
                        .pages
                        .push(format!("_image unavailable_ {}", block.caption));
                }
                page_titles.push(if block.caption.is_empty() {
                    format!("Image {}", idx + 1)
                } else {
                    block.caption.clone()
                });
            }
            "panel" => {
                let key = &block.cell.i;
                let title = if block.cell.title.is_empty() {
                    format!("Panel {}", idx + 1)
                } else {
                    block.cell.title.clone()
                };
                match snapshots.iter().find(|(k, _)| k == key) {
                    Some((_, png)) if !png.is_empty() => {
                        let src = format!("snapshot:{key}");
                        assembled.images.push(ImageAsset::new(
                            src.clone(),
                            format!("{key}.png"),
                            png.clone(),
                        ));
                        assembled.pages.push(image_markdown(&src, &title));
                    }
                    _ => {
                        // No snapshot → honest placeholder (an extension widget in a sandboxed tier
                        // may not be capturable; the export is honest, not failed).
                        assembled
                            .pages
                            .push(format!("**{title}**\n\n_panel snapshot not available_"));
                    }
                }
                page_titles.push(title);
            }
            other => {
                // Unknown kind — keep the report exportable, never fail on a future block kind.
                assembled
                    .pages
                    .push(format!("_unsupported block kind: {other}_"));
                page_titles.push(format!("Block {}", idx + 1));
            }
        }
    }
    assembled.page_titles = page_titles;

    render_pdf(&assembled).map_err(|e| ReportError::Render(e.to_string()))
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

/// A markdown image reference with an optional caption line.
fn image_markdown(src: &str, caption: &str) -> String {
    if caption.is_empty() {
        format!("![]({src})")
    } else {
        format!("![{caption}]({src})\n\n_{caption}_")
    }
}

/// The first `# heading` text of a markdown body, for the index label. Empty when none.
fn first_heading(md: &str) -> String {
    md.lines()
        .find_map(|l| l.trim_start().strip_prefix('#'))
        .map(|h| h.trim_start_matches('#').trim().to_string())
        .unwrap_or_default()
}

/// Pick a virtual filename (extension drives Typst's format detection) from an asset id + mime.
fn asset_filename(id: &str, mime: &str) -> String {
    let ext = ext_for_mime(mime);
    format!("{id}.{ext}")
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
