//! Render rows of string cells into a GitHub-flavored markdown table, capped (doc-extraction
//! scope: "size caps for embedded tables — a 10k-row sheet should summarize + link, not inline").
//! Shared by the CSV and XLSX extractors — a real shared concept (tabular → markdown), one owner,
//! not a `utils` drawer. Deterministic: same rows → same string.

/// Render `rows` (first row treated as the header) as a markdown table, inlining at most
/// `max_cells` cells. Past the cap the table is truncated at a whole-row boundary and an honest
/// elision marker names how many rows were dropped, so a reader knows the doc is a summary and the
/// original (one edge away) is the full data. An empty `rows` renders `_(empty table)_`.
pub(crate) fn to_markdown_table(rows: &[Vec<String>], max_cells: usize) -> String {
    if rows.is_empty() || rows.iter().all(|r| r.is_empty()) {
        return "_(empty table)_".to_string();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return "_(empty table)_".to_string();
    }
    // How many rows fit under the cell cap (at least the header + one row, so a table is never
    // rendered as pure elision).
    let per_row = cols.max(1);
    let max_rows = (max_cells / per_row).max(2);
    let shown = rows.len().min(max_rows);

    let mut out = String::new();
    render_row(&mut out, &rows[0], cols);
    render_divider(&mut out, cols);
    for row in &rows[1..shown] {
        render_row(&mut out, row, cols);
    }
    if shown < rows.len() {
        let elided = rows.len() - shown;
        out.push_str(&format!(
            "\n_… {elided} more row{} elided (source has {} rows) …_\n",
            if elided == 1 { "" } else { "s" },
            rows.len()
        ));
    }
    out
}

fn render_row(out: &mut String, row: &[String], cols: usize) {
    out.push('|');
    for c in 0..cols {
        let cell = row.get(c).map(String::as_str).unwrap_or("");
        out.push(' ');
        out.push_str(&escape_cell(cell));
        out.push_str(" |");
    }
    out.push('\n');
}

fn render_divider(out: &mut String, cols: usize) {
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');
}

/// Escape the markdown-table-breaking characters in a cell: `|` (column break) and newlines
/// (row break) become spaces/escapes so a cell with embedded structure can't corrupt the grid.
fn escape_cell(cell: &str) -> String {
    cell.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
}
