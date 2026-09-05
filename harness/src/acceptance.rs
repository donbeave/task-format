//! Canonical acceptance blocks. README owns behavior; `verify.toml` owns execution.
use regex::Regex;
use std::collections::BTreeSet;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceType {
    Scenario,
    Outline,
    Invariant,
    Gate,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub keyword: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub title: String,
    pub kind: AcceptanceType,
    pub covers: Vec<String>,
    /// Stable verifier check ID (name retained to avoid TaskFile API churn).
    pub evidence: String,
    pub steps: Vec<Step>,
    pub examples: Vec<Vec<String>>,
    pub example_headers: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}
#[derive(Debug, Clone, Default)]
pub struct AcceptanceDocument {
    pub criteria: Vec<AcceptanceCriterion>,
    pub errors: Vec<ParseError>,
    pub detected: bool,
}
fn bad(d: &mut AcceptanceDocument, line: usize, msg: impl Into<String>) {
    d.errors.push(ParseError {
        line,
        message: msg.into(),
    })
}
fn val(s: &str) -> String {
    s.trim().trim_matches('`').trim().into()
}
fn refs(s: &str) -> Vec<String> {
    Regex::new(r"R-[0-9]+")
        .unwrap()
        .find_iter(s)
        .map(|m| m.as_str().into())
        .collect()
}
/// Parse canonical H3 blocks. Legacy tables and fenced shell evidence are deliberately rejected.
pub fn parse(text: &str) -> AcceptanceDocument {
    let h = Regex::new(r"^###\s+(AC-[0-9]+)(?:\s+[—-]\s*)?(.*)\s*$").unwrap();
    let lines: Vec<_> = text.lines().collect();
    let mut d = AcceptanceDocument::default();
    let mut i = 0;
    while i < lines.len() {
        let Some(c) = h.captures(lines[i].trim_end()) else {
            i += 1;
            continue;
        };
        d.detected = true;
        let start = i + 1;
        let id = c[1].into();
        let title = c[2].trim().into();
        i += 1;
        let end = (i..lines.len())
            .find(|&n| lines[n].starts_with("### ") || lines[n].starts_with("## "))
            .unwrap_or(lines.len());
        let (mut typ, mut covers, mut check) = (None, None, None);
        let (mut verification, mut gherkin) = (false, false);
        let (mut steps, mut headers, mut examples) = (Vec::new(), Vec::new(), Vec::new());
        while i < end {
            let line = lines[i].trim();
            if line.is_empty() {
                i += 1;
                continue;
            }
            if line == "```gherkin" {
                if gherkin {
                    bad(&mut d, i + 1, format!("{id} duplicate Gherkin body"))
                };
                gherkin = true;
                i += 1;
                continue;
            }
            if line == "```" && gherkin {
                gherkin = false;
                i += 1;
                continue;
            }
            if gherkin {
                if line == "Examples:" {
                    i += 1;
                    continue;
                }
                if line.starts_with('|') && line.ends_with('|') {
                    let row = line[1..line.len() - 1].split('|').map(val).collect();
                    if headers.is_empty() {
                        headers = row
                    } else {
                        examples.push(row)
                    };
                    i += 1;
                    continue;
                }
                let mut p = line.splitn(2, char::is_whitespace);
                let k = p.next().unwrap_or("");
                let body = p.next().unwrap_or("").trim();
                steps.push(Step {
                    keyword: if ["Given", "When", "Then", "And", "But"].contains(&k) {
                        k.into()
                    } else {
                        "Statement".into()
                    },
                    text: if body.is_empty() {
                        line.into()
                    } else {
                        body.into()
                    },
                });
                i += 1;
                continue;
            }
            if line == "**Verification**" {
                verification = true;
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("- **") {
                let Some((key, raw)) = rest.split_once(":**") else {
                    bad(&mut d, i + 1, format!("{id} malformed metadata bullet"));
                    i += 1;
                    continue;
                };
                if !verification {
                    bad(
                        &mut d,
                        i + 1,
                        format!("{id} metadata must follow **Verification**"),
                    )
                };
                match key.trim().to_ascii_lowercase().as_str() {
                    "type" if typ.is_none() => typ = Some(val(raw)),
                    "covers" if covers.is_none() => covers = Some(refs(&val(raw))),
                    "check" if check.is_none() => check = Some(val(raw)),
                    "type" | "covers" | "check" => {
                        bad(&mut d, i + 1, format!("{id} duplicate {key} metadata"))
                    }
                    "expected" | "evidence" => bad(
                        &mut d,
                        i + 1,
                        format!("{id} legacy machine metadata is forbidden"),
                    ),
                    _ => bad(&mut d, i + 1, format!("{id} unexpected metadata")),
                };
                i += 1;
                continue;
            }
            bad(&mut d, i + 1, format!("{id} unexpected acceptance content"));
            i += 1
        }
        let kind = match typ.as_deref() {
            Some("scenario") => AcceptanceType::Scenario,
            Some("outline") => AcceptanceType::Outline,
            Some("invariant") => AcceptanceType::Invariant,
            Some("gate") => AcceptanceType::Gate,
            _ => {
                bad(
                    &mut d,
                    start,
                    format!("{id} Type must be scenario, outline, invariant, or gate"),
                );
                AcceptanceType::Scenario
            }
        };
        let covers = covers.unwrap_or_default();
        let evidence = check.unwrap_or_default();
        if !verification {
            bad(
                &mut d,
                start,
                format!("{id} missing **Verification** section"),
            )
        }
        if evidence.is_empty() {
            bad(&mut d, start, format!("{id} missing Check metadata"))
        }
        if !matches!(kind, AcceptanceType::Gate) && covers.is_empty() {
            bad(&mut d, start, format!("{id} missing Covers metadata"))
        }
        if matches!(kind, AcceptanceType::Gate) && !covers.is_empty() {
            bad(
                &mut d,
                start,
                format!("{id} gate must not have Covers metadata"),
            )
        }
        if !matches!(kind, AcceptanceType::Gate) && steps.is_empty() {
            bad(&mut d, start, format!("{id} missing gherkin body"))
        }
        if matches!(kind, AcceptanceType::Gate) && !steps.is_empty() {
            bad(
                &mut d,
                start,
                format!("{id} gate must not have Gherkin body"),
            )
        }
        d.criteria.push(AcceptanceCriterion {
            id,
            title,
            kind,
            covers,
            evidence,
            steps,
            examples,
            example_headers: headers,
        });
    }
    d
}
pub fn validate_shape(d: &AcceptanceDocument) -> Vec<ParseError> {
    let mut out = d.errors.clone();
    let re = Regex::new(r"^CHK-[0-9]+$").unwrap();
    for ac in &d.criteria {
        if ac.title.is_empty() {
            out.push(ParseError {
                line: 0,
                message: format!("{} title is empty", ac.id),
            })
        }
        if !re.is_match(&ac.evidence) {
            out.push(ParseError {
                line: 0,
                message: format!("{} Check must be CHK-<digits>", ac.id),
            })
        }
        let unique: BTreeSet<_> = ac.covers.iter().collect();
        if unique.len() != ac.covers.len() {
            out.push(ParseError {
                line: 0,
                message: format!("{} Covers contains duplicate requirements", ac.id),
            })
        }
        if matches!(ac.kind, AcceptanceType::Scenario | AcceptanceType::Outline) {
            let w = ac.steps.iter().filter(|s| s.keyword == "When").count();
            let t = ac.steps.iter().filter(|s| s.keyword == "Then").count();
            if w != 1 || !(1..=3).contains(&t) {
                out.push(ParseError {
                    line: 0,
                    message: format!("{} scenario needs one When and 1-3 Then steps", ac.id),
                })
            }
        }
    }
    out
}
