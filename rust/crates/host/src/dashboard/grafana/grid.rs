//! Grid remap — Grafana's **24-col** `gridPos` → our shipped **12-col** grid (viz
//! grafana-dashboard-fidelity scope, slice 1; Open-Q1 RESOLVED 2026-07-23: keep 12-col, fix the mapper,
//! do NOT touch the UI's `gridGeometry.ts`). Copying a 24-col `gridPos` verbatim onto a 12-col board
//! overflows every tile — a `w=8 x=16` third-width panel lands off-grid and stretches full-width with
//! huge gaps (the measured "looks like shit" alignment bug). This pass halves each tile's `x`/`w`,
//! scales its pixel height (Grafana rows are **30 px**, ours **56 px**), and **repacks `y` by band** so
//! tiles pack 3-across with no vertical overlap.
//!
//! Pure geometry over the whole `cells[]` (band repack needs every tile's original `y`), run once after
//! the panel→cell map. Rule 10: no panel type is special-cased — a `row` marker halves like any tile
//! (its full 24 → full 12) and bands with the rest.

use crate::dashboard::model::Cell;

/// Grafana's fixed grid width (the `gridPos` domain).
pub const GRAFANA_COLS: u32 = 24;
/// Our shipped grid width (`gridGeometry.ts` `GRID_COLS`, unchanged by decision).
pub const OUR_COLS: u32 = 12;
/// Grafana grid-row pixel height.
pub const GRAFANA_ROW_PX: u32 = 30;
/// Our grid-row pixel height (`gridGeometry.ts` `GRID_ROW_H`).
pub const OUR_ROW_PX: u32 = 56;

/// Remap every cell's geometry from Grafana's 24-col grid to our 12-col grid, in place. Idempotent per
/// import (called once on the raw-copied `gridPos` before save).
pub fn remap_cells(cells: &mut [Cell]) {
    // Capture original y (the band key) before we overwrite x/w/h — then y.
    let orig_ys: Vec<u32> = cells.iter().map(|c| c.y).collect();

    for c in cells.iter_mut() {
        let (x, w) = remap_x_w(c.x, c.w);
        c.x = x;
        c.w = w;
        c.h = remap_h(c.h);
    }

    // Repack y by band: every distinct original y becomes a stacked row whose height is the tallest
    // (already-scaled) tile that started in it — so tiles that shared a Grafana row stay top-aligned and
    // no two bands overlap.
    let mut bands: Vec<u32> = orig_ys.clone();
    bands.sort_unstable();
    bands.dedup();
    let mut cursor = 0u32;
    for band in bands {
        let band_h = cells
            .iter()
            .zip(&orig_ys)
            .filter(|(_, y)| **y == band)
            .map(|(c, _)| c.h)
            .max()
            .unwrap_or(1);
        for (c, y) in cells.iter_mut().zip(&orig_ys) {
            if *y == band {
                c.y = cursor;
            }
        }
        cursor += band_h.max(1);
    }
}

/// Halve `x`/`w` from 24-col to 12-col: `x` floors (left edges stay aligned), `w` rounds up to keep a
/// tile at least as wide, and the pair is clamped inside the 12-col width (min width 1).
fn remap_x_w(x: u32, w: u32) -> (u32, u32) {
    let scale = GRAFANA_COLS / OUR_COLS; // 2
    let mut nx = x / scale;
    if nx >= OUR_COLS {
        nx = OUR_COLS - 1;
    }
    // round-up halve so a 1-wide Grafana tile doesn't collapse to 0
    let mut nw = w.div_ceil(scale).max(1);
    if nx + nw > OUR_COLS {
        nw = OUR_COLS - nx;
    }
    (nx, nw.max(1))
}

/// Scale a tile's height so it occupies the same pixels on our taller rows: Grafana `h` rows × 30 px ÷
/// our 56 px, rounded, min 1.
fn remap_h(h: u32) -> u32 {
    ((h * GRAFANA_ROW_PX + OUR_ROW_PX / 2) / OUR_ROW_PX).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(i: &str, x: u32, y: u32, w: u32, h: u32) -> Cell {
        Cell {
            i: i.into(),
            x,
            y,
            w,
            h,
            ..Cell::default()
        }
    }

    #[test]
    fn three_grafana_thirds_pack_three_across_in_twelve_cols() {
        // Grafana: three w=8 tiles at x=0/8/16 on one row.
        let mut cells = [
            cell("a", 0, 0, 8, 4),
            cell("b", 8, 0, 8, 4),
            cell("c", 16, 0, 8, 4),
        ];
        remap_cells(&mut cells);
        let xs: Vec<u32> = cells.iter().map(|c| c.x).collect();
        assert_eq!(xs, vec![0, 4, 8]); // 3 distinct columns across 12
        for c in &cells {
            assert!(c.x + c.w <= OUR_COLS, "cell {} overflows width", c.i);
        }
    }

    #[test]
    fn full_width_and_rows_stay_full_width() {
        let mut cells = [cell("row", 0, 0, 24, 1), cell("wide", 0, 1, 24, 8)];
        remap_cells(&mut cells);
        assert_eq!((cells[0].x, cells[0].w), (0, 12));
        assert_eq!((cells[1].x, cells[1].w), (0, 12));
    }

    #[test]
    fn bands_do_not_overlap_vertically() {
        // Two Grafana rows (y=0 and y=8) each with a tile; after repack the second starts at the first's
        // scaled height, never overlapping.
        let mut cells = [cell("top", 0, 0, 12, 8), cell("bot", 0, 8, 12, 8)];
        remap_cells(&mut cells);
        let top = &cells[0];
        let bot = &cells[1];
        assert_eq!(top.y, 0);
        assert!(
            bot.y >= top.y + top.h,
            "band overlap: {} vs {}",
            top.y,
            bot.y
        );
    }

    #[test]
    fn height_scales_from_30px_rows_to_56px_rows() {
        // 8 Grafana rows = 240px → round(240/56)=4 of our rows.
        assert_eq!(remap_h(8), 4);
        // a 1-row Grafana panel never collapses to 0
        assert_eq!(remap_h(1), 1);
    }

    #[test]
    fn no_cell_exceeds_grid_width_for_odd_geometry() {
        // Odd widths / far-right origin must still land inside 12 cols.
        let mut cells = [cell("odd", 23, 0, 3, 2), cell("mid", 9, 0, 7, 2)];
        remap_cells(&mut cells);
        for c in &cells {
            assert!(c.x < OUR_COLS);
            assert!(
                c.x + c.w <= OUR_COLS,
                "{} overflows: x={} w={}",
                c.i,
                c.x,
                c.w
            );
        }
    }
}
