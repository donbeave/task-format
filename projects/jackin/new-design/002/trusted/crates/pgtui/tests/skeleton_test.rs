//! Trusted TASK-001 gate: the workspace scaffold builds, both binaries are the
//! specified stubs, and the trusted render pipeline rasterizes.

mod support;

use ratatui::style::{Color, Modifier, Style};
use std::process::Command;

#[test]
fn pgtui_stub_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_pgtui"))
        .output()
        .expect("pgtui runs");
    assert_eq!(output.status.code(), Some(2), "status: {:?}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error: not implemented"),
        "stderr: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn gallery_stub_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_gallery"))
        .output()
        .expect("gallery runs");
    assert_eq!(output.status.code(), Some(2), "status: {:?}", output.status);
    assert!(
        !output.stderr.is_empty(),
        "gallery stub must explain itself on stderr"
    );
}

#[test]
fn buffer_to_text_trims_and_keeps_rows() {
    let mut buffer = support::buffer(&["hello pgtui"]);
    buffer.set_string(0, 1, "second row", Style::new());

    let text = pgtui::render::buffer_to_text(&buffer);
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(lines.len(), usize::from(support::ROWS), "{text}");
    assert_eq!(lines[0], "hello pgtui");
    assert_eq!(lines[1], "second row");
    assert!(lines[2].is_empty(), "row 2 must be blank: {:?}", lines[2]);
}

#[test]
fn svg_and_png_pipeline_is_deterministic() {
    let mut buffer = support::buffer(&["hello pgtui"]);
    buffer.set_string(
        0,
        1,
        "styled",
        Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
    );

    let svg = pgtui::render::buffer_to_svg(&buffer);
    let again = pgtui::render::buffer_to_svg(&buffer);
    assert_eq!(
        svg, again,
        "two runs over one buffer must be byte-identical"
    );
    assert!(svg.starts_with("<svg "), "{svg}");
    assert!(svg.contains(pgtui::render::FONT_FAMILY), "{svg}");
    assert!(svg.contains("hello pgtui"), "{svg}");
    assert!(svg.contains("fill=\"#cd3131\""), "{svg}");
    assert!(svg.contains("font-weight=\"bold\""), "{svg}");

    let png = pgtui::render::svg_to_png(&svg);
    assert!(png.len() > 1024, "png too small: {} bytes", png.len());
    assert_eq!(pgtui::render::png_dimensions(&png), (900, 540));
    assert_eq!(
        support::png_dims(&support::svg_of(&["hello pgtui"])),
        (900, 540)
    );
}
