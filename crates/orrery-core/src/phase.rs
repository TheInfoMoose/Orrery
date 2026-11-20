use std::collections::HashSet;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::plan::{ImplementationPlan, PlanStep, StepKind, is_step_id, sha256_hex};
use crate::{Error, Result};

pub const PHASE_FILE: &str = "phase.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseQueue {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub phase: String,
    #[serde(default)]
    pub queue: Vec<QueueEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl PhaseQueue {
    pub fn validate(&self) -> Result<()> {
        if self.queue.is_empty() {
            return Err(Error::Validation {
                field: "phase.queue".into(),
                message: "phase.toml has no [[queue]] entries".into(),
            });
        }

        let mut seen_ids: HashSet<&str> = HashSet::with_capacity(self.queue.len());

        for (i, entry) in self.queue.iter().enumerate() {
            let field = format!("phase.queue[{i}]");

            let entry_id = match (&entry.step, &entry.user) {
                (Some(s), None) => {
                    if !is_step_id(s) {
                        return Err(Error::Validation {
                            field,
                            message: format!("step '{s}' is not a step ID (e.g. 12 or 12A)"),
                        });
                    }
                    s.as_str()
                }
                (None, Some(u)) => {
                    if u.is_empty()
                        || !u
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    {
                        return Err(Error::Validation {
                            field,
                            message: format!("user '{u}' must use only letters, digits, - and _"),
                        });
                    }
                    u.as_str()
                }
                _ => {
                    return Err(Error::Validation {
                        field,
                        message: "each entry needs exactly one of step or user".into(),
                    });
                }
            };

            if let Some(item) = &entry.item
                && (item.is_empty() || item.chars().any(char::is_whitespace))
            {
                return Err(Error::Validation {
                    field,
                    message: "item must be a non-empty ID without whitespace".into(),
                });
            }

            if entry.step.is_some() && entry.title.is_some() {
                return Err(Error::Validation {
                    field,
                    message: "title is only for user entries".into(),
                });
            }

            if !seen_ids.insert(entry_id) {
                return Err(Error::Validation {
                    field,
                    message: format!("duplicate queue ID '{entry_id}'"),
                });
            }
        }

        Ok(())
    }

    pub fn load(orrery_dir: &Path) -> Result<Option<PhaseQueue>> {
        let path = orrery_dir.join(PHASE_FILE);
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Io { path, source }),
        };
        let queue: PhaseQueue =
            toml::from_str(&raw).map_err(|source| Error::TomlDe { path, source })?;
        queue.validate()?;
        Ok(Some(queue))
    }
}

pub fn apply_queue(plan: &ImplementationPlan, queue: &PhaseQueue) -> Result<ImplementationPlan> {
    let mut steps = Vec::with_capacity(queue.queue.len());

    for (i, entry) in queue.queue.iter().enumerate() {
        let mut step = if let Some(id) = &entry.step {
            let mut cloned = plan
                .step_by_id(id)
                .ok_or_else(|| Error::Validation {
                    field: format!("phase.queue[{i}]"),
                    message: format!("step '{id}' is not in the plan"),
                })?
                .clone();
            cloned.item_id = entry.item.clone();
            cloned
        } else if let Some(user) = &entry.user {
            let label = entry.title.as_deref().unwrap_or(user.as_str());
            let title = format!("{user}: {label}");
            let body = entry.title.clone().unwrap_or_default();
            let hash_input = format!("user\n{user}\n{body}");
            PlanStep {
                index: i,
                number: i + 1,
                id: user.clone(),
                kind: StepKind::User,
                item_id: entry.item.clone(),
                title,
                body,
                sha256_hex: sha256_hex(hash_input.as_bytes()),
            }
        } else {
            return Err(Error::Validation {
                field: format!("phase.queue[{i}]"),
                message: "each entry needs exactly one of step or user".into(),
            });
        };

        step.index = i;
        step.number = i + 1;
        steps.push(step);
    }

    Ok(ImplementationPlan {
        source_path: plan.source_path.clone(),
        step_marker: plan.step_marker.clone(),
        preamble: plan.preamble.clone(),
        steps,
        completed_ids: plan.completed_ids.clone(),
    })
}

