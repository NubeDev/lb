//! `ExportOptions` — what a caller may ask of a PDF export (report-pagination-and-export-options
//! scope).
//!
//! Until this existed, `report.export` took `{ snapshots }` and nothing else: no paper, no
//! orientation, no margins, no page numbers, no index — even though the renderer had implemented the
//! last two for months and simply had nobody to set them. This is the typed, defaulted vocabulary for
//! all of it, and it converts to the two things the export actually needs: a [`PageGeometry`] to place
//! against and a [`RenderOptions`] to decorate with.
//!
//! **Absent means today.** Every field is serde-defaulted, and `ExportOptions::default()` resolves to
//! A4 portrait, x 22 / top 24 / bottom 22 mm, scale 2, no page numbers, no index — the exact document
//! that shipped. That is a test (`report_export_options_test.rs`), not an intention.
//!
//! Validation is LOUD. An unknown `paper` or `orientation` is a `BadInput` naming the field and the
//! values that exist, never a silent fall back to A4: a caller who asked for Letter and got A4 without
//! being told has a PDF that is wrong in a way they cannot see.
//!
//! One responsibility: the export's option vocabulary and what it resolves to.

use lb_render::geometry::PageGeometry;
use lb_render::RenderOptions;
use serde::{Deserialize, Serialize};

use super::error::ReportError;

/// The page sizes an export may ask for. Millimetres, portrait-oriented; [`ExportOptions::geometry`]
/// turns them on their side when asked.
const PAPERS: &[(&str, f64, f64)] = &[
    ("a4", 210.0, 297.0),
    ("a3", 297.0, 420.0),
    ("a5", 148.0, 210.0),
    ("letter", 215.9, 279.4),
    ("legal", 215.9, 355.6),
    ("tabloid", 279.4, 431.8),
];

/// What a caller may ask of an export. Every field optional; absent ⇒ the shipped A4 document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExportOptions {
    /// `a4` (default) | `a3` | `a5` | `letter` | `legal` | `tabloid`. Empty ⇒ `a4`.
    pub paper: String,
    /// `portrait` (default) | `landscape`. Empty ⇒ `portrait`.
    pub orientation: String,
    /// Left/right margin override, mm. Absent ⇒ the shipped 22 mm.
    pub margin_x_mm: Option<f64>,
    /// Top margin override, mm (it carries the running header). Absent ⇒ the shipped 24 mm.
    pub margin_top_mm: Option<f64>,
    /// Bottom margin override, mm (it carries the running footer). Absent ⇒ the shipped 22 mm.
    pub margin_bottom_mm: Option<f64>,
    /// The layout-to-paper reduction. Absent ⇒ the shipped `REPORT_PRINT_SCALE`. A caller that raises
    /// it fits more board on a page and shrinks the type; the client's preview is what makes a bad
    /// choice visible.
    pub scale: Option<f64>,
    /// Print a page number in every footer. The renderer has always supported it; nothing set it.
    pub page_numbers: bool,
    /// Prepend a table-of-contents index page. Likewise.
    pub index: bool,
}

