//! The v1 extractors — one per mime family, one file each (FILE-LAYOUT). Each is a unit struct
//! implementing [`Extractor`](crate::Extractor); the registry (`registry.rs`) owns the mime→impl
//! mapping. All are pure, offline, and deterministic.

mod csv;
mod html;
mod pdf;
mod table;
mod text;
mod xlsx;

pub use csv::CsvExtractor;
pub use html::HtmlExtractor;
pub use pdf::PdfExtractor;
pub use text::TextExtractor;
pub use xlsx::XlsxExtractor;

/// The shared default cap for cells inlined from a tabular source before truncation (CSV + XLSX).
/// A 10k-row × 10-col sheet would inline 100k cells otherwise (scope risk); past this the extractor
/// inlines the first rows and appends an honest `_… N more rows elided …_` marker + the total.
pub(crate) const DEFAULT_MAX_TABLE_CELLS: usize = 5_000;
