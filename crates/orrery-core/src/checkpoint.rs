use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{HookEvent, RoleConfig, RoleKind, atomic_write, validate_safe_relative_path};
use crate::plan::ImplementationPlan;
use crate::{Error, Result};

pub const MAX_SUMMARY_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    Dispatch,
    StepPassed,
    Escalate,
    Complete,
    Blocked,
    Confirmed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubtaskAllocation {
    pub target_agent_id: String,
    pub instructions: String,
    #[serde(default)]
    pub write_scope: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentReport {
    pub status: ReportStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_stage: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocations: Vec<SubtaskAllocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
}

impl AgentReport {
    pub fn validate_for_role(&self, role: &RoleConfig, stage_count: usize) -> Result<()> {
        let allowed = match role.kind {
            RoleKind::Orchestrator => matches!(
                self.status,
                ReportStatus::Dispatch | ReportStatus::StepPassed | ReportStatus::Escalate
            ),
            RoleKind::Implementer => {
                matches!(self.status, ReportStatus::Complete | ReportStatus::Blocked)
            }
            RoleKind::Validator => {
                matches!(self.status, ReportStatus::Confirmed | ReportStatus::Denied)
            }
        };
        if !allowed {
            return Err(Error::Report {
                message: format!(
                    "status {:?} is not allowed for role kind {:?}",
                    self.status, role.kind
                ),
            });
        }

        if self.summary.trim().is_empty() {
            return Err(Error::Report {
                message: "summary must not be empty".to_string(),
            });
        }
        if self.summary.len() > MAX_SUMMARY_BYTES {
            return Err(Error::Report {
                message: format!(
                    "summary length {} exceeds maximum of {MAX_SUMMARY_BYTES} bytes",
                    self.summary.len()
                ),
            });
        }

        if self.status == ReportStatus::Dispatch {
            match self.target_stage {
                Some(t) if (1..stage_count).contains(&t) => {}
                Some(t) => {
                    return Err(Error::Report {
                        message: format!("Dispatch target_stage {t} must be in 1..{stage_count}"),
                    });
                }
                None => {
                    return Err(Error::Report {
                        message: "Dispatch report requires target_stage".to_string(),
                    });
                }
            }

            if self.allocations.is_empty() {
                return Err(Error::Report {
                    message: "Dispatch report requires non-empty allocations".to_string(),
                });
            }

            for (idx, alloc) in self.allocations.iter().enumerate() {
                if alloc.target_agent_id.trim().is_empty() {
                    return Err(Error::Report {
                        message: format!("allocations[{idx}].target_agent_id must not be empty"),
                    });
                }
                if alloc.instructions.trim().is_empty() {
                    return Err(Error::Report {
                        message: format!("allocations[{idx}].instructions must not be empty"),
                    });
                }
                for scope in &alloc.write_scope {
                    validate_safe_relative_path(Path::new(scope), "write_scope").map_err(
                        |err| Error::Report {
                            message: format!("invalid write_scope '{scope}': {err}"),
                        },
                    )?;
                }
            }
        } else {
            if self.target_stage.is_some() {
                return Err(Error::Report {
                    message: "target_stage must be omitted when status is not Dispatch".to_string(),
                });
            }
            if !self.allocations.is_empty() {
                return Err(Error::Report {
                    message: "allocations must be empty when status is not Dispatch".to_string(),
                });
            }
        }

        if matches!(
            self.status,
            ReportStatus::Denied | ReportStatus::Blocked | ReportStatus::Escalate
        ) && self
            .feedback
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
        {
            return Err(Error::Report {
                message: format!("{:?} report requires non-empty feedback", self.status),
            });
        }

        Ok(())
    }
}

pub fn report_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["status", "summary"],
        "properties": {
            "status": {
                "type": "string",
                "enum": [
                    "Dispatch",
                    "StepPassed",
                    "Escalate",
                    "Complete",
                    "Blocked",
                    "Confirmed",
                    "Denied"
                ]
            },
            "summary": {
                "type": "string"
            },
            "target_stage": {
                "type": "integer",
                "minimum": 1
            },
            "allocations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target_agent_id", "instructions"],
                    "properties": {
                        "target_agent_id": { "type": "string" },
                        "instructions": { "type": "string" },
                        "write_scope": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    }
                }
            },
            "feedback": {
                "type": "string"
            },
            "artifacts": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StampedReport {
    pub step_number: usize,
    pub plan_step_sha256: String,
    pub round: usize,
    pub stage_index: usize,
    pub role_id: String,
    pub agent_id: String,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub report: AgentReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress,
    Passed,
    AwaitingOperator,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnginePhase {
    Idle,
    OrchestratorTurn,
    StageTurn,
    Gate,
    AwaitingOperator,
    Hook,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorDecision {
    Approve,
    Reject { feedback: String },
}

pub const MAX_HOOK_RESULTS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookResult {
    pub event: HookEvent,
    pub step_number: usize,
    pub tool: String,
    pub success: bool,
    pub output: String,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCheckpoint {
    pub run_id: uuid::Uuid,
    pub plan_sha256: String,
    pub step_hashes: Vec<String>,
    pub step_statuses: Vec<StepStatus>,
    pub current_step_index: usize,
    pub round: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_stage_index: Option<usize>,
    pub retry_count: usize,
    pub steps_passed_this_run: usize,
    pub phase: EnginePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awaiting_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_decision: Option<OperatorDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hook_results: Vec<HookResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileOutcome {
    Unchanged,
    Updated {
        changed_steps: Vec<usize>,
        appended: usize,
        removed: usize,
    },
}

impl WorkflowCheckpoint {
    pub fn new(plan: &ImplementationPlan) -> Self {
        let step_hashes = plan.steps.iter().map(|s| s.sha256_hex.clone()).collect();
        let step_statuses = vec![StepStatus::Pending; plan.steps.len()];
        Self {
            run_id: uuid::Uuid::new_v4(),
            plan_sha256: plan.plan_sha256(),
            step_hashes,
            step_statuses,
            current_step_index: 0,
            round: 0,
            active_stage_index: None,
            retry_count: 0,
            steps_passed_this_run: 0,
            phase: EnginePhase::Idle,
            awaiting_reason: None,
            operator_decision: None,
            hook_results: Vec::new(),
        }
    }

    pub fn reconcile(&mut self, plan: &ImplementationPlan) -> Result<ReconcileOutcome> {
        let old_len = self.step_hashes.len();
        let new_len = plan.steps.len();
        let common_len = old_len.min(new_len);

        let mut changed_steps = Vec::new();
        for i in 0..common_len {
            if self.step_hashes[i] != plan.steps[i].sha256_hex {
                let status = self.step_statuses[i];
                let current_active = i == self.current_step_index
                    && matches!(
                        status,
                        StepStatus::InProgress | StepStatus::AwaitingOperator
                    );
                if status == StepStatus::Passed || current_active {
                    let step_number = i + 1;
                    return Err(Error::Engine {
                        step_number,
                        message: format!("step edited; run `orrery reset --step {step_number}`"),
                    });
                }
                changed_steps.push(i);
            }
        }

        if new_len < old_len {
            for i in new_len..old_len {
                if self.step_statuses[i] != StepStatus::Pending {
                    let step_number = i + 1;
                    return Err(Error::Engine {
                        step_number,
                        message: format!("step edited; run `orrery reset --step {step_number}`"),
                    });
                }
            }
        }

        for &i in &changed_steps {
            self.step_hashes[i] = plan.steps[i].sha256_hex.clone();
        }

        let removed = old_len.saturating_sub(new_len);
        if removed > 0 {
            self.step_hashes.truncate(new_len);
            self.step_statuses.truncate(new_len);
            if new_len > 0 && self.current_step_index >= new_len {
                self.current_step_index = new_len - 1;
            }
        }

        let appended = new_len.saturating_sub(old_len);
        if appended > 0 {
            for step in &plan.steps[old_len..new_len] {
                self.step_hashes.push(step.sha256_hex.clone());
                self.step_statuses.push(StepStatus::Pending);
            }
        }

        self.plan_sha256 = plan.plan_sha256();

        if changed_steps.is_empty() && appended == 0 && removed == 0 {
            Ok(ReconcileOutcome::Unchanged)
        } else {
            Ok(ReconcileOutcome::Updated {
                changed_steps,
                appended,
                removed,
            })
        }
    }

    pub fn reset_from(&mut self, step_number: usize) -> Result<()> {
        if step_number == 0 || step_number > self.step_statuses.len() {
            return Err(Error::Engine {
                step_number,
                message: format!(
                    "step {step_number} is out of range 1..={}",
                    self.step_statuses.len()
                ),
            });
        }

        let start_index = step_number - 1;
        for status in &mut self.step_statuses[start_index..] {
            *status = StepStatus::Pending;
        }
        self.current_step_index = start_index;
        self.round = 0;
        self.active_stage_index = None;
        self.retry_count = 0;
        self.phase = EnginePhase::Idle;
        self.awaiting_reason = None;
        self.operator_decision = None;
        Ok(())
    }

    pub fn push_hook_result(&mut self, result: HookResult) {
        self.hook_results.push(result);
        while self.hook_results.len() > MAX_HOOK_RESULTS {
            self.hook_results.remove(0);
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckpointManager {
    dir: PathBuf,
}

impl CheckpointManager {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn save_state(&self, state: &WorkflowCheckpoint) -> Result<()> {
        let path = self.dir.join("workflow_state.toml");
        let encoded = toml::to_string(state)?;
        atomic_write(&path, encoded.as_bytes())
    }

    pub fn load_state(&self) -> Result<Option<WorkflowCheckpoint>> {
        let path = self.dir.join("workflow_state.toml");
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Io { path, source }),
        };
        let state = toml::from_str::<WorkflowCheckpoint>(&raw)
            .map_err(|source| Error::TomlDe { path, source })?;
        Ok(Some(state))
    }

    pub fn save_report(&self, report: &StampedReport) -> Result<PathBuf> {
        let step_dir = self.dir.join(format!("step_{:03}", report.step_number));
        let file_name = format!(
            "round_{:02}_stage_{:02}_{}.toml",
            report.round, report.stage_index, report.agent_id
        );
        let path = step_dir.join(file_name);
        let encoded = toml::to_string(report)?;
        atomic_write(&path, encoded.as_bytes())?;
        Ok(path)
    }

    pub fn load_reports(
        &self,
        step_number: usize,
        round: Option<usize>,
    ) -> Result<Vec<StampedReport>> {
        let step_dir = self.dir.join(format!("step_{step_number:03}"));
        let entries = match fs::read_dir(&step_dir) {
            Ok(iter) => iter,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(Error::Io {
                    path: step_dir,
                    source,
                });
            }
        };

        let mut reports = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                path: step_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(OsStr::to_str) != Some("toml") {
                continue;
            }
            let raw = fs::read_to_string(&path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
            let report = toml::from_str::<StampedReport>(&raw)
                .map_err(|source| Error::TomlDe { path, source })?;
            if let Some(target_round) = round
                && report.round != target_round
            {
                continue;
            }
            reports.push(report);
        }

        reports.sort_by(|a, b| {
            (a.round, a.stage_index, a.agent_id.as_str()).cmp(&(
                b.round,
                b.stage_index,
                b.agent_id.as_str(),
            ))
        });

        Ok(reports)
    }

    pub fn delete_reports(&self, step_number: usize, round: usize) -> Result<()> {
        let step_dir = self.dir.join(format!("step_{step_number:03}"));
        let entries = match fs::read_dir(&step_dir) {
            Ok(iter) => iter,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(Error::Io {
                    path: step_dir,
                    source,
                });
            }
        };

        let prefix = format!("round_{round:02}_");
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                path: step_dir.clone(),
                source,
            })?;
            let path = entry.path();
            let is_matching_toml = path.extension().and_then(OsStr::to_str) == Some("toml")
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with(&prefix));
            if is_matching_toml {
                fs::remove_file(&path).map_err(|source| Error::Io { path, source })?;
            }
        }

        Ok(())
    }

    pub fn clear_step(&self, step_number: usize) -> Result<()> {
        let step_dir = self.dir.join(format!("step_{step_number:03}"));
        match fs::remove_dir_all(&step_dir) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(Error::Io {
                path: step_dir,
                source,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    use super::{
        AgentReport, CheckpointManager, EnginePhase, HookResult, MAX_SUMMARY_BYTES,
        OperatorDecision, ReconcileOutcome, ReportStatus, StampedReport, StepStatus,
        SubtaskAllocation, WorkflowCheckpoint, report_json_schema,
    };
    use crate::Error;
    use crate::config::{HookEvent, RoleConfig, RoleKind};
    use crate::plan::ImplementationPlan;

    fn role(kind: RoleKind) -> RoleConfig {
        let id = match kind {
            RoleKind::Orchestrator => "orchestrator",
            RoleKind::Implementer => "coder",
            RoleKind::Validator => "validator",
        };
        RoleConfig {
            role_id: id.to_string(),
            display_name: id.to_string(),
            kind,
            rules_file: format!("roles/{id}.md"),
            allowed_mcp_tools: Vec::new(),
        }
    }

    fn base_report(status: ReportStatus) -> AgentReport {
        let (target_stage, allocations) = if status == ReportStatus::Dispatch {
            (
                Some(1),
                vec![SubtaskAllocation {
                    target_agent_id: "coder-1".to_string(),
                    instructions: "implement feature".to_string(),
                    write_scope: vec!["src".to_string()],
                }],
            )
        } else {
            (None, Vec::new())
        };

        let feedback = if matches!(
            status,
            ReportStatus::Denied | ReportStatus::Blocked | ReportStatus::Escalate
        ) {
            Some("actionable feedback".to_string())
        } else {
            None
        };

        AgentReport {
            status,
            summary: "concise report summary".to_string(),
            target_stage,
            allocations,
            feedback,
            artifacts: Vec::new(),
        }
    }

    const ALL_STATUSES: [ReportStatus; 7] = [
        ReportStatus::Dispatch,
        ReportStatus::StepPassed,
        ReportStatus::Escalate,
        ReportStatus::Complete,
        ReportStatus::Blocked,
        ReportStatus::Confirmed,
        ReportStatus::Denied,
    ];

    #[test]
    fn orchestrator_statuses() {
        let r = role(RoleKind::Orchestrator);
        for status in ALL_STATUSES {
            let report = base_report(status);
            let res = report.validate_for_role(&r, 3);
            if matches!(
                status,
                ReportStatus::Dispatch | ReportStatus::StepPassed | ReportStatus::Escalate
            ) {
                assert!(res.is_ok(), "expected {status:?} to be allowed: {res:?}");
            } else {
                assert!(matches!(res, Err(Error::Report { .. })));
            }
        }
    }

    #[test]
    fn implementer_statuses() {
        let r = role(RoleKind::Implementer);
        for status in ALL_STATUSES {
            let report = base_report(status);
            let res = report.validate_for_role(&r, 3);
            if matches!(status, ReportStatus::Complete | ReportStatus::Blocked) {
                assert!(res.is_ok(), "expected {status:?} to be allowed: {res:?}");
            } else {
                assert!(matches!(res, Err(Error::Report { .. })));
            }
        }
    }

    #[test]
    fn validator_statuses() {
        let r = role(RoleKind::Validator);
        for status in ALL_STATUSES {
            let report = base_report(status);
            let res = report.validate_for_role(&r, 3);
            if matches!(status, ReportStatus::Confirmed | ReportStatus::Denied) {
                assert!(res.is_ok(), "expected {status:?} to be allowed: {res:?}");
            } else {
                assert!(matches!(res, Err(Error::Report { .. })));
            }
        }
    }

    #[test]
    fn summary_empty_or_too_long_rejected() {
        let r = role(RoleKind::Implementer);

        let mut blank = base_report(ReportStatus::Complete);
        blank.summary = "  ".to_string();
        assert!(matches!(
            blank.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut exact = base_report(ReportStatus::Complete);
        exact.summary = "a".repeat(MAX_SUMMARY_BYTES);
        assert!(exact.validate_for_role(&r, 3).is_ok());

        let mut too_long = base_report(ReportStatus::Complete);
        too_long.summary = "a".repeat(MAX_SUMMARY_BYTES + 1);
        assert!(matches!(
            too_long.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn dispatch_requires_target_and_allocations() {
        let r = role(RoleKind::Orchestrator);

        let mut missing_target = base_report(ReportStatus::Dispatch);
        missing_target.target_stage = None;
        assert!(matches!(
            missing_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut zero_target = base_report(ReportStatus::Dispatch);
        zero_target.target_stage = Some(0);
        assert!(matches!(
            zero_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut out_of_bounds_target = base_report(ReportStatus::Dispatch);
        out_of_bounds_target.target_stage = Some(3);
        assert!(matches!(
            out_of_bounds_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut empty_allocs = base_report(ReportStatus::Dispatch);
        empty_allocs.allocations.clear();
        assert!(matches!(
            empty_allocs.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut empty_agent = base_report(ReportStatus::Dispatch);
        empty_agent.allocations[0].target_agent_id = "  ".to_string();
        assert!(matches!(
            empty_agent.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut empty_instructions = base_report(ReportStatus::Dispatch);
        empty_instructions.allocations[0].instructions = "  ".to_string();
        assert!(matches!(
            empty_instructions.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn dispatch_rejects_bad_write_scope() {
        let r = role(RoleKind::Orchestrator);
        let mut report = base_report(ReportStatus::Dispatch);
        report.allocations[0].write_scope = vec!["../x".to_string()];
        assert!(matches!(
            report.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn non_dispatch_rejects_target_or_allocations() {
        let r = role(RoleKind::Orchestrator);

        let mut with_target = base_report(ReportStatus::StepPassed);
        with_target.target_stage = Some(1);
        assert!(matches!(
            with_target.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));

        let mut with_allocs = base_report(ReportStatus::StepPassed);
        with_allocs.allocations = vec![SubtaskAllocation {
            target_agent_id: "coder-1".to_string(),
            instructions: "work".to_string(),
            write_scope: Vec::new(),
        }];
        assert!(matches!(
            with_allocs.validate_for_role(&r, 3),
            Err(Error::Report { .. })
        ));
    }

    #[test]
    fn feedback_required_for_denied_blocked_escalate() {
        let cases = [
            (RoleKind::Validator, ReportStatus::Denied),
            (RoleKind::Implementer, ReportStatus::Blocked),
            (RoleKind::Orchestrator, ReportStatus::Escalate),
        ];

        for (kind, status) in cases {
            let r = role(kind);

            let mut missing = base_report(status);
            missing.feedback = None;
            assert!(matches!(
                missing.validate_for_role(&r, 3),
                Err(Error::Report { .. })
            ));

            let mut whitespace = base_report(status);
            whitespace.feedback = Some("   ".to_string());
            assert!(matches!(
                whitespace.validate_for_role(&r, 3),
                Err(Error::Report { .. })
            ));

            let valid = base_report(status);
            assert!(valid.validate_for_role(&r, 3).is_ok());
        }
    }

    #[test]
    fn unknown_field_rejected() {
        let bad_report_toml = r#"
status = "Complete"
summary = "done"
unexpected_key = "nope"
"#;
        assert!(toml::from_str::<AgentReport>(bad_report_toml).is_err());

        let bad_alloc_toml = r#"
status = "Dispatch"
summary = "dispatching"
target_stage = 1

[[allocations]]
target_agent_id = "coder-1"
instructions = "do work"
extra = 123
"#;
        assert!(toml::from_str::<AgentReport>(bad_alloc_toml).is_err());
    }

    #[test]
    fn stamped_report_toml_round_trip() {
        let stamped = StampedReport {
            report: AgentReport {
                status: ReportStatus::Dispatch,
                summary: "dispatching stage 1".to_string(),
                target_stage: Some(1),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "local-coder".to_string(),
                    instructions: "implement checkpoint module".to_string(),
                    write_scope: vec!["crates/orrery-core/src".to_string()],
                }],
                feedback: None,
                artifacts: vec!["crates/orrery-core/src/checkpoint.rs".to_string()],
            },
            step_number: 7,
            plan_step_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            round: 1,
            stage_index: 0,
            role_id: "orchestrator".to_string(),
            agent_id: "local-orch".to_string(),
            recorded_at: Utc.with_ymd_and_hms(2026, 11, 13, 18, 0, 0).unwrap(),
        };

        let encoded = toml::to_string(&stamped).unwrap();
        let decoded: StampedReport = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, stamped);
    }

    #[test]
    fn json_schema_required_matches_struct() {
        let schema = report_json_schema();
        assert_eq!(schema["type"], serde_json::json!("object"));
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
        assert_eq!(schema["required"], serde_json::json!(["status", "summary"]));

        let full_report = AgentReport {
            status: ReportStatus::Dispatch,
            summary: "full".to_string(),
            target_stage: Some(1),
            allocations: vec![SubtaskAllocation {
                target_agent_id: "agent-a".to_string(),
                instructions: "task".to_string(),
                write_scope: vec!["src".to_string()],
            }],
            feedback: Some("note".to_string()),
            artifacts: vec!["src/lib.rs".to_string()],
        };
        let serialized = serde_json::to_value(&full_report).unwrap();
        let struct_keys: BTreeSet<String> =
            serialized.as_object().unwrap().keys().cloned().collect();
        let schema_keys: BTreeSet<String> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(schema_keys, struct_keys);

        let status_enum = schema["properties"]["status"]["enum"].as_array().unwrap();
        assert_eq!(status_enum.len(), 7);

        let minimal = serde_json::json!({
            "status": "Complete",
            "summary": "all good"
        });
        let parsed: AgentReport = serde_json::from_value(minimal).unwrap();
        assert_eq!(parsed.status, ReportStatus::Complete);
        assert_eq!(parsed.summary, "all good");

        let missing_summary = serde_json::json!({
            "status": "Complete"
        });
        assert!(serde_json::from_value::<AgentReport>(missing_summary).is_err());
    }

    fn three_step_plan() -> ImplementationPlan {
        ImplementationPlan::parse(
            "## Step 1: alpha\nbody one\n## Step 2: beta\nbody two\n## Step 3: gamma\nbody three\n",
            "## Step ",
        )
        .unwrap()
    }

    fn make_stamped_report(
        step_number: usize,
        round: usize,
        stage_index: usize,
        agent_id: &str,
    ) -> StampedReport {
        StampedReport {
            step_number,
            plan_step_sha256: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                .to_string(),
            round,
            stage_index,
            role_id: "coder".to_string(),
            agent_id: agent_id.to_string(),
            recorded_at: Utc.with_ymd_and_hms(2026, 11, 13, 19, 0, 0).unwrap(),
            report: base_report(ReportStatus::Complete),
        }
    }

    #[test]
    fn new_checkpoint_all_pending() {
        let plan = three_step_plan();
        let cp = WorkflowCheckpoint::new(&plan);
        assert_eq!(cp.plan_sha256, plan.plan_sha256());
        assert_eq!(cp.step_hashes.len(), 3);
        assert_eq!(
            cp.step_statuses,
            vec![
                StepStatus::Pending,
                StepStatus::Pending,
                StepStatus::Pending
            ]
        );
        assert_eq!(cp.current_step_index, 0);
        assert_eq!(cp.round, 0);
        assert_eq!(cp.active_stage_index, None);
        assert_eq!(cp.retry_count, 0);
        assert_eq!(cp.steps_passed_this_run, 0);
        assert_eq!(cp.phase, EnginePhase::Idle);
        assert!(cp.hook_results.is_empty());
        assert!(cp.awaiting_reason.is_none());
        assert!(cp.operator_decision.is_none());
        assert!(!cp.run_id.is_nil());
    }

    #[test]
    fn state_save_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new(dir.path().join("checkpoints"));
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        cp.step_statuses[0] = StepStatus::Passed;
        cp.step_statuses[1] = StepStatus::AwaitingOperator;
        cp.current_step_index = 1;
        cp.round = 2;
        cp.active_stage_index = Some(1);
        cp.retry_count = 1;
        cp.steps_passed_this_run = 1;
        cp.phase = EnginePhase::AwaitingOperator;
        cp.awaiting_reason = Some("needs review".to_string());
        cp.operator_decision = Some(OperatorDecision::Reject {
            feedback: "update tests".to_string(),
        });
        cp.hook_results.push(HookResult {
            event: HookEvent::StepStart,
            step_number: 2,
            tool: "git:status".to_string(),
            success: true,
            output: "clean".to_string(),
            recorded_at: Utc.with_ymd_and_hms(2026, 11, 13, 19, 30, 0).unwrap(),
        });

        mgr.save_state(&cp).unwrap();
        let loaded = mgr.load_state().unwrap().expect("saved state must load");
        assert_eq!(loaded, cp);
    }

    #[test]
    fn load_state_missing_is_none() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new(dir.path().join("checkpoints"));
        assert_eq!(mgr.load_state().unwrap(), None);
    }

    #[test]
    fn report_path_format() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new(dir.path().join("checkpoints"));
        let report = make_stamped_report(1, 2, 1, "local-coder");
        let saved_path = mgr.save_report(&report).unwrap();
        assert!(
            saved_path.ends_with("step_001/round_02_stage_01_local-coder.toml"),
            "unexpected report path: {}",
            saved_path.display()
        );
        assert!(saved_path.exists());
    }

    #[test]
    fn load_reports_filters_and_sorts() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new(dir.path().join("checkpoints"));

        // Save out of order across 2 rounds and multiple stages/agents
        let r2_s1_b = make_stamped_report(1, 2, 1, "coder-b");
        let r1_s2_a = make_stamped_report(1, 1, 2, "validator-a");
        let r1_s1_a = make_stamped_report(1, 1, 1, "coder-a");

        mgr.save_report(&r2_s1_b).unwrap();
        mgr.save_report(&r1_s2_a).unwrap();
        mgr.save_report(&r1_s1_a).unwrap();

        let round_1 = mgr.load_reports(1, Some(1)).unwrap();
        assert_eq!(round_1, vec![r1_s1_a.clone(), r1_s2_a.clone()]);

        let all = mgr.load_reports(1, None).unwrap();
        assert_eq!(all, vec![r1_s1_a, r1_s2_a, r2_s1_b]);

        // Missing step directory returns empty Vec
        assert!(mgr.load_reports(99, None).unwrap().is_empty());
    }

    #[test]
    fn delete_reports_removes_round_only() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new(dir.path().join("checkpoints"));

        let r1 = make_stamped_report(1, 1, 1, "coder-a");
        let r2_a = make_stamped_report(1, 2, 1, "coder-a");
        let r2_b = make_stamped_report(1, 2, 2, "validator-a");

        mgr.save_report(&r1).unwrap();
        mgr.save_report(&r2_a).unwrap();
        mgr.save_report(&r2_b).unwrap();

        mgr.delete_reports(1, 2).unwrap();

        let remaining = mgr.load_reports(1, None).unwrap();
        assert_eq!(remaining, vec![r1]);

        // Deleting from a non-existent step directory is a no-op
        mgr.delete_reports(99, 1).unwrap();
    }

    #[test]
    fn clear_step_removes_reports() {
        let dir = TempDir::new().unwrap();
        let mgr = CheckpointManager::new(dir.path().join("checkpoints"));

        let r1 = make_stamped_report(2, 1, 1, "coder-a");
        let r2 = make_stamped_report(2, 2, 1, "coder-b");
        mgr.save_report(&r1).unwrap();
        mgr.save_report(&r2).unwrap();

        mgr.clear_step(2).unwrap();
        assert!(mgr.load_reports(2, None).unwrap().is_empty());
        assert!(mgr.clear_step(9).is_ok());
    }

    #[test]
    fn reconcile_unchanged() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        let outcome = cp.reconcile(&plan).unwrap();
        assert_eq!(outcome, ReconcileOutcome::Unchanged);
    }

    #[test]
    fn reconcile_accepts_later_edit() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        cp.current_step_index = 0;
        cp.step_statuses[0] = StepStatus::InProgress;
        let old_hash_step3 = cp.step_hashes[2].clone();
        let old_plan_hash = cp.plan_sha256.clone();

        let edited_plan = ImplementationPlan::parse(
            "## Step 1: alpha\nbody one\n## Step 2: beta\nbody two\n## Step 3: gamma\nedited body three\n",
            "## Step ",
        )
        .unwrap();

        let outcome = cp.reconcile(&edited_plan).unwrap();
        assert_eq!(
            outcome,
            ReconcileOutcome::Updated {
                changed_steps: vec![2],
                appended: 0,
                removed: 0,
            }
        );
        assert_ne!(cp.step_hashes[2], old_hash_step3);
        assert_eq!(cp.step_hashes[2], edited_plan.steps[2].sha256_hex);
        assert_ne!(cp.plan_sha256, old_plan_hash);
        assert_eq!(cp.plan_sha256, edited_plan.plan_sha256());
    }

    #[test]
    fn reconcile_accepts_appended_step() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        cp.step_statuses[0] = StepStatus::Passed;

        let appended_plan = ImplementationPlan::parse(
            "## Step 1: alpha\nbody one\n## Step 2: beta\nbody two\n## Step 3: gamma\nbody three\n## Step 4: delta\nbody four\n",
            "## Step ",
        )
        .unwrap();

        let outcome = cp.reconcile(&appended_plan).unwrap();
        assert_eq!(
            outcome,
            ReconcileOutcome::Updated {
                changed_steps: Vec::new(),
                appended: 1,
                removed: 0,
            }
        );
        assert_eq!(cp.step_hashes.len(), 4);
        assert_eq!(cp.step_statuses.len(), 4);
        assert_eq!(cp.step_statuses[3], StepStatus::Pending);
        assert_eq!(cp.plan_sha256, appended_plan.plan_sha256());
    }

    #[test]
    fn reconcile_rejects_passed_step_edit() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        cp.step_statuses[0] = StepStatus::Passed;
        cp.current_step_index = 1;

        let edited_step1 = ImplementationPlan::parse(
            "## Step 1: alpha\nchanged body one\n## Step 2: beta\nbody two\n## Step 3: gamma\nbody three\n",
            "## Step ",
        )
        .unwrap();

        let err = cp.reconcile(&edited_step1).unwrap_err();
        assert!(matches!(
            err,
            Error::Engine {
                step_number: 1,
                ref message,
            } if message.contains("orrery reset --step 1")
        ));
    }

    #[test]
    fn reconcile_rejects_current_in_progress_edit() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        cp.step_statuses[0] = StepStatus::Passed;
        cp.current_step_index = 1;
        cp.step_statuses[1] = StepStatus::InProgress;

        let edited_step2 = ImplementationPlan::parse(
            "## Step 1: alpha\nbody one\n## Step 2: beta\nchanged body two\n## Step 3: gamma\nbody three\n",
            "## Step ",
        )
        .unwrap();

        let err = cp.reconcile(&edited_step2).unwrap_err();
        assert!(matches!(
            err,
            Error::Engine {
                step_number: 2,
                ref message,
            } if message.contains("orrery reset --step 2")
        ));

        cp.step_statuses[1] = StepStatus::AwaitingOperator;
        let err_awaiting = cp.reconcile(&edited_step2).unwrap_err();
        assert!(matches!(err_awaiting, Error::Engine { step_number: 2, .. }));
    }

    #[test]
    fn reconcile_removed_pending_ok_removed_passed_err() {
        let plan = three_step_plan();
        let two_step_plan = ImplementationPlan::parse(
            "## Step 1: alpha\nbody one\n## Step 2: beta\nbody two\n",
            "## Step ",
        )
        .unwrap();

        // Removing a trailing Pending step succeeds
        let mut cp_ok = WorkflowCheckpoint::new(&plan);
        cp_ok.step_statuses[0] = StepStatus::Passed;
        let outcome = cp_ok.reconcile(&two_step_plan).unwrap();
        assert_eq!(
            outcome,
            ReconcileOutcome::Updated {
                changed_steps: Vec::new(),
                appended: 0,
                removed: 1,
            }
        );
        assert_eq!(cp_ok.step_hashes.len(), 2);
        assert_eq!(cp_ok.step_statuses.len(), 2);

        // Removing a trailing Passed step fails with Error::Engine
        let mut cp_err = WorkflowCheckpoint::new(&plan);
        cp_err.step_statuses[2] = StepStatus::Passed;
        let err = cp_err.reconcile(&two_step_plan).unwrap_err();
        assert!(matches!(err, Error::Engine { step_number: 3, .. }));
    }

    #[test]
    fn reset_from_marks_later_pending() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        cp.step_statuses = vec![StepStatus::Passed, StepStatus::Passed, StepStatus::Passed];
        cp.current_step_index = 2;
        cp.round = 3;
        cp.active_stage_index = Some(2);
        cp.retry_count = 2;
        cp.phase = EnginePhase::AwaitingOperator;
        cp.awaiting_reason = Some("reason".to_string());
        cp.operator_decision = Some(OperatorDecision::Approve);

        cp.reset_from(2).unwrap();
        assert_eq!(
            cp.step_statuses,
            vec![StepStatus::Passed, StepStatus::Pending, StepStatus::Pending]
        );
        assert_eq!(cp.current_step_index, 1);
        assert_eq!(cp.round, 0);
        assert_eq!(cp.active_stage_index, None);
        assert_eq!(cp.retry_count, 0);
        assert_eq!(cp.phase, EnginePhase::Idle);
        assert_eq!(cp.awaiting_reason, None);
        assert_eq!(cp.operator_decision, None);

        // Out-of-bounds step numbers fail
        assert!(matches!(
            cp.reset_from(0),
            Err(Error::Engine { step_number: 0, .. })
        ));
        assert!(matches!(
            cp.reset_from(4),
            Err(Error::Engine { step_number: 4, .. })
        ));
    }

    #[test]
    fn unknown_field_in_state_rejected() {
        let plan = three_step_plan();
        let cp = WorkflowCheckpoint::new(&plan);
        let mut encoded = toml::to_string(&cp).unwrap();
        encoded.push_str("\nextra_field = \"not_allowed\"\n");

        let dir = TempDir::new().unwrap();
        let cp_dir = dir.path().join("checkpoints");
        std::fs::create_dir_all(&cp_dir).unwrap();
        std::fs::write(cp_dir.join("workflow_state.toml"), encoded).unwrap();

        let mgr = CheckpointManager::new(cp_dir);
        let err = mgr.load_state().unwrap_err();
        assert!(matches!(err, Error::TomlDe { .. }));
    }

    #[test]
    fn hook_results_capped() {
        let plan = three_step_plan();
        let mut cp = WorkflowCheckpoint::new(&plan);
        for step_number in 1..=205 {
            cp.push_hook_result(HookResult {
                event: HookEvent::StepStart,
                step_number,
                tool: "git:status".to_string(),
                success: true,
                output: "ok".to_string(),
                recorded_at: Utc.with_ymd_and_hms(2026, 11, 13, 19, 30, 0).unwrap(),
            });
        }
        assert_eq!(cp.hook_results.len(), 200);
        assert_eq!(cp.hook_results[0].step_number, 6);
        assert_eq!(cp.hook_results[199].step_number, 205);
    }
}
