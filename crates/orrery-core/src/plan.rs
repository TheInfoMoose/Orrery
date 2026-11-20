use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StepKind {
    #[default]
    Agent,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    pub index: usize,
    pub number: usize,
    pub id: String,
    pub kind: StepKind,
    pub item_id: Option<String>,
    pub title: String,
    pub body: String,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationPlan {
    pub source_path: Option<PathBuf>,
    pub step_marker: String,
    pub preamble: String,
    pub steps: Vec<PlanStep>,
    pub completed_ids: BTreeSet<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_fence_open(line: &str) -> Option<(u8, usize)> {
    let bytes = line.as_bytes();
    let indent = bytes.iter().take_while(|&&b| b == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &bytes[indent..];
    let ch = *rest.first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let run_len = rest.iter().take_while(|&&b| b == ch).count();
    if run_len >= 3 {
        Some((ch, run_len))
    } else {
        None
    }
}

fn is_fence_close(line: &str, fence_char: u8, open_len: usize) -> bool {
    let bytes = line.as_bytes();
    let indent = bytes.iter().take_while(|&&b| b == b' ').count();
    if indent > 3 {
        return false;
    }
    let rest = &bytes[indent..];
    let run_len = rest.iter().take_while(|&&b| b == fence_char).count();
    if run_len < open_len {
        return false;
    }
    let after = &line[indent + run_len..];
    after.chars().all(char::is_whitespace)
}

fn join_and_trim(lines: &[&str]) -> String {
    lines.join("\n").trim_end_matches('\n').to_string()
}

pub fn parse_step_id(title: &str) -> Option<&str> {
    let bytes = title.as_bytes();
    let digit_len = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digit_len == 0 {
        return None;
    }
    let letter_len = bytes[digit_len..]
        .iter()
        .take_while(|b| b.is_ascii_uppercase())
        .count();
    let id_len = digit_len + letter_len;
    let rest = &title[id_len..];
    if rest.is_empty() || rest.starts_with(':') || rest.starts_with('.') || rest.starts_with(' ') {
        Some(&title[..id_len])
    } else {
        None
    }
}

pub fn is_step_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    let digit_len = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    digit_len > 0 && bytes[digit_len..].iter().all(u8::is_ascii_uppercase)
}

fn completed_row_id(line: &str, label: &str) -> Option<String> {
    if label.is_empty() {
        return None;
    }
    let after_first = line.trim_start().strip_prefix('|')?;
    let (raw_cell, _) = after_first.split_once('|')?;
    let cleaned: String = raw_cell.chars().filter(|&c| c != '*' && c != '`').collect();
    let rest = cleaned.trim().strip_prefix(label)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let id = rest.trim();
    if is_step_id(id) {
        Some(id.to_string())
    } else {
        None
    }
}

impl ImplementationPlan {
    pub fn parse(markdown: &str, step_marker: &str) -> Result<Self> {
        if step_marker.is_empty() {
            return Err(Error::PlanParse {
                line: 1,
                message: "step marker must not be empty".into(),
            });
        }

        let normalized = markdown.replace("\r\n", "\n");
        let label = step_marker.trim_start_matches('#').trim();
        let mut preamble_lines: Vec<&str> = Vec::new();
        let mut raw_steps: Vec<(usize, String, Vec<&str>)> = Vec::new();
        let mut active_fence: Option<(u8, usize, usize)> = None;
        let mut in_completed_section = false;
        let mut completed_ids: BTreeSet<String> = BTreeSet::new();

        for (idx, line) in normalized.split('\n').enumerate() {
            let line_num = idx + 1;

            if let Some((fence_char, open_len, _)) = active_fence {
                if is_fence_close(line, fence_char, open_len) {
                    active_fence = None;
                }
                if let Some((_, _, body_lines)) = raw_steps.last_mut() {
                    body_lines.push(line);
                } else {
                    preamble_lines.push(line);
                }
                continue;
            }

            if let Some((fence_char, open_len)) = parse_fence_open(line) {
                active_fence = Some((fence_char, open_len, line_num));
                if let Some((_, _, body_lines)) = raw_steps.last_mut() {
                    body_lines.push(line);
                } else {
                    preamble_lines.push(line);
                }
                continue;
            }

            if line.starts_with('#') {
                let heading_text = line.trim_start_matches('#').trim();
                in_completed_section = heading_text.eq_ignore_ascii_case("Completed");
            } else if in_completed_section && let Some(id) = completed_row_id(line, label) {
                completed_ids.insert(id);
            }

            if let Some(after_marker) = line.strip_prefix(step_marker) {
                let title = after_marker.trim().to_string();
                raw_steps.push((line_num, title, Vec::new()));
            } else if let Some((_, _, body_lines)) = raw_steps.last_mut() {
                body_lines.push(line);
            } else {
                preamble_lines.push(line);
            }
        }

        if let Some((_, _, open_line)) = active_fence {
            return Err(Error::PlanParse {
                line: open_line,
                message: "unclosed code fence".into(),
            });
        }

        if raw_steps.is_empty() {
            return Err(Error::PlanParse {
                line: 1,
                message: "plan contains no steps".into(),
            });
        }

        let parsed_ids: Vec<Option<String>> = raw_steps
            .iter()
            .map(|(_, title, _)| parse_step_id(title).map(str::to_string))
            .collect();
        let any_has_id = parsed_ids.iter().any(Option::is_some);
        let mut seen_ids: HashMap<String, usize> = HashMap::new();

        let mut steps = Vec::with_capacity(raw_steps.len());
        for (index, ((marker_line, title, body_lines), parsed_id)) in
            raw_steps.into_iter().zip(parsed_ids).enumerate()
        {
            let number = index + 1;
            let id = if any_has_id {
                match parsed_id {
                    Some(id) => {
                        if let Some(&first_line) = seen_ids.get(&id) {
                            return Err(Error::PlanParse {
                                line: marker_line,
                                message: format!(
                                    "duplicate step ID '{id}' (first used on line {first_line})"
                                ),
                            });
                        }
                        seen_ids.insert(id.clone(), marker_line);
                        id
                    }
                    None => {
                        return Err(Error::PlanParse {
                            line: marker_line,
                            message: "step heading without an ID (e.g. '12:' or '12A:') in a plan that uses IDs".into(),
                        });
                    }
                }
            } else {
                number.to_string()
            };

            let body = join_and_trim(&body_lines);
            let hash_input = format!("{title}\n{body}");
            let step_hash = sha256_hex(hash_input.as_bytes());

            steps.push(PlanStep {
                index,
                number,
                id,
                kind: StepKind::Agent,
                item_id: None,
                title,
                body,
                sha256_hex: step_hash,
            });
        }

        Ok(Self {
            source_path: None,
            step_marker: step_marker.to_string(),
            preamble: join_and_trim(&preamble_lines),
            steps,
            completed_ids,
        })
    }

    pub fn from_file(path: &Path, step_marker: &str) -> Result<Self> {
        let markdown = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut plan = Self::parse(&markdown, step_marker)?;
        plan.source_path = Some(path.to_path_buf());
        Ok(plan)
    }

    pub fn plan_sha256(&self) -> String {
        let joined: String = self
            .steps
            .iter()
            .map(|s| s.sha256_hex.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        sha256_hex(joined.as_bytes())
    }

    pub fn step(&self, number: usize) -> Option<&PlanStep> {
        self.steps.iter().find(|s| s.number == number)
    }

    pub fn step_by_id(&self, id: &str) -> Option<&PlanStep> {
        self.steps.iter().find(|s| s.id == id)
    }

    pub fn resolve_step(&self, arg: &str) -> Option<&PlanStep> {
        self.step_by_id(arg)
            .or_else(|| arg.parse::<usize>().ok().and_then(|n| self.step(n)))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{ImplementationPlan, StepKind, is_step_id, sha256_hex};
    use crate::Error;

    const SAMPLE_PLAN: &str = "intro\n## S 1: a\nbody a\n## S 2: b\nbody b\n";

    #[test]
    fn sha256_hex_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parses_preamble_and_steps() {
        let plan = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        assert_eq!(plan.source_path, None);
        assert_eq!(plan.step_marker, "## S ");
        assert_eq!(plan.preamble, "intro");
        assert_eq!(plan.steps.len(), 2);

        assert_eq!(plan.steps[0].index, 0);
        assert_eq!(plan.steps[0].number, 1);
        assert_eq!(plan.steps[0].title, "1: a");
        assert_eq!(plan.steps[0].body, "body a");

        assert_eq!(plan.steps[1].index, 1);
        assert_eq!(plan.steps[1].number, 2);
        assert_eq!(plan.steps[1].title, "2: b");
        assert_eq!(plan.steps[1].body, "body b");
    }

    #[test]
    fn marker_inside_backtick_fence_ignored() {
        let input = "## S 1: a\n```\n## S 2: fake\n```\n";
        let plan = ImplementationPlan::parse(input, "## S ").unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert!(plan.steps[0].body.contains("## S 2: fake"));
    }

    #[test]
    fn marker_inside_tilde_fence_ignored() {
        let input = "## S 1: a\n~~~\n## S 2: fake\n~~~\n";
        let plan = ImplementationPlan::parse(input, "## S ").unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert!(plan.steps[0].body.contains("## S 2: fake"));
    }

    #[test]
    fn longer_fence_contains_shorter_fence_line() {
        let input = "## S 1: a\n````\n```\n## S 2: fake\n````\n";
        let plan = ImplementationPlan::parse(input, "## S ").unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert!(plan.steps[0].body.contains("## S 2: fake"));
    }

    #[test]
    fn unclosed_fence_errors_with_line() {
        let input = "## S 1: a\ntext\n```\n## S 2: b\n";
        let err = ImplementationPlan::parse(input, "## S ").unwrap_err();
        assert!(matches!(err, Error::PlanParse { line: 3, .. }));
    }

    #[test]
    fn empty_marker_rejected() {
        let err = ImplementationPlan::parse("## S 1: a\n", "").unwrap_err();
        assert!(matches!(err, Error::PlanParse { .. }));
    }

    #[test]
    fn zero_steps_rejected() {
        let err = ImplementationPlan::parse("no markers here\n", "## S ").unwrap_err();
        assert!(matches!(err, Error::PlanParse { .. }));
    }

    #[test]
    fn ids_allow_gaps_and_letters() {
        let plan =
            ImplementationPlan::parse("## S 1: a\n## S 3: b\n## S 3A: c\n", "## S ").unwrap();
        let ids: Vec<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
        let numbers: Vec<usize> = plan.steps.iter().map(|s| s.number).collect();
        assert_eq!(ids, vec!["1", "3", "3A"]);
        assert_eq!(numbers, vec![1, 2, 3]);
        assert!(plan.steps.iter().all(|s| s.kind == StepKind::Agent));
        assert!(plan.steps.iter().all(|s| s.item_id.is_none()));
        assert!(is_step_id("3A"));
        assert!(!is_step_id("3a"));
        assert!(!is_step_id("A3"));
    }

    #[test]
    fn mixed_numbered_and_unnumbered_rejected() {
        let err = ImplementationPlan::parse("## S 1: a\n## S b\n", "## S ").unwrap_err();
        assert!(matches!(err, Error::PlanParse { line: 2, .. }));
    }

    #[test]
    fn duplicate_id_rejected() {
        let err =
            ImplementationPlan::parse("## S 1: a\n## S 2: b\n## S 1: c\n", "## S ").unwrap_err();
        match err {
            Error::PlanParse { line: 3, message } => {
                assert!(message.contains("duplicate step ID '1'"));
                assert!(message.contains("line 1"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn unnumbered_titles_use_index() {
        let plan = ImplementationPlan::parse("## S alpha\n## S beta\n", "## S ").unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].number, 1);
        assert_eq!(plan.steps[0].id, "1");
        assert_eq!(plan.steps[1].number, 2);
        assert_eq!(plan.steps[1].id, "2");
    }

    #[test]
    fn resolve_step_prefers_id_then_position() {
        let plan = ImplementationPlan::parse("## S 2: a\n## S 1: b\n", "## S ").unwrap();
        assert_eq!(
            plan.resolve_step("1").map(|s| s.title.as_str()),
            Some("1: b")
        );
        assert!(plan.resolve_step("9").is_none());
        assert_eq!(plan.step_by_id("2").map(|s| s.title.as_str()), Some("2: a"));

        let lettered = ImplementationPlan::parse("## S 1A: a\n## S 1B: b\n", "## S ").unwrap();
        assert_eq!(
            lettered.resolve_step("2").map(|s| s.title.as_str()),
            Some("1B: b")
        );
        assert!(lettered.resolve_step("1C").is_none());
    }

    #[test]
    fn crlf_and_lf_hash_identical() {
        let lf_plan = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        let crlf_input = SAMPLE_PLAN.replace('\n', "\r\n");
        let crlf_plan = ImplementationPlan::parse(&crlf_input, "## S ").unwrap();

        assert_eq!(lf_plan.steps.len(), crlf_plan.steps.len());
        for (lf_step, crlf_step) in lf_plan.steps.iter().zip(crlf_plan.steps.iter()) {
            assert_eq!(lf_step.sha256_hex, crlf_step.sha256_hex);
        }
        assert_eq!(lf_plan.plan_sha256(), crlf_plan.plan_sha256());
    }

    #[test]
    fn body_edit_changes_hash() {
        let plan1 = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        let edited_input = SAMPLE_PLAN.replace("body b", "body c");
        let plan2 = ImplementationPlan::parse(&edited_input, "## S ").unwrap();

        assert_eq!(plan1.steps[0].sha256_hex, plan2.steps[0].sha256_hex);
        assert_ne!(plan1.steps[1].sha256_hex, plan2.steps[1].sha256_hex);
        assert_ne!(plan1.plan_sha256(), plan2.plan_sha256());
    }

    #[test]
    fn step_lookup_by_number() {
        let plan = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        assert_eq!(plan.step(2).map(|s| s.title.as_str()), Some("2: b"));
        assert!(plan.step(3).is_none());
        assert!(plan.step(0).is_none());
    }

    #[test]
    fn from_file_reads_and_sets_source_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("plan.md");
        std::fs::write(&file_path, SAMPLE_PLAN).unwrap();

        let plan = ImplementationPlan::from_file(&file_path, "## S ").unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.source_path.as_deref(), Some(file_path.as_path()));
    }

    #[test]
    fn from_file_missing_is_io_error() {
        let dir = TempDir::new().unwrap();
        let missing_path = dir.path().join("missing.md");
        let err = ImplementationPlan::from_file(&missing_path, "## S ").unwrap_err();
        assert!(matches!(err, Error::Io { .. }));
    }

    #[test]
    fn completed_table_ids() {
        let input = [
            "# P",
            "### Up Next",
            "| # | Step |",
            "| 1 | **STEP 3** |",
            "### Completed",
            "| Step | Status |",
            "| :--- | :---: |",
            "| **STEP 1** | **COMPLETED** |",
            "| STEP 2A | done abc1234 |",
            "| [USER] CI-1 | done |",
            "| `STEP 9` | x |",
            "| STEP x1 | bad id |",
            "### Notes",
            "| STEP 3 | not done |",
            "### STEP 1: a",
            "body",
            "```",
            "### Completed",
            "| STEP 3 | fenced |",
            "```",
            "### STEP 2A: b",
            "### STEP 3: c",
        ]
        .join("\n");
        let plan = ImplementationPlan::parse(&input, "### STEP ").unwrap();
        let expected: std::collections::BTreeSet<String> =
            ["1", "2A", "9"].into_iter().map(String::from).collect();
        assert_eq!(plan.completed_ids, expected);
        let ids: Vec<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "2A", "3"]);
    }

    #[test]
    fn completed_table_absent_is_empty() {
        let plan = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        assert!(plan.completed_ids.is_empty());
    }

    #[test]
    fn acceptance_fixture_counts_plan_steps() {
        let plan = ImplementationPlan::parse(
            include_str!("../../../STEPPED_IMPLEMENTATION_PLAN.md"),
            "### STEP ",
        )
        .unwrap();
        assert_eq!(plan.steps.len(), 122);
        let numbers: Vec<usize> = plan.steps.iter().map(|s| s.number).collect();
        assert_eq!(numbers, (1..=122).collect::<Vec<_>>());
        assert!(plan.steps.iter().all(|s| s.id == s.number.to_string()));
        assert!(plan.steps[5].title.starts_with("6: "));
        assert!(plan.completed_ids.contains("1"));
        assert!(plan.completed_ids.contains("110"));
    }
}
