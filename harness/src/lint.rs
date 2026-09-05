//! Strict task/v5 + verify/v2 package lint. Markdown states behavior; TOML states execution.
use crate::acceptance;
use crate::taskfile::{self, TaskFile};
use crate::verifycfg::{self, VerifyConfig};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
pub const SCHEMA: &str = "task/v5";
type LeafRefs = (String, BTreeSet<String>, BTreeSet<String>, BTreeSet<String>);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}
#[derive(Debug, Clone)]
pub struct LintReport {
    pub target: PathBuf,
    pub findings: Vec<Finding>,
}
impl LintReport {
    pub fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|x| x.severity == Severity::Error)
            .count()
    }
    pub fn warnings(&self) -> usize {
        self.findings
            .iter()
            .filter(|x| x.severity == Severity::Warn)
            .count()
    }
    pub fn passed(&self) -> bool {
        self.errors() == 0
    }
    pub fn render(&self) -> String {
        let mut s = String::new();
        for x in &self.findings {
            s.push_str(&format!(
                "{} {}: {}\n",
                if x.severity == Severity::Error {
                    "ERROR"
                } else {
                    "WARN "
                },
                x.rule,
                x.message
            ));
        }
        s.push_str(&format!(
            "SUMMARY errors={} warnings={}\nLINT {}\n",
            self.errors(),
            self.warnings(),
            if self.passed() { "PASS" } else { "FAIL" }
        ));
        s
    }
}
fn fail(out: &mut Vec<Finding>, rule: &'static str, msg: impl Into<String>) {
    out.push(Finding {
        severity: Severity::Error,
        rule,
        message: msg.into(),
    })
}
pub fn lint_path(target: &Path) -> LintReport {
    let dir = if target.is_dir() {
        target.to_path_buf()
    } else {
        target.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let readme = dir.join("README.md");
    let text = match std::fs::read_to_string(&readme) {
        Ok(x) => x,
        Err(e) => {
            return LintReport {
                target: readme,
                findings: vec![Finding {
                    severity: Severity::Error,
                    rule: "readme",
                    message: format!("cannot read: {e}"),
                }],
            };
        }
    };
    LintReport {
        target: readme.clone(),
        findings: lint(&text, &readme, Some(&dir)),
    }
}
/// In-memory entry point. It validates Markdown; file-backed lint additionally validates TOML.
pub fn lint_text(text: &str, readme: &Path) -> Vec<Finding> {
    lint(text, readme, readme.parent())
}
fn lint(text: &str, readme: &Path, dir: Option<&Path>) -> Vec<Finding> {
    let mut out = Vec::new();
    let tf = match TaskFile::parse(text.into(), readme) {
        Ok(x) => x,
        Err(e) => {
            fail(&mut out, "frontmatter", format!("{e:#}"));
            return out;
        }
    };
    frontmatter(&mut out, &tf);
    sections(&mut out, &tf);
    let req = requirements(&mut out, &tf);
    let ac = acceptance(&mut out, &tf);
    let leaves = checklist(&mut out, &tf);
    if let Some(dir) = dir {
        match VerifyConfig::load(&dir.join(verifycfg::FILE_NAME)) {
            Ok(cfg) => graph(&mut out, &tf, &req, &ac, &leaves, &cfg),
            Err(e) => fail(
                &mut out,
                "config",
                format!("{} invalid: {e:#}", verifycfg::FILE_NAME),
            ),
        }
    };
    out
}
fn frontmatter(out: &mut Vec<Finding>, tf: &TaskFile) {
    if tf.frontmatter.schema != SCHEMA {
        fail(
            out,
            "frontmatter",
            format!("schema is {:?}, want {SCHEMA}", tf.frontmatter.schema),
        )
    }
    if !Regex::new(r"^TASK-[0-9]+$")
        .unwrap()
        .is_match(&tf.frontmatter.id)
    {
        fail(out, "frontmatter", "id must be TASK-<digits>")
    }
    if tf.frontmatter.title.is_empty() {
        fail(out, "frontmatter", "title missing")
    }
    if !matches!(
        tf.frontmatter.kind.as_str(),
        "bugfix" | "feature" | "refactor" | "removal" | "migration" | "test" | "docs"
    ) {
        fail(
            out,
            "frontmatter",
            format!("invalid kind {:?}", tf.frontmatter.kind),
        )
    }
}
fn sections(out: &mut Vec<Finding>, tf: &TaskFile) {
    let want = [
        "Goal",
        "Context",
        "Preconditions",
        "Scope",
        "Requirements",
        "Acceptance criteria",
        "Fixed decisions",
        "Checklist",
    ];
    let got: Vec<_> = tf.sections.iter().map(|x| x.0.as_str()).collect();
    for s in want {
        if !got.contains(&s) {
            fail(out, "sections", format!("missing H2: {s}"))
        }
    }
    if tf
        .h1
        .as_deref()
        .is_none_or(|h| !h.starts_with(&format!("{} ", tf.frontmatter.id)))
    {
        fail(
            out,
            "heading",
            format!("H1 must start with {}", tf.frontmatter.id),
        )
    }
}
fn definitions(body: Option<&Vec<String>>, prefix: &str) -> Vec<String> {
    let re = Regex::new(&format!(r"^- \*\*({prefix}-[0-9]+)(?:[^*]*)\*\*")).unwrap();
    body.into_iter()
        .flatten()
        .filter_map(|s| re.captures(s).map(|c| c[1].into()))
        .collect()
}
fn requirements(out: &mut Vec<Finding>, tf: &TaskFile) -> BTreeSet<String> {
    let ids = definitions(tf.section("Requirements"), "R");
    let mut set = BTreeSet::new();
    for x in ids {
        if !set.insert(x.clone()) {
            fail(out, "requirements", format!("duplicate {x}"))
        }
    }
    if set.is_empty() {
        fail(out, "requirements", "no R-NNN entries")
    };
    set
}
fn acceptance(
    out: &mut Vec<Finding>,
    tf: &TaskFile,
) -> BTreeMap<String, (BTreeSet<String>, String)> {
    let legacy = Regex::new(r"(?m)^\s*\|.*\bAC-[0-9]+").unwrap().is_match(
        &tf.section("Acceptance criteria")
            .map(|x| x.join("\n"))
            .unwrap_or_default(),
    );
    if legacy {
        fail(out, "acceptance", "legacy table acceptance is forbidden")
    }
    if !tf.typed_acceptance.detected {
        fail(out, "acceptance", "no canonical AC-NNN blocks")
    }
    for e in acceptance::validate_shape(&tf.typed_acceptance) {
        fail(out, "acceptance", format!("line {}: {}", e.line, e.message))
    }
    let mut map = BTreeMap::new();
    for x in &tf.typed_acceptance.criteria {
        if map
            .insert(
                x.id.clone(),
                (x.covers.iter().cloned().collect(), x.evidence.clone()),
            )
            .is_some()
        {
            fail(out, "acceptance", format!("duplicate {}", x.id))
        }
    }
    map
}
fn refs(prefix: &str, s: &str) -> BTreeSet<String> {
    Regex::new(&format!(r"\b{prefix}-[0-9]+\b"))
        .unwrap()
        .find_iter(s)
        .map(|m| m.as_str().into())
        .collect()
}
fn checklist(out: &mut Vec<Finding>, tf: &TaskFile) -> Vec<LeafRefs> {
    let starts = tf.text.matches(taskfile::CHECKLIST_START).count();
    let ends = tf.text.matches(taskfile::CHECKLIST_END).count();
    if starts != 1 || ends != 1 {
        fail(
            out,
            "checklist",
            format!("need one checklist marker pair, found start={starts} end={ends}"),
        );
        return vec![];
    }
    let items = taskfile::parse_checklist(&tf.checklist);
    if items.is_empty() {
        fail(out, "checklist", "empty checklist");
        return vec![];
    }
    for x in &items {
        if !x.well_formed {
            fail(out, "checklist", format!("malformed item: {}", x.raw))
        }
    }
    let flags = taskfile::leaf_flags(&items);
    let mut ids = BTreeSet::new();
    let mut leaves = Vec::new();
    for (x, leaf) in items.iter().zip(flags) {
        if !x.well_formed {
            continue;
        }
        if !ids.insert(x.id.clone()) {
            fail(out, "checklist", format!("duplicate item {}", x.id))
        }
        if leaf {
            let r = refs("R", &x.text);
            let a = refs("AC", &x.text);
            let c = refs("CHK", &x.text);
            if r.is_empty() || a.is_empty() || c.is_empty() {
                fail(
                    out,
                    "checklist",
                    format!("leaf {} must reference R-*, AC-*, and CHK-*", x.id),
                )
            }
            leaves.push((x.id.clone(), r, a, c));
        }
    }
    leaves
}
fn graph(
    out: &mut Vec<Finding>,
    tf: &TaskFile,
    req: &BTreeSet<String>,
    ac: &BTreeMap<String, (BTreeSet<String>, String)>,
    leaves: &[LeafRefs],
    cfg: &VerifyConfig,
) {
    if cfg.task_id != tf.frontmatter.id {
        fail(
            out,
            "config",
            format!(
                "task_id {} does not match README {}",
                cfg.task_id, tf.frontmatter.id
            ),
        );
    }
    let checks: BTreeMap<_, _> = cfg.checks.iter().map(|x| (x.id.as_str(), x)).collect();
    let (mut ru, mut au, mut cu) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for (id, (covers, chk)) in ac {
        for r in covers {
            if !req.contains(r) {
                fail(
                    out,
                    "graph",
                    format!("{id} references unknown requirement {r}"),
                );
            } else {
                ru.insert(r.clone());
            }
        }
        if !checks.contains_key(chk.as_str()) {
            fail(out, "graph", format!("{id} references unknown check {chk}"));
        } else {
            cu.insert(chk.clone());
        }
    }
    for c in &cfg.checks {
        if c.requirements.is_empty() {
            fail(out, "graph", format!("{} has no requirements", c.id));
        }
        if c.acceptance.is_empty() {
            fail(out, "graph", format!("{} has no acceptance criteria", c.id));
        }
        for r in &c.requirements {
            if !req.contains(r) {
                fail(
                    out,
                    "graph",
                    format!("{} references unknown requirement {r}", c.id),
                );
            } else {
                ru.insert(r.clone());
            }
        }
        for a in &c.acceptance {
            match ac.get(a) {
                None => fail(
                    out,
                    "graph",
                    format!("{} references unknown acceptance {a}", c.id),
                ),
                Some((_, linked)) if linked != &c.id => fail(
                    out,
                    "graph",
                    format!("{a} Check {linked} disagrees with verifier {}", c.id),
                ),
                Some(_) => {
                    au.insert(a.clone());
                    cu.insert(c.id.clone());
                }
            }
        }
    }
    for (leaf, rs, as_, cs) in leaves {
        for r in rs {
            if !req.contains(r) {
                fail(
                    out,
                    "graph",
                    format!("leaf {leaf} references unknown requirement {r}"),
                );
            } else {
                ru.insert(r.clone());
            }
        }
        for a in as_ {
            if !ac.contains_key(a) {
                fail(
                    out,
                    "graph",
                    format!("leaf {leaf} references unknown acceptance {a}"),
                );
            } else {
                au.insert(a.clone());
            }
        }
        for c in cs {
            if !checks.contains_key(c.as_str()) {
                fail(
                    out,
                    "graph",
                    format!("leaf {leaf} references unknown check {c}"),
                );
            } else {
                cu.insert(c.clone());
            }
        }
    }
    for r in req {
        if !ru.contains(r) {
            fail(out, "graph", format!("unused/uncovered requirement {r}"));
        }
    }
    for a in ac.keys() {
        if !au.contains(a) {
            fail(out, "graph", format!("unused/uncovered acceptance {a}"));
        }
    }
    for c in checks.keys() {
        if !cu.contains(*c) {
            fail(out, "graph", format!("unused/uncovered check {c}"));
        }
    }
}
