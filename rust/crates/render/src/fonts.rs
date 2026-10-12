//! The fonts a brand may name — the ones the renderer can actually set type in.
//!
//! There is no system font on a node: a face is either compiled in or it does not exist, and a
//! brand that names one that does not exist prints in the fallback serif without a word. So the
//! set is closed and stated here once. [`BRAND_FONT_FAMILIES`] is the list a brand editor should
//! offer; `tests/brand_fonts_test.rs` proves every name on it has a real Regular and Bold behind it.
//!
//! Three come with `typst-assets`. The rest are bundled from `fonts/` — see its README for what
//! those files are (static, Latin-subset, OFL) and how to rebuild them.

/// Every family a brand's `heading`/`body` font may name, built-in and bundled.
pub const BRAND_FONT_FAMILIES: &[&str] = &[
    "Libertinus Serif",
    "New Computer Modern",
    "DejaVu Sans Mono",
    "Inter",
    "Geist",
    "IBM Plex Sans",
    "Source Serif 4",
    "JetBrains Mono",
    "IBM Plex Mono",
];

/// The bundled faces, one static file per weight.
pub(crate) const BUNDLED: &[&[u8]] = &[
    include_bytes!("../fonts/Inter-Regular.ttf"),
    include_bytes!("../fonts/Inter-Bold.ttf"),
    include_bytes!("../fonts/Geist-Regular.ttf"),
    include_bytes!("../fonts/Geist-Bold.ttf"),
    include_bytes!("../fonts/IBMPlexSans-Regular.ttf"),
    include_bytes!("../fonts/IBMPlexSans-Bold.ttf"),
    include_bytes!("../fonts/SourceSerif4-Regular.ttf"),
    include_bytes!("../fonts/SourceSerif4-Bold.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../fonts/IBMPlexMono-Bold.ttf"),
];
