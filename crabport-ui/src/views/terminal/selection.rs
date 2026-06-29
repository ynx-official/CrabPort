use gpui::{Bounds, Pixels, Point, px};

/// Terminal selection.
///
/// Rows are stored as **alacritty grid absolute line indices** (not viewport
/// rows). This means the selection stays anchored to the text content as the
/// viewport scrolls, instead of sliding with the viewport.
///
/// Conversion: viewport_row = grid_line + display_offset
#[derive(Clone, Debug)]
pub(crate) struct Selection {
    pub(crate) active: bool,
    pub(crate) start_col: usize,
    pub(crate) start_row: i32,
    pub(crate) end_col: usize,
    pub(crate) end_row: i32,
}

impl Selection {
    pub(crate) fn new(col: usize, row: i32) -> Self {
        Self {
            active: true,
            start_col: col,
            start_row: row,
            end_col: col,
            end_row: row,
        }
    }

    /// Returns (start_row, end_row, start_col, end_col) in grid coordinates,
    /// normalized so start <= end.
    pub(crate) fn range(&self) -> (i32, i32, usize, usize) {
        if self.start_row < self.end_row {
            (self.start_row, self.end_row, self.start_col, self.end_col)
        } else if self.start_row > self.end_row {
            (self.end_row, self.start_row, self.end_col, self.start_col)
        } else {
            let (lo, hi) = if self.start_col <= self.end_col {
                (self.start_col, self.end_col)
            } else {
                (self.end_col, self.start_col)
            };
            (self.start_row, self.end_row, lo, hi)
        }
    }
}

/// Convert a mouse position to a **grid absolute line** + viewport column.
///
/// `viewport_row` is the visible row (0 = top of viewport).
/// The grid line is `viewport_row - display_offset` (matching alacritty's
/// `Line(row as i32 - offset as i32)` indexing used in prepaint).
pub(crate) fn mouse_to_grid(
    pos: Point<Pixels>,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    display_offset: i32,
) -> Option<(usize, i32)> {
    let local_x = pos.x - bounds.origin.x;
    let local_y = pos.y - bounds.origin.y;
    if local_x < px(0.0) || local_y < px(0.0) {
        return None;
    }
    let col = ((local_x / cell_width) as f32).floor() as usize;
    let viewport_row = ((local_y / line_height) as f32).floor() as i32;
    // Convert viewport row to grid line.
    let grid_line = viewport_row - display_offset;
    Some((col.min(999), grid_line))
}
