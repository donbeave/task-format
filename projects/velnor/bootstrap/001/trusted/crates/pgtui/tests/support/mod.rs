//! Planner-shipped test support (trusted material, never edited by an executor).
//!
//! Helpers grow per task as the application surface grows; the buffer-level
//! helpers below exist from TASK-001 on and never change.

#![allow(dead_code)]

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Snapshot/render terminal width in cells.
pub const COLUMNS: u16 = 100;
/// Snapshot/render terminal height in cells.
pub const ROWS: u16 = 30;

/// A `COLUMNS` x `ROWS` buffer with `lines` written from the top-left corner.
pub fn buffer(lines: &[&str]) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, COLUMNS, ROWS));
    for (y, line) in lines.iter().enumerate() {
        buffer.set_string(0, u16::try_from(y).unwrap_or(ROWS), line, Style::new());
    }
    buffer
}

/// `pgtui::render::buffer_to_svg` on a hand-built buffer.
pub fn svg_of(lines: &[&str]) -> String {
    pgtui::render::buffer_to_svg(&buffer(lines))
}

/// Rasterizes `svg` and returns the PNG pixel dimensions.
pub fn png_dims(svg: &str) -> (u32, u32) {
    let png = pgtui::render::svg_to_png(svg);
    pgtui::render::png_dimensions(&png)
}
