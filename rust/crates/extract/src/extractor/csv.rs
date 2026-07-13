//! The CSV extractor (doc-extraction scope) — one delimited sheet → one markdown table. Uses the
//! `csv` crate (pure Rust, already in the workspace) so quoting/embedded-newline handling is real,
//! not a naive split. Always one doc (a CSV is single-part); the cell cap bounds a runaway file.

use crate::error::ExtractError;
use crate::extractor::table::to_markdown_table;
use crate::extractor::DEFAULT_MAX_TABLE_CELLS;
use crate::model::{ExtractOpts, ExtractedDoc};
use crate::trait_def::Extractor;

/// CSV → a single markdown-table doc.
pub struct CsvExtractor;

impl Extractor for CsvExtractor {
    fn id(&self) -> &'static str {
        "csv"
    }

    fn version(&self) -> u32 {
        1
    }

    fn extract(
        &self,
        bytes: &[u8],
        _mime: &str,
        opts: &ExtractOpts,
    ) -> Result<Vec<ExtractedDoc>, ExtractError> {
        let cap = if opts.max_table_cells == 0 {
            DEFAULT_MAX_TABLE_CELLS
        } else {
            opts.max_table_cells
        };
        // `flexible` — real CSVs have ragged rows; the table renderer pads to the widest row rather
        // than the parser rejecting the file (a rejected file would be a `Failed`, over-strict).
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(bytes);
        let mut rows: Vec<Vec<String>> = Vec::new();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| ExtractError::failed(format!("malformed CSV: {e}")))?;
            rows.push(rec.iter().map(|f| f.to_string()).collect());
        }
        let md = to_markdown_table(&rows, cap);
        Ok(vec![ExtractedDoc::whole("", md)])
    }
}
