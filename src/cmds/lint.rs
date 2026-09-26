use std::path::Path;

use crate::lint;

pub fn run(task_dir: &Path, json: bool) -> anyhow::Result<i32> {
    let report = lint::lint_path(task_dir);
    if json {
        println!("{}", report.render_json());
    } else {
        print!("{}", report.render());
    }
    Ok(i32::from(!report.passed()))
}
