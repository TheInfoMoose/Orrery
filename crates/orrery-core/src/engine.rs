use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::checkpoint::{
    CheckpointManager, EnginePhase, OperatorDecision, ReportStatus, StepStatus,
};
use crate::config::{
    AgentConfig, ExecutionMode, McpServerConfig, ProjectConfig, RoleConfig, RoleKind,
};
use crate::docker::{ResourceManager, StageLease};
use crate::llm::AgentDriver;
use crate::mcp::McpGatekeeper;
use crate::plan::ImplementationPlan;
use crate::stream_log::LiveStreamHub;
use crate::{Error, Result};

const EVENT_CHANNEL_CAPACITY: usize = 256;
const OPERATOR_CHANNEL_CAPACITY: usize = 8;

pub struct DriverSet {
    pub chat: Arc<dyn AgentDriver>,
    pub cli: Arc<dyn AgentDriver>,
}

pub struct EngineContext {
    pub project_root: PathBuf,
    pub project: ProjectConfig,
    pub roles: Vec<RoleConfig>,
    pub role_rules: HashMap<String, String>,
    pub agents: Vec<AgentConfig>,
    pub servers: Vec<McpServerConfig>,
    pub plan: ImplementationPlan,
    pub checkpoints: CheckpointManager,
    pub stream: Arc<LiveStreamHub>,
    pub gatekeeper: Arc<McpGatekeeper>,
    pub resources: Arc<ResourceManager>,
    pub drivers: DriverSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunOptions {
    pub mode: Option<ExecutionMode>,
    pub cycles: Option<usize>,
    pub start_step: Option<usize>,
    pub wait_for_operator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    Completed { steps_passed: usize },
    NoPendingSteps,
    AwaitingOperator { step_number: usize, reason: String },
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    PhaseChanged(EnginePhase),
    StepStatusChanged {
        step_number: usize,
        status: StepStatus,
    },
    RoundStarted {
        step_number: usize,
        round: usize,
    },
    AwaitingOperator {
        step_number: usize,
        reason: String,
    },
    RunFinished(RunOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchKind {
    Forward,
    Retry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTurnRecord {
    pub stage_index: usize,
    pub kind: RoleKind,
    pub expected_agents: Vec<String>,
    pub statuses: Vec<(String, ReportStatus)>,
}

pub fn check_dispatch(
    target: usize,
    last_stage_run: Option<usize>,
    kinds: &[RoleKind],
) -> Result<DispatchKind> {
    if target == 0 {
        return Err(Error::Engine {
            step_number: 0,
            message: "cannot dispatch to orchestrator stage 0".to_string(),
        });
    }
    if target >= kinds.len() {
        return Err(Error::Engine {
            step_number: 0,
            message: format!(
                "target stage {target} is out of range for {} stages",
                kinds.len()
            ),
        });
    }
    if kinds[target] == RoleKind::Orchestrator {
        return Err(Error::Engine {
            step_number: 0,
            message: format!("cannot dispatch to orchestrator stage {target}"),
        });
    }

    let next = last_stage_run.map_or(1, |last| last + 1);
    if target == next {
        return Ok(DispatchKind::Forward);
    }

    if target < next {
        if kinds[target] != RoleKind::Validator {
            return Ok(DispatchKind::Retry);
        }
        return Err(Error::Engine {
            step_number: 0,
            message: format!("cannot retry directly to validator stage {target}"),
        });
    }

    Err(Error::Engine {
        step_number: 0,
        message: format!(
            "cannot dispatch to stage {target}; expected next stage {next} or an earlier non-validator stage"
        ),
    })
}

pub fn agents_pass_rule(
    history: &[StageTurnRecord],
    gate_ok: bool,
) -> std::result::Result<(), String> {
    let Some((val_idx, val_record)) = history
        .iter()
        .enumerate()
        .rev()
        .find(|(_, record)| record.kind == RoleKind::Validator)
    else {
        return Err("no validator stage has run in this step".to_string());
    };

    if val_record.expected_agents.is_empty() {
        return Err("validator stage has no expected agents".to_string());
    }

    for expected in &val_record.expected_agents {
        let Some((_, status)) = val_record.statuses.iter().find(|(id, _)| id == expected) else {
            return Err(format!("missing report from validator agent '{expected}'"));
        };
        if *status != ReportStatus::Confirmed {
            return Err(format!(
                "validator agent '{expected}' reported {status:?}, expected Confirmed"
            ));
        }
    }

    for (agent_id, status) in &val_record.statuses {
        if *status != ReportStatus::Confirmed {
            return Err(format!(
                "validator agent '{agent_id}' reported {status:?}, expected Confirmed"
            ));
        }
    }

    if history[val_idx + 1..]
        .iter()
        .any(|record| record.kind == RoleKind::Implementer)
    {
        return Err("an implementer stage ran after the last validator stage".to_string());
    }

    if !gate_ok {
        return Err("gate check failed".to_string());
    }

    Ok(())
}

pub struct WorkflowEngine {
    ctx: EngineContext,
    events: broadcast::Sender<EngineEvent>,
    operator_tx: mpsc::Sender<OperatorDecision>,
    operator_rx: tokio::sync::Mutex<mpsc::Receiver<OperatorDecision>>,
    current_lease: tokio::sync::Mutex<Option<StageLease>>,
}

impl WorkflowEngine {
    pub fn new(ctx: EngineContext) -> Result<Self> {
        ctx.project.validate_for_run(&ctx.roles, &ctx.agents)?;
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (operator_tx, operator_rx) = mpsc::channel(OPERATOR_CHANNEL_CAPACITY);
        let engine = Self {
            ctx,
            events,
            operator_tx,
            operator_rx: tokio::sync::Mutex::new(operator_rx),
            current_lease: tokio::sync::Mutex::new(None),
        };
        let _ = (&engine.ctx, &engine.operator_rx, &engine.current_lease);
        Ok(engine)
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub fn operator_sender(&self) -> tokio::sync::mpsc::Sender<OperatorDecision> {
        self.operator_tx.clone()
    }

    pub async fn run(&self, _opts: RunOptions, _cancel: CancellationToken) -> Result<RunOutcome> {
        Err(Error::Engine {
            step_number: 0,
            message: "not implemented until Step 21".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;

    use tokio_util::sync::CancellationToken;

    use super::{
        DispatchKind, DriverSet, EngineContext, RunOptions, StageTurnRecord, WorkflowEngine,
        agents_pass_rule, check_dispatch,
    };
    use crate::Error;
    use crate::checkpoint::{CheckpointManager, OperatorDecision, ReportStatus};
    use crate::config::{
        AgentBackend, AgentConfig, InferenceConfig, InferenceLifecycle, InferenceMode,
        ProjectConfig, RoleConfig, RoleKind, StageConfig,
    };
    use crate::docker::{MockContainerBackend, MockInferenceControl, ResourceManager};
    use crate::llm::{AgentDriver, MockAgentDriver};
    use crate::mcp::McpGatekeeper;
    use crate::plan::ImplementationPlan;
    use crate::security::SecretRedactor;
    use crate::stream_log::{LiveStreamConfig, LiveStreamHub};

    pub(super) fn test_context(dir: &Path, stages: &[(RoleKind, &[&str])]) -> EngineContext {
        let orrery_dir = dir.join(".orrery");
        let mut roles = Vec::new();
        let mut role_rules = HashMap::new();
        let mut stage_configs = Vec::new();
        let mut agents = Vec::new();

        for (idx, (kind, agent_ids)) in stages.iter().enumerate() {
            let base = match kind {
                RoleKind::Orchestrator => "orchestrator",
                RoleKind::Implementer => "implementer",
                RoleKind::Validator => "validator",
            };
            let role_id = if roles.iter().any(|r: &RoleConfig| r.role_id == base) {
                format!("{base}_{idx}")
            } else {
                base.to_string()
            };

            roles.push(RoleConfig {
                role_id: role_id.clone(),
                display_name: role_id.clone(),
                kind: *kind,
                rules_file: format!("roles/{role_id}.md"),
                allowed_mcp_tools: Vec::new(),
            });
            role_rules.insert(role_id.clone(), format!("Rules for {role_id}"));

            stage_configs.push(StageConfig {
                role_id,
                agent_ids: agent_ids.iter().map(|id| (*id).to_string()).collect(),
            });

            for &agent_id in *agent_ids {
                if !agents.iter().any(|a: &AgentConfig| a.agent_id == agent_id) {
                    agents.push(AgentConfig {
                        agent_id: agent_id.to_string(),
                        display_name: agent_id.to_string(),
                        backend: AgentBackend::LocalModel {
                            inference_id: "router".to_string(),
                            model: "test-model".to_string(),
                            temperature: 0.0,
                            max_context_tokens: 4096,
                        },
                    });
                }
            }
        }

        let inference = vec![InferenceConfig {
            inference_id: "router".to_string(),
            endpoint_url: "http://192.0.2.1:8080".to_string(),
            mode: InferenceMode::Router,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 30,
        }];

        let mut project = ProjectConfig::default_template("test-project");
        project.stages = stage_configs;
        project.inference = inference.clone();

        let plan = ImplementationPlan::parse(
            "## Step 1: First\nStep 1 body\n## Step 2: Second\nStep 2 body\n## Step 3: Third\nStep 3 body\n",
            "## Step ",
        )
        .expect("3-step test plan should parse");

        let checkpoints = CheckpointManager::new(orrery_dir.join("checkpoints"));
        let stream = Arc::new(
            LiveStreamHub::new(
                orrery_dir.join("logs"),
                LiveStreamConfig::default(),
                Arc::new(SecretRedactor::new()),
            )
            .expect("test stream hub should initialize"),
        );
        let gatekeeper = Arc::new(McpGatekeeper::new());
        let resources = Arc::new(ResourceManager::new(
            Arc::new(MockContainerBackend::new()),
            Arc::new(MockInferenceControl::new()),
            inference,
            project.max_concurrent_agents,
            dir.to_path_buf(),
        ));

        let mock_driver: Arc<dyn AgentDriver> = Arc::new(MockAgentDriver::new());
        let drivers = DriverSet {
            chat: Arc::clone(&mock_driver),
            cli: mock_driver,
        };

        EngineContext {
            project_root: dir.to_path_buf(),
            project,
            roles,
            role_rules,
            agents,
            servers: Vec::new(),
            plan,
            checkpoints,
            stream,
            gatekeeper,
            resources,
            drivers,
        }
    }

    #[test]
    fn dispatch_forward_from_start() {
        let kinds = [
            RoleKind::Orchestrator,
            RoleKind::Implementer,
            RoleKind::Validator,
        ];
        assert_eq!(
            check_dispatch(1, None, &kinds).unwrap(),
            DispatchKind::Forward
        );
        let err = check_dispatch(2, None, &kinds).unwrap_err();
        assert!(matches!(err, Error::Engine { step_number: 0, .. }));
    }

    #[test]
    fn dispatch_forward_next() {
        let kinds = [
            RoleKind::Orchestrator,
            RoleKind::Implementer,
            RoleKind::Validator,
        ];
        assert_eq!(
            check_dispatch(2, Some(1), &kinds).unwrap(),
            DispatchKind::Forward
        );
    }

    #[test]
    fn dispatch_retry_to_earlier_implementer() {
        let kinds = [
            RoleKind::Orchestrator,
            RoleKind::Implementer,
            RoleKind::Validator,
        ];
        assert_eq!(
            check_dispatch(1, Some(2), &kinds).unwrap(),
            DispatchKind::Retry
        );
    }

    #[test]
    fn dispatch_to_orchestrator_rejected() {
        let kinds = [
            RoleKind::Orchestrator,
            RoleKind::Implementer,
            RoleKind::Validator,
        ];
        let err_from_start = check_dispatch(0, None, &kinds).unwrap_err();
        assert!(matches!(
            err_from_start,
            Error::Engine { step_number: 0, .. }
        ));

        let err_after_stage = check_dispatch(0, Some(1), &kinds).unwrap_err();
        assert!(matches!(
            err_after_stage,
            Error::Engine { step_number: 0, .. }
        ));
    }

    #[test]
    fn dispatch_retry_to_validator_rejected() {
        let kinds = [
            RoleKind::Orchestrator,
            RoleKind::Implementer,
            RoleKind::Validator,
            RoleKind::Implementer,
        ];
        let err = check_dispatch(2, Some(2), &kinds).unwrap_err();
        assert!(matches!(err, Error::Engine { step_number: 0, .. }));

        let err_later = check_dispatch(2, Some(3), &kinds).unwrap_err();
        assert!(matches!(err_later, Error::Engine { step_number: 0, .. }));
    }

    #[test]
    fn dispatch_out_of_range_rejected() {
        let kinds = [
            RoleKind::Orchestrator,
            RoleKind::Implementer,
            RoleKind::Validator,
        ];
        let err_at_len = check_dispatch(3, Some(2), &kinds).unwrap_err();
        assert!(matches!(err_at_len, Error::Engine { step_number: 0, .. }));

        let err_beyond = check_dispatch(99, None, &kinds).unwrap_err();
        assert!(matches!(err_beyond, Error::Engine { step_number: 0, .. }));
    }

    #[test]
    fn pass_rule_all_validators_confirmed() {
        let history = vec![
            StageTurnRecord {
                stage_index: 1,
                kind: RoleKind::Implementer,
                expected_agents: vec!["coder".into()],
                statuses: vec![("coder".into(), ReportStatus::Complete)],
            },
            StageTurnRecord {
                stage_index: 2,
                kind: RoleKind::Validator,
                expected_agents: vec!["val-1".into(), "val-2".into()],
                statuses: vec![
                    ("val-1".into(), ReportStatus::Confirmed),
                    ("val-2".into(), ReportStatus::Confirmed),
                ],
            },
        ];
        assert!(agents_pass_rule(&history, true).is_ok());
    }

    #[test]
    fn pass_rule_one_denied_fails() {
        let history = vec![
            StageTurnRecord {
                stage_index: 1,
                kind: RoleKind::Implementer,
                expected_agents: vec!["coder".into()],
                statuses: vec![("coder".into(), ReportStatus::Complete)],
            },
            StageTurnRecord {
                stage_index: 2,
                kind: RoleKind::Validator,
                expected_agents: vec!["val-1".into(), "val-2".into()],
                statuses: vec![
                    ("val-1".into(), ReportStatus::Confirmed),
                    ("val-2".into(), ReportStatus::Denied),
                ],
            },
        ];
        let err = agents_pass_rule(&history, true).unwrap_err();
        assert!(err.contains("val-2"));
    }

    #[test]
    fn pass_rule_missing_validator_report_fails() {
        let history = vec![
            StageTurnRecord {
                stage_index: 1,
                kind: RoleKind::Implementer,
                expected_agents: vec!["coder".into()],
                statuses: vec![("coder".into(), ReportStatus::Complete)],
            },
            StageTurnRecord {
                stage_index: 2,
                kind: RoleKind::Validator,
                expected_agents: vec!["val-1".into(), "val-2".into()],
                statuses: vec![("val-1".into(), ReportStatus::Confirmed)],
            },
        ];
        let err = agents_pass_rule(&history, true).unwrap_err();
        assert!(err.contains("val-2"));
    }

    #[test]
    fn pass_rule_implementer_after_validator_fails() {
        let history = vec![
            StageTurnRecord {
                stage_index: 1,
                kind: RoleKind::Implementer,
                expected_agents: vec!["coder".into()],
                statuses: vec![("coder".into(), ReportStatus::Complete)],
            },
            StageTurnRecord {
                stage_index: 2,
                kind: RoleKind::Validator,
                expected_agents: vec!["val-1".into()],
                statuses: vec![("val-1".into(), ReportStatus::Confirmed)],
            },
            StageTurnRecord {
                stage_index: 1,
                kind: RoleKind::Implementer,
                expected_agents: vec!["coder".into()],
                statuses: vec![("coder".into(), ReportStatus::Complete)],
            },
        ];
        let err = agents_pass_rule(&history, true).unwrap_err();
        assert!(err.contains("implementer"));
    }

    #[test]
    fn pass_rule_gate_failed_fails() {
        let history = vec![
            StageTurnRecord {
                stage_index: 1,
                kind: RoleKind::Implementer,
                expected_agents: vec!["coder".into()],
                statuses: vec![("coder".into(), ReportStatus::Complete)],
            },
            StageTurnRecord {
                stage_index: 2,
                kind: RoleKind::Validator,
                expected_agents: vec!["val-1".into()],
                statuses: vec![("val-1".into(), ReportStatus::Confirmed)],
            },
        ];
        let err = agents_pass_rule(&history, false).unwrap_err();
        assert!(err.contains("gate"));
    }

    #[test]
    fn pass_rule_no_validator_stage_fails() {
        let history = vec![StageTurnRecord {
            stage_index: 1,
            kind: RoleKind::Implementer,
            expected_agents: vec!["coder".into()],
            statuses: vec![("coder".into(), ReportStatus::Complete)],
        }];
        let err = agents_pass_rule(&history, true).unwrap_err();
        assert!(err.contains("validator"));
    }

    #[tokio::test]
    async fn new_rejects_config_invalid_for_run() {
        let dir = tempfile::tempdir().unwrap();
        let bad_ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &[]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let err = match WorkflowEngine::new(bad_ctx) {
            Ok(_) => panic!("expected invalid config to fail"),
            Err(err) => err,
        };
        assert!(matches!(err, Error::Validation { .. }));

        let ok_ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let engine = WorkflowEngine::new(ok_ctx).unwrap();
        let _rx = engine.subscribe();
        let tx = engine.operator_sender();
        tx.send(OperatorDecision::Approve).await.unwrap();
        let stub_err = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(
            stub_err,
            Error::Engine {
                step_number: 0,
                ref message,
            } if message == "not implemented until Step 21"
        ));
    }
}