pub fn load_run_plan(
    plan_path: &Path,
    step_marker: &str,
    orrery_dir: &Path,
) -> Result<ImplementationPlan> {
    let plan = ImplementationPlan::from_file(plan_path, step_marker)?;
    match PhaseQueue::load(orrery_dir)? {
        Some(queue) => apply_queue(&plan, &queue),
        None => Ok(plan),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{PHASE_FILE, PhaseQueue, QueueEntry, apply_queue, load_run_plan};
    use crate::Error;
    use crate::plan::{ImplementationPlan, StepKind};

    const SAMPLE_PLAN: &str = "## S 1: a\nA\n## S 2: b\nB\n## S 2A: c\nC\n## S 3: d\nD\n";

    #[test]
    fn queue_selects_and_orders_steps() {
        let plan = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        let queue = PhaseQueue {
            phase: "O".into(),
            queue: vec![
                QueueEntry {
                    step: Some("3".into()),
                    item: Some("TASK-1".into()),
                    ..Default::default()
                },
                QueueEntry {
                    user: Some("LOOP-1".into()),
                    title: Some("Check".into()),
                    ..Default::default()
                },
                QueueEntry {
                    step: Some("1".into()),
                    ..Default::default()
                },
            ],
        };
        queue.validate().unwrap();
        let run_plan = apply_queue(&plan, &queue).unwrap();

        let ids: Vec<&str> = run_plan.steps.iter().map(|s| s.id.as_str()).collect();
        let numbers: Vec<usize> = run_plan.steps.iter().map(|s| s.number).collect();
        let indexes: Vec<usize> = run_plan.steps.iter().map(|s| s.index).collect();
        let kinds: Vec<StepKind> = run_plan.steps.iter().map(|s| s.kind).collect();
        let item_ids: Vec<Option<&str>> = run_plan
            .steps
            .iter()
            .map(|s| s.item_id.as_deref())
            .collect();

        assert_eq!(ids, vec!["3", "LOOP-1", "1"]);
        assert_eq!(numbers, vec![1, 2, 3]);
        assert_eq!(indexes, vec![0, 1, 2]);
        assert_eq!(
            kinds,
            vec![StepKind::Agent, StepKind::User, StepKind::Agent]
        );
        assert_eq!(item_ids, vec![Some("TASK-1"), None, None]);

        assert_eq!(run_plan.steps[1].title, "LOOP-1: Check");
        assert_eq!(run_plan.steps[1].body, "Check");

        let orig_step_3 = plan.step_by_id("3").unwrap();
        assert_eq!(run_plan.steps[0].sha256_hex, orig_step_3.sha256_hex);
        assert_eq!(run_plan.steps[0].body, orig_step_3.body);
        assert_eq!(run_plan.step_marker, plan.step_marker);

        let untitled_queue = PhaseQueue {
            phase: String::new(),
            queue: vec![QueueEntry {
                user: Some("U".into()),
                ..Default::default()
            }],
        };
        let untitled_plan = apply_queue(&plan, &untitled_queue).unwrap();
        assert_eq!(untitled_plan.steps[0].title, "U: U");
        assert_eq!(untitled_plan.steps[0].body, "");
    }

    #[test]
    fn queue_rejects_bad_entries() {
        let empty = PhaseQueue::default();
        assert!(matches!(
            empty.validate(),
            Err(Error::Validation { field, message })
                if field == "phase.queue" && message == "phase.toml has no [[queue]] entries"
        ));

        let check_entry = |entry: QueueEntry, expected_msg: &str| {
            let q = PhaseQueue {
                phase: String::new(),
                queue: vec![entry],
            };
            match q.validate() {
                Err(Error::Validation { field, message }) => {
                    assert_eq!(field, "phase.queue[0]");
                    assert_eq!(message, expected_msg);
                }
                other => panic!("expected Error::Validation, got {other:?}"),
            }
        };

        check_entry(
            QueueEntry {
                step: Some("1".into()),
                user: Some("U1".into()),
                ..Default::default()
            },
            "each entry needs exactly one of step or user",
        );

        check_entry(
            QueueEntry::default(),
            "each entry needs exactly one of step or user",
        );

        check_entry(
            QueueEntry {
                step: Some("x1".into()),
                ..Default::default()
            },
            "step 'x1' is not a step ID (e.g. 12 or 12A)",
        );

        check_entry(
            QueueEntry {
                user: Some("a b".into()),
                ..Default::default()
            },
            "user 'a b' must use only letters, digits, - and _",
        );

        check_entry(
            QueueEntry {
                step: Some("1".into()),
                item: Some("T 1".into()),
                ..Default::default()
            },
            "item must be a non-empty ID without whitespace",
        );

        check_entry(
            QueueEntry {
                step: Some("1".into()),
                title: Some("nope".into()),
                ..Default::default()
            },
            "title is only for user entries",
        );

        let dup = PhaseQueue {
            phase: String::new(),
            queue: vec![
                QueueEntry {
                    step: Some("1".into()),
                    ..Default::default()
                },
                QueueEntry {
                    step: Some("1".into()),
                    ..Default::default()
                },
            ],
        };
        assert!(matches!(
            dup.validate(),
            Err(Error::Validation { field, message })
                if field == "phase.queue[1]" && message == "duplicate queue ID '1'"
        ));

        let plan = ImplementationPlan::parse(SAMPLE_PLAN, "## S ").unwrap();
        let missing_q = PhaseQueue {
            phase: String::new(),
            queue: vec![QueueEntry {
                step: Some("9".into()),
                ..Default::default()
            }],
        };
        assert!(matches!(
            apply_queue(&plan, &missing_q),
            Err(Error::Validation { field, message })
                if field == "phase.queue[0]" && message.contains("step '9' is not in the plan")
        ));
    }

    #[test]
    fn load_run_plan_reads_phase_file() {
        let dir = TempDir::new().unwrap();
        let plan_path = dir.path().join("plan.md");
        let orrery_dir = dir.path().join(".orrery");
        std::fs::create_dir_all(&orrery_dir).unwrap();
        std::fs::write(&plan_path, SAMPLE_PLAN).unwrap();

        let full = load_run_plan(&plan_path, "## S ", &orrery_dir).unwrap();
        let full_ids: Vec<&str> = full.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(full_ids, vec!["1", "2", "2A", "3"]);

        let phase_path = orrery_dir.join(PHASE_FILE);
        std::fs::write(
            &phase_path,
            "phase = \"O\"\n[[queue]]\nstep = \"2A\"\n[[queue]]\nuser = \"U1\"\ntitle = \"Check\"\n",
        )
        .unwrap();
        let queued = load_run_plan(&plan_path, "## S ", &orrery_dir).unwrap();
        let queued_ids: Vec<&str> = queued.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(queued_ids, vec!["2A", "U1"]);

        std::fs::write(&phase_path, "[[queue]]\nstep = \"1\"\nbogus = 1\n").unwrap();
        assert!(matches!(
            load_run_plan(&plan_path, "## S ", &orrery_dir),
            Err(Error::TomlDe { .. })
        ));
    }
}
