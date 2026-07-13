//! The XLSX extractor (doc-extraction scope) — a workbook's sheets → markdown tables, via
//! `calamine` (pure Rust, no C/Excel dependency). Multi-part: [`SplitPolicy::Whole`] (default)
//! renders every sheet as a `## {sheet}` section of ONE doc; [`SplitPolicy::PerPart`] returns one
//! doc per sheet, each keyed by the sheet name so re-derivation lands on the same doc id.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};

use crate::error::ExtractError;
use crate::extractor::table::to_markdown_table;
use crate::extractor::DEFAULT_MAX_TABLE_CELLS;
use crate::model::{ExtractOpts, ExtractedDoc, SplitPolicy};
use crate::trait_def::Extractor;

/// XLSX → markdown table(s), one section or one doc per sheet.
pub struct XlsxExtractor;

impl Extractor for XlsxExtractor {
    fn id(&self) -> &'static str {
        "xlsx"
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
        let mut wb: Xlsx<_> = Xlsx::new(Cursor::new(bytes))
            .map_err(|e| ExtractError::failed(format!("not a readable workbook: {e}")))?;
        let names = wb.sheet_names().to_vec();
        if names.is_empty() {
            return Err(ExtractError::failed("workbook has no sheets"));
        }

        let mut sheets: Vec<(String, String)> = Vec::with_capacity(names.len());
        for name in &names {
            let range = wb
                .worksheet_range(name)
                .map_err(|e| ExtractError::failed(format!("sheet {name:?}: {e}")))?;
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|r| r.iter().map(cell_to_string).collect())
                .collect();
            sheets.push((name.clone(), to_markdown_table(&rows, cap)));
        }

        match opts.split {
            SplitPolicy::PerPart => Ok(sheets
                .into_iter()
                .map(|(name, md)| ExtractedDoc::part(name.clone(), md, name))
                .collect()),
            SplitPolicy::Whole => {
                let title = names.first().cloned().unwrap_or_default();
                let mut body = String::new();
                for (name, md) in &sheets {
                    body.push_str(&format!("## {name}\n\n{md}\n\n"));
                }
                Ok(vec![ExtractedDoc::whole(
                    title,
                    body.trim_end().to_string(),
                )])
            }
        }
    }
}

/// Render one cell deterministically. Floats print without a trailing `.0` when integral (so
/// `3.0` → `3`), matching how a spreadsheet reader sees a whole number — snapshot-stable.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR:{e:?}"),
    }
}
