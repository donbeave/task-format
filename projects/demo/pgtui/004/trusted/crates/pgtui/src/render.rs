//! Trusted render utilities: `ratatui::Buffer` -> text / SVG / PNG.
//!
//! Planner-shipped verification material. Only buffer-level functions are
//! exposed: nothing here mentions an application type, so the module compiles
//! from TASK-001 on. This file is part of the trusted base commit and is never
//! edited by an executor.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

/// Cell width in px. A 100x30 cell buffer rasterizes to 900x540 px.
pub const CELL_WIDTH: u32 = 9;
/// Cell height in px.
pub const CELL_HEIGHT: u32 = 18;
/// Font size in px; DejaVu Sans Mono advance is ~= `CELL_WIDTH` at this size.
pub const FONT_SIZE: f64 = 15.0;
/// Font family the SVG names and `svg_to_png` loads (never system fonts).
pub const FONT_FAMILY: &str = "DejaVu Sans Mono";

const FONT_BYTES: &[u8] = include_bytes!("fonts/DejaVuSansMono.ttf");
const DEFAULT_BG: &str = "#1e1e2e";
const DEFAULT_FG: &str = "#cdd6f4";
/// Distance from the top of a cell row to the text baseline.
const BASELINE_IN_CELL: f64 = 14.0;

/// Renders the buffer as plain text: one line per row, trailing spaces of every
/// row trimmed.
pub fn buffer_to_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut rows = Vec::with_capacity(area.height as usize);
    for y in 0..area.height {
        let mut row = String::with_capacity(area.width as usize);
        for x in 0..area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        rows.push(row.trim_end().to_string());
    }
    rows.join("\n")
}

/// Renders the buffer as a deterministic SVG document: fixed cell metrics, the
/// bundled font family, no timestamps, no random data. Two runs over the same
/// buffer produce byte-identical output.
pub fn buffer_to_svg(buffer: &Buffer) -> String {
    let area = buffer.area;
    let width = CELL_WIDTH * u32::from(area.width);
    let height = CELL_HEIGHT * u32::from(area.height);
    let mut svg = String::with_capacity(64 * 1024);
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" \
         viewBox=\"0 0 {width} {height}\" font-family=\"{FONT_FAMILY}\" font-size=\"{FONT_SIZE}\">\n"
    ));
    svg.push_str(&format!(
        "<rect width=\"{width}\" height=\"{height}\" fill=\"{DEFAULT_BG}\"/>\n"
    ));
    for y in 0..area.height {
        let mut x = 0;
        while x < area.width {
            let start = x;
            let (cells, fg, bg, mods) = styled_run(buffer, start, y);
            x += cells.len() as u16;
            let run_width = u32::try_from(cells.len()).unwrap_or(0) * CELL_WIDTH;
            let px = u32::from(start) * CELL_WIDTH;
            let py = u32::from(y);
            if bg != DEFAULT_BG {
                svg.push_str(&format!(
                    "<rect x=\"{px}\" y=\"{}\" width=\"{run_width}\" height=\"{CELL_HEIGHT}\" fill=\"{bg}\"/>\n",
                    py * CELL_HEIGHT,
                ));
            }
            let text: String = cells.concat();
            if text.trim().is_empty() {
                continue;
            }
            let mut attrs = String::new();
            if mods.contains(Modifier::BOLD) {
                attrs.push_str(" font-weight=\"bold\"");
            }
            if mods.contains(Modifier::ITALIC) {
                attrs.push_str(" font-style=\"italic\"");
            }
            if mods.contains(Modifier::UNDERLINED) {
                attrs.push_str(" text-decoration=\"underline\"");
            }
            svg.push_str(&format!(
                "<text x=\"{px}\" y=\"{}\" fill=\"{fg}\"{attrs} xml:space=\"preserve\" \
                 textLength=\"{run_width}\">{}</text>\n",
                py as f64 * f64::from(CELL_HEIGHT) + BASELINE_IN_CELL,
                escape_xml(&text),
            ));
        }
    }
    svg.push_str("</svg>\n");
    svg
}

