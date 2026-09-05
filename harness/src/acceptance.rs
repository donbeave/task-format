//! Typed acceptance blocks embedded in the README acceptance section.
//!
//! This is deliberately a small task-oriented Gherkin profile. It is not a Cucumber parser and
//! has no runtime semantics: the block remains the single source of truth for evidence and shape.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

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
    pub evidence: String,
    pub expected: String,
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

fn err(errors: &mut Vec<ParseError>, line: usize, message: impl Into<String>) {
    errors.push(ParseError {
        line,
        message: message.into(),
    });
}

fn strip_value(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('`')
        .and_then(|v| v.strip_suffix('`'))
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn split_row(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    if !line.starts_with('|') || !line.ends_with('|') {
        return None;
    }
    Some(
        line[1..line.len() - 1]
            .split('|')
            .map(|v| v.trim().to_string())
            .collect(),
    )
}

fn parse_covers(value: &str) -> Vec<String> {
    let range = Regex::new(r"^R-([0-9]+)\.\.R-([0-9]+)$").expect("static regex");
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .flat_map(|part| {
            let Some(caps) = range.captures(part) else {
                return vec![part.to_string()];
            };
            let Some(first) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) else {
                return vec![part.to_string()];
            };
            let Some(last) = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok()) else {
                return vec![part.to_string()];
            };
            if first > last {
                return vec![part.to_string()];
            }
            let width = caps.get(1).map(|m| m.as_str().len()).unwrap_or(1);
            (first..=last)
                .map(|number| format!("R-{number:0width$}"))
                .collect()
        })
        .collect()
}

