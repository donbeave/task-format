//! `taskfmt selfcheck <task> <workspace> [--base …] [--reference …] [--keep]` — prove the gate of
//! one task package is RED on the untouched baseline and (with a reference) GREEN on the solution.
//! Exit 0 only on `SELFCHECK RESULT PASS`; 1 on FAIL; 66 missing input; 70 internal error.

use std::path::Path;

use crate::gate;
use crate::selfcheck::{self, SelfcheckOpts};
use crate::verifycfg::{FILE_NAME, VerifyConfig};

pub fn run(
    task: &Path,
    workspace: &Path,
    base: Option<String>,
    reference: Option<&Path>,
    keep: bool,
) -> anyhow::Result<i32> {
    let base = match resolve_base(task, base) {
        Ok(base) => base,
        Err(err) => return Ok(report_error(&err)),
    };
    let opts = SelfcheckOpts {
        task_dir: task.to_path_buf(),
        workspace: workspace.to_path_buf(),
        base,
        reference: reference.map(Path::to_path_buf),
        keep,
    };
    match selfcheck::run(opts) {
        Ok(report) => {
            print!("{}", report.render());
            let _ = std::io::Write::flush(&mut std::io::stdout());
            Ok(report.exit_code())
        }
        Err(err) => Ok(report_error(&err)),
    }
}

/// `--base` > `TASKFMT_BASE` > `base_ref` in verify.toml > `baseline` (the verify order). A
/// missing task dir / verify.toml surfaces as a missing input (66), never as a config error.
fn resolve_base(task: &Path, explicit: Option<String>) -> anyhow::Result<String> {
    if let Some(base) = explicit {
        return Ok(base);
    }
    let path = task.join(FILE_NAME);
    let cfg = if path.is_file() {
        VerifyConfig::load(&path).unwrap_or_default()
    } else {
        VerifyConfig::default()
    };
    Ok(gate::resolve_base(&None, &cfg))
}

fn report_error(err: &anyhow::Error) -> i32 {
    let code = selfcheck::exit_code_for(err);
    if code == selfcheck::EXIT_INTERNAL {
        crate::redact::emit(&format!("SELFCHECK RESULT FAIL internal-error {err:#}"));
    } else {
        crate::redact::eemit(&format!("taskfmt selfcheck: {err:#}"));
    }
    code
}