impl ExportOptions {
    /// The page box this export places and renders against.
    ///
    /// # Errors
    /// [`ReportError::BadInput`] naming the offending field when `paper` or `orientation` is not one
    /// of the known values, or when a margin/scale is not a usable positive number.
    pub fn geometry(&self) -> Result<PageGeometry, ReportError> {
        let default = PageGeometry::a4_portrait();

        let paper = if self.paper.is_empty() {
            "a4"
        } else {
            self.paper.as_str()
        };
        let (_, w_mm, h_mm) = PAPERS
            .iter()
            .find(|(name, _, _)| name.eq_ignore_ascii_case(paper))
            .ok_or_else(|| {
                ReportError::BadInput(format!(
                    "options.paper {paper:?} is not a known paper size — expected one of {}",
                    PAPERS
                        .iter()
                        .map(|(n, _, _)| *n)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;

        let orientation = if self.orientation.is_empty() {
            "portrait"
        } else {
            self.orientation.as_str()
        };
        let landscape = match orientation.to_ascii_lowercase().as_str() {
            "portrait" => false,
            "landscape" => true,
            other => {
                return Err(ReportError::BadInput(format!(
                    "options.orientation {other:?} is not known — expected portrait or landscape"
                )));
            }
        };

        let mut geo = PageGeometry {
            w_mm: *w_mm,
            h_mm: *h_mm,
            margin_x_mm: positive("options.marginXMm", self.margin_x_mm, default.margin_x_mm)?,
            margin_top_mm: positive(
                "options.marginTopMm",
                self.margin_top_mm,
                default.margin_top_mm,
            )?,
            margin_bottom_mm: positive(
                "options.marginBottomMm",
                self.margin_bottom_mm,
                default.margin_bottom_mm,
            )?,
            scale: positive("options.scale", self.scale, default.scale)?,
        };
        if landscape {
            geo = geo.landscape();
        }

        // A margin set larger than the paper leaves no content box; every rect would then be placed
        // into negative space and Typst would paint them over each other. Caught here, named, rather
        // than discovered as an unreadable PDF.
        if geo.content_w_mm() <= 0.0 || geo.content_h_mm() <= 0.0 {
            return Err(ReportError::BadInput(format!(
                "options margins leave no printable area on {paper} ({:.0}×{:.0}mm)",
                geo.w_mm, geo.h_mm
            )));
        }
        Ok(geo)
    }

    /// The renderer's decoration toggles. These two already rendered; `report_export` simply never set
    /// them, which is why "page numbers" was a finished feature nobody could turn on.
    #[must_use]
    pub fn render_options(&self) -> RenderOptions {
        RenderOptions {
            page_numbers: self.page_numbers,
            index: self.index,
        }
    }
}

/// A caller-supplied override that must be a usable positive number, or the shipped default.
fn positive(field: &str, given: Option<f64>, fallback: f64) -> Result<f64, ReportError> {
    match given {
        None => Ok(fallback),
        Some(v) if v.is_finite() && v > 0.0 => Ok(v),
        Some(v) => Err(ReportError::BadInput(format!(
            "{field} must be a positive number, got {v}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE BACKWARD-COMPATIBILITY GUARANTEE, at the geometry level. Absent options must resolve to
    /// the exact page that shipped — not merely to "an A4-ish page".
    #[test]
    fn absent_options_resolve_to_exactly_the_shipped_a4_page() {
        let g = ExportOptions::default().geometry().unwrap();
        assert_eq!(g, PageGeometry::a4_portrait());
        assert_eq!(g.rows_per_page(), 28);
        assert_eq!(g.content_w_mm(), 166.0);
        assert_eq!(g.content_h_mm(), 251.0);
        assert_eq!(
            ExportOptions::default().render_options(),
            RenderOptions::default()
        );
    }

    /// An omitted field is not the same as a field the caller set — but for these, absent and the
    /// explicit default must agree, or a client that sends its whole profile every time would get a
    /// different document from one that sends nothing.
    #[test]
    fn spelling_the_defaults_out_gives_the_same_page_as_omitting_them() {
        let spelled = ExportOptions {
            paper: "a4".into(),
            orientation: "portrait".into(),
            margin_x_mm: Some(22.0),
            margin_top_mm: Some(24.0),
            margin_bottom_mm: Some(22.0),
            scale: Some(2.0),
            page_numbers: false,
            index: false,
        };
        assert_eq!(
            spelled.geometry().unwrap(),
            ExportOptions::default().geometry().unwrap()
        );
    }

    #[test]
    fn a_named_paper_and_orientation_produce_that_page() {
        let letter = ExportOptions {
            paper: "letter".into(),
            ..ExportOptions::default()
        }
        .geometry()
        .unwrap();
        assert_eq!((letter.w_mm, letter.h_mm), (215.9, 279.4));

        let land = ExportOptions {
            paper: "a4".into(),
            orientation: "landscape".into(),
            ..ExportOptions::default()
        }
        .geometry()
        .unwrap();
        assert_eq!((land.w_mm, land.h_mm), (297.0, 210.0));
        // A shorter page holds fewer rows — the seam the client has to compute rather than hardcode.
        assert!(land.rows_per_page() < 28);
    }

    /// LOUD, not silent. A caller who asked for something that does not exist must be told which
    /// field was wrong — a quiet fall back to A4 is a PDF that is wrong invisibly.
    #[test]
    fn an_unknown_paper_or_orientation_is_a_named_bad_input_not_a_fallback() {
        let paper = ExportOptions {
            paper: "a4-ish".into(),
            ..ExportOptions::default()
        }
        .geometry()
        .unwrap_err();
        let msg = format!("{paper:?}");
        assert!(msg.contains("options.paper"), "must name the field: {msg}");
        assert!(msg.contains("a4"), "must list what exists: {msg}");

        let orient = ExportOptions {
            orientation: "sideways".into(),
            ..ExportOptions::default()
        }
        .geometry()
        .unwrap_err();
        assert!(format!("{orient:?}").contains("options.orientation"));
    }

    #[test]
    fn a_nonsense_margin_or_scale_is_refused_by_name() {
        for (opts, field) in [
            (
                ExportOptions {
                    margin_x_mm: Some(-1.0),
                    ..ExportOptions::default()
                },
                "options.marginXMm",
            ),
            (
                ExportOptions {
                    scale: Some(0.0),
                    ..ExportOptions::default()
                },
                "options.scale",
            ),
            (
                ExportOptions {
                    scale: Some(f64::NAN),
                    ..ExportOptions::default()
                },
                "options.scale",
            ),
        ] {
            let err = format!("{:?}", opts.geometry().unwrap_err());
            assert!(err.contains(field), "expected {field} named, got {err}");
        }
    }

    #[test]
    fn margins_that_swallow_the_page_are_refused_rather_than_placed_into_nothing() {
        let err = ExportOptions {
            margin_x_mm: Some(200.0),
            ..ExportOptions::default()
        }
        .geometry()
        .unwrap_err();
        assert!(format!("{err:?}").contains("printable area"));
    }

    /// The wire shape is camelCase, and an older client's `{ snapshots }`-only body deserialises to
    /// the default rather than failing.
    #[test]
    fn the_wire_shape_is_camel_case_and_every_field_is_optional() {
        let from_empty: ExportOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(from_empty, ExportOptions::default());

        let parsed: ExportOptions = serde_json::from_str(
            r#"{"paper":"letter","orientation":"landscape","marginXMm":10,"pageNumbers":true}"#,
        )
        .unwrap();
        assert_eq!(parsed.paper, "letter");
        assert_eq!(parsed.margin_x_mm, Some(10.0));
        assert!(parsed.page_numbers);
        assert!(!parsed.index);
    }
}