/// Rasterizes `svg` to PNG bytes with the bundled font (no system font lookup).
/// Cell geometry is preserved: a 100x30 buffer yields a 900x540 image.
pub fn svg_to_png(svg: &str) -> Vec<u8> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_font_data(FONT_BYTES.to_vec());
    options.font_family = FONT_FAMILY.to_string();
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .expect("trusted render: svg_to_png given an invalid SVG document");
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .expect("trusted render: pixmap allocation failed");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .expect("trusted render: PNG encoding failed")
}

/// PNG pixel dimensions read from the IHDR chunk, `(width, height)`.
pub fn png_dimensions(png: &[u8]) -> (u32, u32) {
    const IHDR_WIDTH_OFFSET: usize = 16;
    let width = u32::from_be_bytes(
        png[IHDR_WIDTH_OFFSET..IHDR_WIDTH_OFFSET + 4]
            .try_into()
            .expect("trusted render: PNG shorter than the IHDR chunk"),
    );
    let height = u32::from_be_bytes(
        png[IHDR_WIDTH_OFFSET + 4..IHDR_WIDTH_OFFSET + 8]
            .try_into()
            .expect("trusted render: PNG shorter than the IHDR chunk"),
    );
    (width, height)
}

/// Cells of one run: consecutive cells in a row sharing fg/bg/modifier.
fn styled_run(buffer: &Buffer, start_x: u16, y: u16) -> (Vec<&str>, String, String, Modifier) {
    let area = buffer.area;
    let first = &buffer[(start_x, y)];
    let (fg, bg, mods) = (first.fg, first.bg, first.modifier);
    let mut cells = Vec::new();
    let mut x = start_x;
    while x < area.width {
        let cell = &buffer[(x, y)];
        if cell.fg != fg || cell.bg != bg || cell.modifier != mods {
            break;
        }
        cells.push(cell.symbol());
        x += 1;
    }
    let fg_hex = color_to_hex(fg, DEFAULT_FG);
    let bg_hex = color_to_hex(bg, DEFAULT_BG);
    (cells, fg_hex, bg_hex, mods)
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Fixed ANSI palette + xterm 256 extension + direct RGB. `Reset` maps to the
/// theme default, never to "no color", so output is identical everywhere.
fn color_to_hex(color: Color, default: &str) -> String {
    match color {
        Color::Reset => default.to_string(),
        Color::Black => "#000000".to_string(),
        Color::Red => "#cd3131".to_string(),
        Color::Green => "#0dbc79".to_string(),
        Color::Yellow => "#e5e510".to_string(),
        Color::Blue => "#2472c8".to_string(),
        Color::Magenta => "#bc3fbc".to_string(),
        Color::Cyan => "#11a8cd".to_string(),
        Color::Gray => "#e5e5e5".to_string(),
        Color::DarkGray => "#666666".to_string(),
        Color::LightRed => "#f14c4c".to_string(),
        Color::LightGreen => "#23d18b".to_string(),
        Color::LightYellow => "#f5f543".to_string(),
        Color::LightBlue => "#3b8eea".to_string(),
        Color::LightMagenta => "#d670d6".to_string(),
        Color::LightCyan => "#29b8db".to_string(),
        Color::White => "#ffffff".to_string(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(index) => indexed_to_hex(index, default),
    }
}

fn indexed_to_hex(index: u8, default: &str) -> String {
    match index {
        0..=15 => color_to_hex(ansi16(index), default),
        16..=231 => {
            let i = u32::from(index) - 16;
            let levels = [0_u8, 95, 135, 175, 215, 255];
            let (r, rest) = (i / 36, i % 36);
            let (g, b) = (rest / 6, rest % 6);
            format!(
                "#{:02x}{:02x}{:02x}",
                levels[r as usize], levels[g as usize], levels[b as usize]
            )
        }
        232..=255 => {
            let level = 8 + 10 * (u32::from(index) - 232);
            format!("#{level:02x}{level:02x}{level:02x}")
        }
    }
}

fn ansi16(index: u8) -> Color {
    match index {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        _ => Color::White,
    }
}
