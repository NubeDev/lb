//! The cover switch and the letterhead, proved on real PDFs: the template these produce has to
//! COMPILE, and a page count is the only honest evidence that the cover is gone.

use lb_render::{Assembled, Brand, Fonts, ImageAsset, Letterhead, RenderOptions, render_pdf};

fn page_count(pdf: &[u8]) -> usize {
    let s = String::from_utf8_lossy(pdf);
    s.matches("/Type /Page\n").count() + s.matches("/Type/Page").count()
}

fn two_pages() -> Assembled {
    Assembled::with_pages("Monthly Energy", vec!["One.".to_owned(), "Two.".to_owned()])
}

#[test]
fn no_cover_drops_exactly_the_cover_page() {
    let with = render_pdf(&two_pages()).expect("default renders");
    let without = render_pdf(&two_pages().with_options(RenderOptions {
        no_cover: true,
        ..RenderOptions::default()
    }))
    .expect("coverless renders");
    assert_eq!(page_count(&with), page_count(&without) + 1);
}

#[test]
fn a_letterhead_with_every_token_and_hostile_text_compiles() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#1d4ed8"/></svg>"##.to_vec();
    let mut doc = two_pages().with_logo(ImageAsset::new("", "logo.svg", svg));
    doc.letterhead = Some(Letterhead {
        top_left: "${__logo}".into(),
        top_right: "Monthly Energy\n120 Harbour Street - Sydney".into(),
        // Typst-significant characters must print, not execute.
        bottom_left: "a \"quoted\" #line \\ [x] $y$ ${unknown}".into(),
        bottom_right: "Page ${__report.page} of ${__report.pages}".into(),
    });
    let pdf = render_pdf(&doc).expect("letterhead renders");
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn a_logo_token_without_a_logo_still_compiles() {
    let mut doc = two_pages();
    doc.letterhead = Some(Letterhead {
        top_left: "${__logo} Northside".into(),
        ..Letterhead::default()
    });
    render_pdf(&doc).expect("renders with the logo token dropped");
}

/// The letterhead is set in the BRAND'S body font. Proved on a page whose ONLY text is the
/// letterhead, which is what every report page is (placed images, no flowed text): changing nothing
/// but the brand's body font must change the document. This failed before the band named its font
/// explicitly — with no body text to carry the document's font to the page, both renders fell back
/// to the default face and came out byte-identical. (Compared as bytes because the PDF's font names
/// sit inside compressed streams.)
#[test]
fn the_letterhead_is_set_in_the_brands_body_font() {
    let render = |body: &str| {
        let mut doc = Assembled::with_pages("T", vec![String::new()])
            .with_brand(Brand {
                fonts: Fonts {
                    body: body.into(),
                    ..Fonts::default()
                },
                ..Brand::default()
            })
            .with_options(RenderOptions {
                no_cover: true,
                ..RenderOptions::default()
            });
        doc.letterhead = Some(Letterhead {
            top_right: "Northside Facilities".into(),
            bottom_right: "Page ${__report.page} of ${__report.pages}".into(),
            ..Letterhead::default()
        });
        render_pdf(&doc).expect("renders")
    };
    assert_ne!(render("DejaVu Sans Mono"), render("New Computer Modern"));
}
