//! The bundled brand fonts actually reach a PDF.
//!
//! A brand names its font; the renderer can only set type in a face it carries. Before these were
//! bundled, every family below silently printed in the fallback serif — which is exactly what a
//! render with a made-up family name does, so "differs from that" is the proof each one is real.
//! Bold is checked separately: a family with only its Regular registered would print a brand's
//! headings in a faked or fallback weight.

use lb_render::{
    Assembled, BRAND_FONT_FAMILIES, Brand, Fonts, Letterhead, RenderOptions, render_pdf,
};

/// A page whose only text is the letterhead (a report page: placed images, no flowed text).
fn letterhead_only(body_font: &str) -> Vec<u8> {
    let mut doc = Assembled::with_pages("T", vec![String::new()])
        .with_brand(Brand {
            fonts: Fonts {
                body: body_font.into(),
                ..Fonts::default()
            },
            ..Brand::default()
        })
        .with_options(RenderOptions {
            no_cover: true,
            ..RenderOptions::default()
        });
    doc.letterhead = Some(Letterhead {
        top_right: "Northside Facilities 120 Harbour Street".into(),
        ..Letterhead::default()
    });
    render_pdf(&doc).expect("renders")
}

/// A document with a bold heading in `font`, and nothing else.
fn heading_only(font: &str) -> Vec<u8> {
    let doc = Assembled::with_pages("T", vec!["# Monthly Energy".to_owned()])
        .with_brand(Brand {
            fonts: Fonts {
                heading: font.into(),
                ..Fonts::default()
            },
            ..Brand::default()
        })
        .with_options(RenderOptions {
            no_cover: true,
            ..RenderOptions::default()
        });
    render_pdf(&doc).expect("renders")
}

#[test]
fn every_advertised_family_is_set_in_its_own_face_not_the_fallback() {
    let fallback = letterhead_only("No Such Family");
    let fallback_bold = heading_only("No Such Family");
    assert!(
        BRAND_FONT_FAMILIES.len() >= 9,
        "the six bundled + the three built-in"
    );
    for family in BRAND_FONT_FAMILIES {
        // The renderer's own fallback face IS the fallback, so it cannot differ from it.
        if *family == "Libertinus Serif" {
            continue;
        }
        assert_ne!(
            letterhead_only(family),
            fallback,
            "{family} regular fell back"
        );
        assert_ne!(
            heading_only(family),
            fallback_bold,
            "{family} bold fell back"
        );
    }
}

#[test]
fn two_bundled_families_do_not_print_alike() {
    assert_ne!(letterhead_only("Inter"), letterhead_only("Geist"));
    assert_ne!(
        letterhead_only("IBM Plex Sans"),
        letterhead_only("Source Serif 4")
    );
}
