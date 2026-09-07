//! Trusted TASK-007 gate: the `gallery` binary contract (D-071, D-080).

mod support;

use std::fs;
use std::process::Command;

/// The ten D-071 screens, in D-080 order.
const SCREENS: [&str; 10] = [
    "screen__connection_list_empty",
    "screen__connection_list_two",
    "screen__create_form_blank",
    "screen__create_form_filled",
    "screen__browser_sidebar_empty_body",
    "screen__preview_unsorted",
    "screen__preview_sorted_asc",
    "screen__preview_sorted_desc",
    "screen__custom_sql_empty",
    "screen__custom_sql_results",
];

fn run_gallery(out: &std::path::Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_gallery"))
        .arg("--out")
        .arg(out)
        .output()
        .expect("gallery runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn gallery_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("trusted support: tempdir");
    let out = dir.path().join("screens");
    (dir, out)
}

fn svg_of(out: &std::path::Path, name: &str) -> String {
    fs::read_to_string(out.join(format!("{name}.svg"))).expect("svg file")
}

#[test]
fn writes_ten_svgs_and_pngs() {
    let (_dir, out) = gallery_dir();
    run_gallery(&out);

    let entries: Vec<String> = fs::read_dir(&out)
        .expect("output directory is created")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(entries.len(), 20, "10 svg + 10 png: {entries:?}");

    for name in SCREENS {
        let svg = svg_of(&out, name);
        assert!(svg.starts_with("<svg "), "{name}: {svg}");
        assert!(svg.contains(pgtui::render::FONT_FAMILY), "{name}");

        let png = fs::read(out.join(format!("{name}.png"))).expect("png file");
        assert_eq!(
            pgtui::render::png_dimensions(&png),
            (900, 540),
            "{name}: 100x30 cells at 9x18 px"
        );
    }
}

#[test]
fn screen_content_matches_the_named_state() {
    let (_dir, out) = gallery_dir();
    run_gallery(&out);

    assert!(
        svg_of(&out, "screen__connection_list_empty")
            .contains("No saved connections. Press n to create one."),
        "empty list state"
    );
    assert!(
        svg_of(&out, "screen__connection_list_two").contains("beta"),
        "two-connection state renders the second row"
    );
    assert!(
        // The SVG writer XML-escapes cell text (D-080), so the focus marker reaches this artifact
        // as its escaped form; the raw form is only ever seen in the text render.
        svg_of(&out, "screen__create_form_blank").contains("&gt; Name:"),
        "blank form state"
    );
    assert!(
        svg_of(&out, "screen__create_form_filled").contains("******"),
        "filled form masks the password"
    );
    assert!(
        svg_of(&out, "screen__browser_sidebar_empty_body").contains(" Tables (4)* "),
        "browser sidebar state"
    );
    assert!(
        svg_of(&out, "screen__preview_sorted_asc").contains("[balance ^]"),
        "sorted preview state"
    );
    assert!(
        svg_of(&out, "screen__custom_sql_results").contains(" Results "),
        "custom SQL result state"
    );
}

#[test]
fn output_is_deterministic() {
    let (_first, first_out) = gallery_dir();
    let (_second, second_out) = gallery_dir();
    run_gallery(&first_out);
    run_gallery(&second_out);

    for name in SCREENS {
        let left = fs::read(first_out.join(format!("{name}.svg"))).expect("svg");
        let right = fs::read(second_out.join(format!("{name}.svg"))).expect("svg");
        assert_eq!(left, right, "{name}: SVG bytes differ between runs");

        let left = fs::read(first_out.join(format!("{name}.png"))).expect("png");
        let right = fs::read(second_out.join(format!("{name}.png"))).expect("png");
        assert_eq!(left, right, "{name}: PNG bytes differ between runs");
    }
}

#[test]
fn readme_lists_all_ten_screens() {
    // The test runner's working directory is the package root (`CARGO_MANIFEST_DIR`), not the
    // repository root that `verify.toml` declares for the gate; anchor the path to the manifest
    // directory, which the build system guarantees, instead of to the process CWD.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let readme =
        fs::read_to_string(repo_root.join("README.md")).expect("README.md at the repository root");
    assert!(
        readme.contains("## Screens"),
        "README screen section: {readme}"
    );

    for name in SCREENS {
        assert!(
            readme.contains(&format!("docs/screens/{name}.png")),
            "{name} missing from README.md"
        );
    }
}