/// Parse all canonical H3 AC blocks. A non-typed legacy table produces an empty, clean document.
pub fn parse(text: &str) -> AcceptanceDocument {
    let heading = Regex::new(r"^###\s+(AC-[0-9]+)(?:\s+[—-]\s*)?(.*)\s*$").unwrap();
    let mut doc = AcceptanceDocument::default();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut markdown_fence = false;
    while i < lines.len() {
        let line = lines[i].trim_end();
        if line.trim_start().starts_with("```") {
            markdown_fence = !markdown_fence;
            i += 1;
            continue;
        }
        if markdown_fence {
            i += 1;
            continue;
        }
        let Some(caps) = heading.captures(line) else {
            i += 1;
            continue;
        };
        doc.detected = true;
        let start = i + 1;
        let id = caps[1].to_string();
        let title = caps[2].trim().to_string();
        let mut fields: BTreeMap<String, (String, usize)> = BTreeMap::new();
        i += 1;
        let end = (i..lines.len())
            .find(|&index| {
                lines[index].trim_start().starts_with("### ")
                    || lines[index].trim_start().starts_with("## ")
            })
            .unwrap_or(lines.len());
        let mut steps = Vec::new();
        let mut headers = Vec::new();
        let mut rows = Vec::new();
        let mut examples_seen = false;
        let mut gherkin_seen = false;
        let mut verification_seen = false;
        let mut evidence = String::new();
        let mut evidence_line = None;
        let mut metadata_order = Vec::new();
        while i < end {
            let line = lines[i].trim();
            if line.is_empty() {
                i += 1;
                continue;
            }
            if line == "**Verification**" {
                if verification_seen {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} duplicate Verification section"),
                    );
                }
                verification_seen = true;
                i += 1;
                continue;
            }
            if line == "```gherkin" {
                if verification_seen {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} Gherkin body must precede Verification"),
                    );
                }
                if gherkin_seen {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} duplicate Gherkin body"),
                    );
                }
                gherkin_seen = true;
                i += 1;
                let body_start = i + 1;
                while i < end && lines[i].trim() != "```" {
                    let body_line = lines[i].trim();
                    if body_line.eq_ignore_ascii_case("Examples:") {
                        examples_seen = true;
                    } else if let Some(row) = split_row(body_line) {
                        if !examples_seen {
                            err(
                                &mut doc.errors,
                                i + 1,
                                format!("{id} Examples row appears before Examples:"),
                            );
                        } else if headers.is_empty() {
                            headers = row;
                        } else {
                            rows.push(row);
                        }
                    } else if !body_line.is_empty() {
                        let mut parts = body_line.splitn(2, char::is_whitespace);
                        let keyword = parts.next().unwrap_or_default().trim_end_matches(':');
                        if ["Given", "When", "Then", "And", "But"].contains(&keyword) {
                            let body = parts.next().unwrap_or("").trim();
                            if body.is_empty() {
                                err(&mut doc.errors, i + 1, format!("{id} empty {keyword} step"));
                            }
                            steps.push(Step {
                                keyword: keyword.to_string(),
                                text: body.to_string(),
                            });
                        } else {
                            steps.push(Step {
                                keyword: "Statement".to_string(),
                                text: body_line.to_string(),
                            });
                        }
                    }
                    i += 1;
                }
                if i == end {
                    err(
                        &mut doc.errors,
                        body_start,
                        format!("{id} unterminated gherkin fence"),
                    );
                } else {
                    i += 1;
                }
                continue;
            }
            if line == "```sh" {
                if !verification_seen {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} Evidence command must follow Verification metadata"),
                    );
                }
                if !evidence.is_empty() {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} duplicate Evidence command"),
                    );
                }
                evidence_line = Some(i + 1);
                i += 1;
                let body_start = i + 1;
                let mut command = Vec::new();
                while i < end && lines[i].trim() != "```" {
                    command.push(lines[i].trim_end());
                    i += 1;
                }
                evidence = command.join("\n").trim().to_string();
                if i == end {
                    err(
                        &mut doc.errors,
                        body_start,
                        format!("{id} unterminated Evidence shell fence"),
                    );
                } else {
                    i += 1;
                }
                continue;
            }
            if let Some(metadata) = line.strip_prefix("- **") {
                if !verification_seen {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} metadata must follow **Verification**"),
                    );
                }
                if let Some((key, value)) = metadata.split_once(":**") {
                    let key = key.trim().to_ascii_lowercase();
                    if ["type", "covers", "expected"].contains(&key.as_str()) {
                        if fields.contains_key(&key) {
                            err(
                                &mut doc.errors,
                                i + 1,
                                format!("{id} duplicate {key} metadata"),
                            );
                        }
                        metadata_order.push(key.clone());
                        fields.insert(key, (strip_value(value), i + 1));
                    } else {
                        err(&mut doc.errors, i + 1, format!("{id} unexpected metadata"));
                    }
                } else {
                    err(
                        &mut doc.errors,
                        i + 1,
                        format!("{id} malformed metadata bullet"),
                    );
                }
                i += 1;
                continue;
            }
            err(
                &mut doc.errors,
                i + 1,
                format!("{id} unexpected acceptance content"),
            );
            i += 1;
        }
        for key in ["type", "covers", "expected"] {
            if fields
                .get(key)
                .is_some_and(|(value, _)| value.trim().is_empty())
            {
                err(
                    &mut doc.errors,
                    start,
                    format!("{id} {key} metadata is empty"),
                );
            }
        }
        let typ = fields.get("type").map(|v| v.0.as_str()).unwrap_or("");
        let kind = match typ {
            "scenario" => AcceptanceType::Scenario,
            "outline" => AcceptanceType::Outline,
            "invariant" => AcceptanceType::Invariant,
            "gate" => AcceptanceType::Gate,
            _ => {
                err(
                    &mut doc.errors,
                    fields.get("type").map(|v| v.1).unwrap_or(start),
                    format!("{id} Type must be scenario, outline, invariant, or gate"),
                );
                AcceptanceType::Scenario
            }
        };
        let required = ["type", "expected"];
        for key in required {
            if !fields.contains_key(key) {
                err(
                    &mut doc.errors,
                    start,
                    format!("{id} missing {key} metadata"),
                );
            }
        }
        if !matches!(kind, AcceptanceType::Gate) && !fields.contains_key("covers") {
            err(
                &mut doc.errors,
                start,
                format!("{id} missing Covers metadata"),
            );
        }
        if matches!(kind, AcceptanceType::Gate) && fields.contains_key("covers") {
            err(
                &mut doc.errors,
                fields.get("covers").map(|v| v.1).unwrap_or(start),
                format!("{id} gate must not have Covers metadata"),
            );
        }
        let expected_order: &[&str] = if matches!(kind, AcceptanceType::Gate) {
            &["type", "expected"]
        } else {
            &["type", "covers", "expected"]
        };
        if metadata_order
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != expected_order
        {
            err(
                &mut doc.errors,
                start,
                format!(
                    "{id} Verification bullets must be ordered: {}",
                    expected_order.join(", ")
                ),
            );
        }
        if evidence_line.is_some_and(|line| {
            fields
                .get("expected")
                .is_some_and(|(_, expected_line)| line < *expected_line)
        }) {
            err(
                &mut doc.errors,
                evidence_line.unwrap_or(start),
                format!("{id} Evidence command must follow Verification bullets"),
            );
        }
        if !gherkin_seen && !matches!(kind, AcceptanceType::Gate) {
            err(
                &mut doc.errors,
                start,
                format!("{id} missing ```gherkin body"),
            );
        }
        if !verification_seen {
            err(
                &mut doc.errors,
                start,
                format!("{id} missing **Verification** section"),
            );
        }
        if matches!(kind, AcceptanceType::Gate) && gherkin_seen {
            err(
                &mut doc.errors,
                start,
                format!("{id} gate must not have a Gherkin body"),
            );
        }
        if !matches!(kind, AcceptanceType::Outline) && (examples_seen || !rows.is_empty()) {
            err(
                &mut doc.errors,
                start,
                format!("{id} Examples are only valid for an outline"),
            );
        }
        let covers = fields
            .get("covers")
            .map(|v| parse_covers(&v.0))
            .unwrap_or_default();
        let expected = fields
            .get("expected")
            .map(|v| v.0.clone())
            .unwrap_or_default();
        doc.criteria.push(AcceptanceCriterion {
            id,
            title,
            kind,
            covers,
            evidence,
            expected,
            steps,
            examples: rows,
            example_headers: headers,
        });
    }
    doc
}

/// Semantic checks shared by lint and direct verification. Cross-file checks stay in lint.
pub fn validate_shape(doc: &AcceptanceDocument) -> Vec<ParseError> {
    let mut errors = doc.errors.clone();
    let placeholder = Regex::new(r"<([^<>]+)>").unwrap();
    let verifier = Regex::new(
        r"(?i)\b(test passes|command exits|exits 0|grep finds|clippy succeeds|gate reports)\b",
    )
    .unwrap();
    for ac in &doc.criteria {
        if ac.title.is_empty() {
            err(&mut errors, 0, format!("{} title is empty", ac.id));
        }
        if ac.evidence.is_empty() {
            err(&mut errors, 0, format!("{} Evidence is empty", ac.id));
        }
        if ac.expected.is_empty() {
            err(&mut errors, 0, format!("{} Expected is empty", ac.id));
        }
        if !matches!(ac.kind, AcceptanceType::Gate) && ac.covers.is_empty() {
            err(&mut errors, 0, format!("{} Covers is empty", ac.id));
        }
        let mut unique_covers = BTreeSet::new();
        for cover in &ac.covers {
            unique_covers.insert(cover);
        }
        if unique_covers.len() != ac.covers.len() {
            err(
                &mut errors,
                0,
                format!("{} Covers contains duplicate requirements", ac.id),
            );
        }
        if !matches!(ac.kind, AcceptanceType::Gate) && ac.steps.is_empty() {
            err(&mut errors, 0, format!("{} Gherkin body is empty", ac.id));
        }
        if matches!(ac.kind, AcceptanceType::Scenario | AcceptanceType::Outline) {
            let whens = ac.steps.iter().filter(|s| s.keyword == "When").count();
            let thens = ac.steps.iter().filter(|s| s.keyword == "Then").count();
            let givens = ac.steps.iter().filter(|s| s.keyword == "Given").count();
            if whens != 1 {
                err(
                    &mut errors,
                    0,
                    format!("{} must have exactly one When", ac.id),
                );
            }
            if !(1..=3).contains(&thens) {
                err(
                    &mut errors,
                    0,
                    format!("{} must have 1-3 Then steps", ac.id),
                );
            }
            if givens > 3 || ac.steps.len() > 6 {
                err(&mut errors, 0, format!("{} has too many steps", ac.id));
            }
        }
        let mut names = BTreeSet::new();
        for step in &ac.steps {
            for cap in placeholder.captures_iter(&step.text) {
                names.insert(cap[1].to_string());
            }
        }
        let columns: BTreeSet<String> = ac.example_headers.iter().cloned().collect();
        if matches!(ac.kind, AcceptanceType::Outline) {
            if ac.examples.len() < 2 {
                err(
                    &mut errors,
                    0,
                    format!("{} outline needs at least 2 example rows", ac.id),
                );
            }
            if ac.example_headers.is_empty() {
                err(
                    &mut errors,
                    0,
                    format!("{} outline needs Examples columns", ac.id),
                );
            }
            if ac.example_headers.iter().any(|header| header.is_empty()) {
                err(
                    &mut errors,
                    0,
                    format!("{} has an empty Examples column", ac.id),
                );
            }
            if columns.len() != ac.example_headers.len() {
                err(
                    &mut errors,
                    0,
                    format!("{} has duplicate Examples columns", ac.id),
                );
            }
            for row in &ac.examples {
                if row.len() != ac.example_headers.len() {
                    err(
                        &mut errors,
                        0,
                        format!("{} example row width differs from columns", ac.id),
                    );
                }
            }
            let mut unique = BTreeSet::new();
            for row in &ac.examples {
                unique.insert(row);
            }
            if unique.len() != ac.examples.len() {
                err(
                    &mut errors,
                    0,
                    format!("{} has duplicate example rows", ac.id),
                );
            }
            if names.iter().any(|name| !columns.contains(name)) {
                err(
                    &mut errors,
                    0,
                    format!("{} has placeholder without Examples column", ac.id),
                );
            }
            if columns.iter().any(|column| !names.contains(column)) {
                err(
                    &mut errors,
                    0,
                    format!("{} has unused Examples column", ac.id),
                );
            }
        } else if !names.is_empty() {
            err(
                &mut errors,
                0,
                format!("{} uses placeholders but is not an outline", ac.id),
            );
        }
        if ac.steps.iter().any(|step| verifier.is_match(&step.text)) {
            err(
                &mut errors,
                0,
                format!("{} Gherkin contains verifier language", ac.id),
            );
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENARIO: &str = r#"### AC-001 — Recoverable failure
```gherkin
Given a saved connection
When the connection fails
Then the list remains visible
And the error is shown
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002`
- **Expected:** exit 0

```sh
cargo test ac_001
```
"#;

    #[test]
    fn parses_all_types_and_metadata() {
        let text = format!(
            "{SCENARIO}\n### AC-002 — Static rule\n```gherkin\nThe rule is enforced\n```\n\n**Verification**\n\n- **Type:** invariant\n- **Covers:** `R-003`\n- **Expected:** no output\n\n```sh\ngrep rule\n```\n\n### AC-003 — Matrix\n```gherkin\nWhen sorted <direction>\nThen NULL is <placement>\nExamples:\n| direction | placement |\n| ascending | last |\n| descending | first |\n```\n\n**Verification**\n\n- **Type:** outline\n- **Covers:** `R-004`\n- **Expected:** exit 0\n\n```sh\ncargo test matrix\n```\n\n### AC-004 — Gate\n**Verification**\n\n- **Type:** gate\n- **Expected:** DONE\n\n```sh\ntaskfmt verify\n```\n"
        );
        let doc = parse(&text);
        assert!(doc.detected);
        assert_eq!(doc.criteria.len(), 4);
        assert!(
            validate_shape(&doc).is_empty(),
            "{:?}",
            validate_shape(&doc)
        );
    }

    #[test]
    fn parses_fenced_multiline_evidence() {
        let text = SCENARIO.replace(
            "cargo test ac_001",
            "grep -q first Cargo.toml &&\ngrep -q second Cargo.toml",
        );
        let doc = parse(&text);
        assert_eq!(
            doc.criteria[0].evidence,
            "grep -q first Cargo.toml &&\ngrep -q second Cargo.toml"
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert!(validate_shape(&doc).is_empty());
    }

    #[test]
    fn rejects_noncanonical_verification_forms() {
        for replacement in ["```bash", "Evidence:\n```sh", "- Type: scenario"] {
            let text = SCENARIO
                .replace("```sh", replacement)
                .replace("- **Type:** scenario", replacement);
            let doc = parse(&text);
            assert!(!doc.errors.is_empty(), "{replacement:?}: {:?}", doc.errors);
        }
    }

    #[test]
    fn rejects_unterminated_fenced_evidence() {
        let text = SCENARIO.trim_end_matches("```\n");
        let doc = parse(text);
        assert!(
            doc.errors
                .iter()
                .any(|error| error.message.contains("unterminated Evidence shell fence"))
        );
    }

    #[test]
    fn rejects_shape_and_outline_errors() {
        let bad = SCENARIO
            .replace(
                "When the connection fails",
                "When it fails\nWhen it retries",
            )
            .replace(
                "Then the list remains visible",
                "Then the list <missing> visible",
            );
        let doc = parse(&bad);
        let errors = validate_shape(&doc);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("exactly one When"))
        );
        assert!(errors.iter().any(|e| e.message.contains("not an outline")));
    }

    #[test]
    fn rejects_verifier_language_and_duplicate_metadata() {
        let bad = SCENARIO
            .replace(
                "- **Expected:** exit 0",
                "- **Expected:** exit 0\n- **Expected:** other",
            )
            .replace("Then the list remains visible", "Then the test passes");
        let doc = parse(&bad);
        let errors = validate_shape(&doc);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("duplicate expected"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("verifier language"))
        );
    }
}
