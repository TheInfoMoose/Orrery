use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::checkpoint::{
    AgentReport, CheckpointManager, EnginePhase, HookResult, OperatorDecision, ReconcileOutcome,
    ReportStatus, StampedReport, StepStatus, SubtaskAllocation, WorkflowCheckpoint,
};
use crate::config::{
    AgentBackend, AgentConfig, ConfigStore, ExecutionMode, HookEvent, HookFailure, HooksConfig,
    McpServerConfig, McpTransportConfig, ProjectConfig, RoleConfig, RoleKind, ValidationMode,
    format_mapped_tool_name, validate_concrete_tool_ref,
};
use crate::docker::{
    BollardContainerBackend, ContainerBackend, DockerComposeCli, DockerInferenceControl,
    InferenceControl, ResourceManager, StageLease,
};
use crate::llm::{
    AgentDriver, AgentInvocation, CliContainerDriver, HttpChatDriver, StageSummary,
    build_orchestrator_prompt, build_system_prompt, build_user_prompt,
};
use crate::mcp::{
    McpConnector, McpGatekeeper, RmcpConnector, WriteScope, tool_result_is_error, tool_result_text,
};
use crate::plan::{ImplementationPlan, PlanStep};
use crate::security::{OsKeyringStore, SecretRedactor, SecretStore};
use crate::stream_log::{LiveStreamConfig, LiveStreamHub, StreamEvent, StreamEventKind};
use crate::{Error, Result};

const EVENT_CHANNEL_CAPACITY: usize = 256;
const OPERATOR_CHANNEL_CAPACITY: usize = 8;
const PASS_CHECK_AWAITING_REASON: &str = "awaiting operator approval for step pass";
const STEP_PASS_HOOK_AWAITING_REASON: &str = "hook step_pass failed";
const MAX_HOOK_OUTPUT_BYTES: usize = 4096;

pub struct DriverSet {
    pub chat: Arc<dyn AgentDriver>,
    pub cli: Arc<dyn AgentDriver>,
}

impl DriverSet {
    fn driver_for(&self, backend: &AgentBackend) -> &Arc<dyn AgentDriver> {
        match backend {
            AgentBackend::LocalModel { .. } | AgentBackend::CloudApi { .. } => &self.chat,
            AgentBackend::AgentCli { .. } => &self.cli,
        }
    }
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

enum StageTurnOutcome {
    Completed(StageTurnRecord),
    ProtocolViolation(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AwaitingContext {
    PassCheck,
    StepPassHook,
    HaltOrEscalation,
}

enum AwaitResolution {
    PassStep,
    ContinueOrchestrator,
    HaltRun(RunOutcome),
}

enum HookOutcome {
    Skipped,
    Completed,
    Halt(String),
}

enum StepPassOutcome {
    FinishedRun(RunOutcome),
    NextStep,
    ContinueRound,
}

fn hook_event_name(event: HookEvent) -> &'static str {
    match event {
        HookEvent::StepStart => "step_start",
        HookEvent::StepPass => "step_pass",
        HookEvent::StepRetry => "step_retry",
        HookEvent::Halt => "halt",
    }
}

fn build_hook_role(hooks: &HooksConfig) -> RoleConfig {
    let allowed_mcp_tools = [
        &hooks.on_step_start,
        &hooks.on_step_pass,
        &hooks.on_step_retry,
        &hooks.on_halt,
    ]
    .into_iter()
    .flatten()
    .map(|h| h.tool.clone())
    .collect();

    RoleConfig {
        role_id: "__orrery_hooks".to_string(),
        display_name: "__orrery_hooks".to_string(),
        kind: RoleKind::Orchestrator,
        rules_file: String::new(),
        allowed_mcp_tools,
    }
}

fn truncate_utf8(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

struct StepRunState {
    step_idx: usize,
    step_number: usize,
    notes: Vec<String>,
    history: Vec<StageTurnRecord>,
    last_stage_run: Option<usize>,
    consecutive_violations: usize,
    stage_ran_since_gate_failure: bool,
}

pub struct WorkflowEngine {
    ctx: EngineContext,
    events: broadcast::Sender<EngineEvent>,
    operator_tx: mpsc::Sender<OperatorDecision>,
    operator_rx: tokio::sync::Mutex<mpsc::Receiver<OperatorDecision>>,
    current_lease: tokio::sync::Mutex<Option<Arc<StageLease>>>,
}

macro_rules! race_cancel {
    ($self:expr, $cancel:expr, $state:expr, $step_idx:expr, $step:expr, $fut:expr) => {
        tokio::select! {
            biased;
            () = $cancel.cancelled() => {
                return $self.handle_cancel(&mut $state, $step_idx, $step).await;
            }
            res = $fut => res,
        }
    };
}

impl WorkflowEngine {
    pub fn new(ctx: EngineContext) -> Result<Self> {
        ctx.project.validate_for_run(&ctx.roles, &ctx.agents)?;
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (operator_tx, operator_rx) = mpsc::channel(OPERATOR_CHANNEL_CAPACITY);
        Ok(Self {
            ctx,
            events,
            operator_tx,
            operator_rx: tokio::sync::Mutex::new(operator_rx),
            current_lease: tokio::sync::Mutex::new(None),
        })
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub fn operator_sender(&self) -> tokio::sync::mpsc::Sender<OperatorDecision> {
        self.operator_tx.clone()
    }

    pub fn subscribe_stream(&self) -> tokio::sync::broadcast::Receiver<StreamEvent> {
        self.ctx.stream.subscribe()
    }

    pub async fn run(&self, opts: RunOptions, cancel: CancellationToken) -> Result<RunOutcome> {
        let result = self.run_steps(opts, cancel).await;
        if let Err(err) = self.ctx.resources.shutdown().await {
            self.ctx.stream.emit(StreamEvent::notice(format!(
                "inference shutdown failed: {err}"
            )))?;
        }
        result
    }

    async fn run_steps(&self, opts: RunOptions, cancel: CancellationToken) -> Result<RunOutcome> {
        let mode = opts.mode.unwrap_or(self.ctx.project.execution_mode);
        let cycles = opts.cycles.unwrap_or(self.ctx.project.cycle_count);
        let validation_mode = self.ctx.project.effective_validation_mode(&self.ctx.roles);
        let mut requested_start = opts.start_step;

        let mut state = match self.ctx.checkpoints.load_state()? {
            Some(mut loaded) => {
                if let ReconcileOutcome::Updated {
                    changed_steps,
                    appended,
                    removed,
                } = loaded.reconcile(&self.ctx.plan)?
                {
                    let changed_numbers: Vec<usize> = changed_steps
                        .iter()
                        .map(|&i| self.ctx.plan.steps.get(i).map_or(i + 1, |s| s.number))
                        .collect();
                    self.ctx.stream.emit(StreamEvent {
                        timestamp: Utc::now(),
                        step_number: None,
                        stage_index: None,
                        agent_id: None,
                        kind: StreamEventKind::SystemNotice,
                        text: format!(
                            "plan reconciled: changed steps {changed_numbers:?}, appended {appended}, removed {removed}"
                        ),
                    })?;
                    self.ctx.checkpoints.save_state(&loaded)?;
                }
                loaded
            }
            None => WorkflowCheckpoint::new(&self.ctx.plan),
        };

        if !state
            .step_statuses
            .iter()
            .any(|s| matches!(s, StepStatus::InProgress | StepStatus::AwaitingOperator))
        {
            state.steps_passed_this_run = 0;
        }

        let stage_summaries = self.build_stage_summaries()?;
        let stage_kinds: Vec<RoleKind> = stage_summaries.iter().map(|s| s.kind).collect();

        loop {
            let Some(step_idx) = self.pick_next_step_index(&state, requested_start.take())? else {
                let outcome = if state.steps_passed_this_run == 0 {
                    RunOutcome::NoPendingSteps
                } else {
                    RunOutcome::Completed {
                        steps_passed: state.steps_passed_this_run,
                    }
                };
                state.phase = EnginePhase::Done;
                state.active_stage_index = None;
                self.emit_event(EngineEvent::PhaseChanged(EnginePhase::Done));
                self.emit_event(EngineEvent::RunFinished(outcome.clone()));
                return Ok(outcome);
            };

            let step = &self.ctx.plan.steps[step_idx];
            state.current_step_index = step_idx;

            let mut step_state = if state.step_statuses[step_idx] == StepStatus::AwaitingOperator {
                let (history, last_stage_run, last_is_step_passed) =
                    self.reconstruct_step_history(step.number)?;
                let mut resumed = StepRunState {
                    step_idx,
                    step_number: step.number,
                    notes: Vec::new(),
                    history,
                    last_stage_run,
                    consecutive_violations: 0,
                    stage_ran_since_gate_failure: true,
                };
                let context =
                    if state.awaiting_reason.as_deref() == Some(STEP_PASS_HOOK_AWAITING_REASON) {
                        AwaitingContext::StepPassHook
                    } else if validation_mode == ValidationMode::GateAndOperator
                        && last_is_step_passed
                        && state.awaiting_reason.as_deref() == Some(PASS_CHECK_AWAITING_REASON)
                    {
                        AwaitingContext::PassCheck
                    } else {
                        AwaitingContext::HaltOrEscalation
                    };
                let resolution = match state.operator_decision.take() {
                    Some(decision) => {
                        self.apply_operator_decision(&mut state, &mut resumed, context, decision)?
                    }
                    None => {
                        let reason = state
                            .awaiting_reason
                            .clone()
                            .unwrap_or_else(|| "awaiting operator".to_string());
                        race_cancel!(
                            self,
                            cancel,
                            state,
                            step_idx,
                            step,
                            self.enter_awaiting_operator(
                                &mut state,
                                &mut resumed,
                                reason,
                                context,
                                opts.wait_for_operator,
                            )
                        )?
                    }
                };
                match resolution {
                    AwaitResolution::PassStep => {
                        match race_cancel!(
                            self,
                            cancel,
                            state,
                            step_idx,
                            step,
                            self.complete_step_pass(
                                &mut state,
                                &mut resumed,
                                step,
                                mode,
                                cycles,
                                opts.wait_for_operator,
                            )
                        )? {
                            StepPassOutcome::FinishedRun(outcome) => return Ok(outcome),
                            StepPassOutcome::NextStep => continue,
                            StepPassOutcome::ContinueRound => resumed,
                        }
                    }
                    AwaitResolution::ContinueOrchestrator => resumed,
                    AwaitResolution::HaltRun(outcome) => return Ok(outcome),
                }
            } else if state.step_statuses[step_idx] == StepStatus::InProgress {
                self.ctx
                    .checkpoints
                    .delete_reports(step.number, state.round)?;
                let (history, last_stage_run, _) = self.reconstruct_step_history(step.number)?;
                state.round = state.round.saturating_sub(1);
                state.active_stage_index = None;
                state.awaiting_reason = None;
                let mut resumed = StepRunState {
                    step_idx,
                    step_number: step.number,
                    notes: Vec::new(),
                    history,
                    last_stage_run,
                    consecutive_violations: 0,
                    stage_ran_since_gate_failure: true,
                };
                if let Some(decision) = state.operator_decision.take() {
                    let _ = self.apply_operator_decision(
                        &mut state,
                        &mut resumed,
                        AwaitingContext::HaltOrEscalation,
                        decision,
                    )?;
                }
                resumed
            } else {
                state.step_statuses[step_idx] = StepStatus::InProgress;
                state.round = 0;
                state.active_stage_index = None;
                state.retry_count = 0;
                state.awaiting_reason = None;
                self.emit_event(EngineEvent::StepStatusChanged {
                    step_number: step.number,
                    status: StepStatus::InProgress,
                });
                self.ctx.checkpoints.clear_step(step.number)?;
                let mut fresh = StepRunState {
                    step_idx,
                    step_number: step.number,
                    notes: Vec::new(),
                    history: Vec::new(),
                    last_stage_run: None,
                    consecutive_violations: 0,
                    stage_ran_since_gate_failure: true,
                };
                if let Some(decision) = state.operator_decision.take() {
                    let _ = self.apply_operator_decision(
                        &mut state,
                        &mut fresh,
                        AwaitingContext::HaltOrEscalation,
                        decision,
                    )?;
                }
                if let HookOutcome::Halt(reason) = race_cancel!(
                    self,
                    cancel,
                    state,
                    step_idx,
                    step,
                    self.run_hook(&mut state, step, HookEvent::StepStart)
                )? {
                    match race_cancel!(
                        self,
                        cancel,
                        state,
                        step_idx,
                        step,
                        self.enter_awaiting_operator(
                            &mut state,
                            &mut fresh,
                            reason,
                            AwaitingContext::HaltOrEscalation,
                            opts.wait_for_operator,
                        )
                    )? {
                        AwaitResolution::ContinueOrchestrator | AwaitResolution::PassStep => {}
                        AwaitResolution::HaltRun(outcome) => return Ok(outcome),
                    }
                }
                fresh
            };

            loop {
                state.round += 1;
                state.phase = EnginePhase::OrchestratorTurn;
                state.active_stage_index = Some(0);
                self.emit_event(EngineEvent::PhaseChanged(EnginePhase::OrchestratorTurn));
                self.emit_event(EngineEvent::RoundStarted {
                    step_number: step.number,
                    round: state.round,
                });
                self.ctx.checkpoints.save_state(&state)?;

                let turn_notes = std::mem::take(&mut step_state.notes);
                let orch_report = match race_cancel!(
                    self,
                    cancel,
                    state,
                    step_idx,
                    step,
                    self.run_orchestrator_turn(&state, step, &stage_summaries, &turn_notes)
                ) {
                    Ok(report) => report,
                    Err(Error::Report { message }) => {
                        if let Some(outcome) = race_cancel!(
                            self,
                            cancel,
                            state,
                            step_idx,
                            step,
                            self.record_violation(
                                &mut state,
                                &mut step_state,
                                &message,
                                opts.wait_for_operator,
                            )
                        )? {
                            return Ok(outcome);
                        }
                        continue;
                    }
                    Err(err) => return Err(err),
                };

                match orch_report.status {
                    ReportStatus::Dispatch => {
                        let Some(target_stage) = orch_report.target_stage else {
                            if let Some(outcome) = race_cancel!(
                                self,
                                cancel,
                                state,
                                step_idx,
                                step,
                                self.record_violation(
                                    &mut state,
                                    &mut step_state,
                                    "Dispatch report requires target_stage",
                                    opts.wait_for_operator,
                                )
                            )? {
                                return Ok(outcome);
                            }
                            continue;
                        };

                        let dispatch_kind = match check_dispatch(
                            target_stage,
                            step_state.last_stage_run,
                            &stage_kinds,
                        ) {
                            Ok(kind) => kind,
                            Err(Error::Engine { message, .. }) => {
                                if let Some(outcome) = race_cancel!(
                                    self,
                                    cancel,
                                    state,
                                    step_idx,
                                    step,
                                    self.record_violation(
                                        &mut state,
                                        &mut step_state,
                                        &message,
                                        opts.wait_for_operator,
                                    )
                                )? {
                                    return Ok(outcome);
                                }
                                continue;
                            }
                            Err(other) => return Err(other),
                        };

                        if let Err(msg) =
                            self.validate_stage_allocations(target_stage, &orch_report.allocations)
                        {
                            if let Some(outcome) = race_cancel!(
                                self,
                                cancel,
                                state,
                                step_idx,
                                step,
                                self.record_violation(
                                    &mut state,
                                    &mut step_state,
                                    &msg,
                                    opts.wait_for_operator,
                                )
                            )? {
                                return Ok(outcome);
                            }
                            continue;
                        }

                        if dispatch_kind == DispatchKind::Retry {
                            state.retry_count += 1;
                            if state.retry_count > self.ctx.project.max_step_retries {
                                let reason = format!(
                                    "max step retries ({}) exhausted",
                                    self.ctx.project.max_step_retries
                                );
                                match race_cancel!(
                                    self,
                                    cancel,
                                    state,
                                    step_idx,
                                    step,
                                    self.enter_awaiting_operator(
                                        &mut state,
                                        &mut step_state,
                                        reason,
                                        AwaitingContext::HaltOrEscalation,
                                        opts.wait_for_operator,
                                    )
                                )? {
                                    AwaitResolution::ContinueOrchestrator
                                    | AwaitResolution::PassStep => continue,
                                    AwaitResolution::HaltRun(outcome) => return Ok(outcome),
                                }
                            }

                            if let HookOutcome::Halt(reason) = race_cancel!(
                                self,
                                cancel,
                                state,
                                step_idx,
                                step,
                                self.run_hook(&mut state, step, HookEvent::StepRetry)
                            )? {
                                match race_cancel!(
                                    self,
                                    cancel,
                                    state,
                                    step_idx,
                                    step,
                                    self.enter_awaiting_operator(
                                        &mut state,
                                        &mut step_state,
                                        reason,
                                        AwaitingContext::HaltOrEscalation,
                                        opts.wait_for_operator,
                                    )
                                )? {
                                    AwaitResolution::ContinueOrchestrator
                                    | AwaitResolution::PassStep => continue,
                                    AwaitResolution::HaltRun(outcome) => return Ok(outcome),
                                }
                            }
                        }

                        state.phase = EnginePhase::StageTurn;
                        state.active_stage_index = Some(target_stage);
                        self.emit_event(EngineEvent::PhaseChanged(EnginePhase::StageTurn));

                        match race_cancel!(
                            self,
                            cancel,
                            state,
                            step_idx,
                            step,
                            self.run_stage_turn(
                                &state,
                                step,
                                target_stage,
                                &orch_report.allocations
                            )
                        )? {
                            StageTurnOutcome::Completed(record) => {
                                step_state.consecutive_violations = 0;
                                step_state.stage_ran_since_gate_failure = true;
                                step_state.last_stage_run = Some(target_stage);
                                step_state.history.push(record);
                                state.active_stage_index = None;
                                self.ctx.checkpoints.save_state(&state)?;
                            }
                            StageTurnOutcome::ProtocolViolation(msg) => {
                                if let Some(outcome) = race_cancel!(
                                    self,
                                    cancel,
                                    state,
                                    step_idx,
                                    step,
                                    self.record_violation(
                                        &mut state,
                                        &mut step_state,
                                        &msg,
                                        opts.wait_for_operator,
                                    )
                                )? {
                                    return Ok(outcome);
                                }
                                continue;
                            }
                        }
                    }
                    ReportStatus::Escalate => {
                        step_state.consecutive_violations = 0;
                        let reason = orch_report
                            .feedback
                            .unwrap_or_else(|| orch_report.summary.clone());
                        match race_cancel!(
                            self,
                            cancel,
                            state,
                            step_idx,
                            step,
                            self.enter_awaiting_operator(
                                &mut state,
                                &mut step_state,
                                reason,
                                AwaitingContext::HaltOrEscalation,
                                opts.wait_for_operator,
                            )
                        )? {
                            AwaitResolution::ContinueOrchestrator | AwaitResolution::PassStep => {
                                continue;
                            }
                            AwaitResolution::HaltRun(outcome) => return Ok(outcome),
                        }
                    }
                    ReportStatus::StepPassed => match validation_mode {
                        ValidationMode::Agents | ValidationMode::Auto => {
                            if let Err(reason) = agents_pass_rule(&step_state.history, true) {
                                if let Some(outcome) = race_cancel!(
                                    self,
                                    cancel,
                                    state,
                                    step_idx,
                                    step,
                                    self.record_violation(
                                        &mut state,
                                        &mut step_state,
                                        &reason,
                                        opts.wait_for_operator,
                                    )
                                )? {
                                    return Ok(outcome);
                                }
                                continue;
                            }

                            let (gate_ok, gate_note) = race_cancel!(
                                self,
                                cancel,
                                state,
                                step_idx,
                                step,
                                self.run_gates(&mut state, step)
                            )?;
                            match agents_pass_rule(&step_state.history, gate_ok) {
                                Ok(()) => {
                                    match race_cancel!(
                                        self,
                                        cancel,
                                        state,
                                        step_idx,
                                        step,
                                        self.complete_step_pass(
                                            &mut state,
                                            &mut step_state,
                                            step,
                                            mode,
                                            cycles,
                                            opts.wait_for_operator,
                                        )
                                    )? {
                                        StepPassOutcome::FinishedRun(outcome) => {
                                            return Ok(outcome);
                                        }
                                        StepPassOutcome::NextStep => break,
                                        StepPassOutcome::ContinueRound => continue,
                                    }
                                }
                                Err(reason) => {
                                    let note = match gate_note {
                                        Some(detail) => format!("{reason}: {detail}"),
                                        None => reason,
                                    };
                                    if !step_state.stage_ran_since_gate_failure {
                                        let msg = format!(
                                            "StepPassed again without new work after a failed gate: {note}"
                                        );
                                        if let Some(outcome) = race_cancel!(
                                            self,
                                            cancel,
                                            state,
                                            step_idx,
                                            step,
                                            self.record_violation(
                                                &mut state,
                                                &mut step_state,
                                                &msg,
                                                opts.wait_for_operator,
                                            )
                                        )? {
                                            return Ok(outcome);
                                        }
                                        continue;
                                    }
                                    step_state.stage_ran_since_gate_failure = false;
                                    step_state.notes.push(note);
                                    continue;
                                }
                            }
                        }
                        ValidationMode::GateAndOperator => {
                            let (gate_ok, gate_note) = race_cancel!(
                                self,
                                cancel,
                                state,
                                step_idx,
                                step,
                                self.run_gates(&mut state, step)
                            )?;
                            if !gate_ok {
                                let note =
                                    gate_note.unwrap_or_else(|| "gate check failed".to_string());
                                if !step_state.stage_ran_since_gate_failure {
                                    let msg = format!(
                                        "StepPassed again without new work after a failed gate: {note}"
                                    );
                                    if let Some(outcome) = race_cancel!(
                                        self,
                                        cancel,
                                        state,
                                        step_idx,
                                        step,
                                        self.record_violation(
                                            &mut state,
                                            &mut step_state,
                                            &msg,
                                            opts.wait_for_operator,
                                        )
                                    )? {
                                        return Ok(outcome);
                                    }
                                    continue;
                                }
                                step_state.stage_ran_since_gate_failure = false;
                                step_state.notes.push(note);
                                continue;
                            }

                            step_state.consecutive_violations = 0;
                            match race_cancel!(
                                self,
                                cancel,
                                state,
                                step_idx,
                                step,
                                self.enter_awaiting_operator(
                                    &mut state,
                                    &mut step_state,
                                    PASS_CHECK_AWAITING_REASON.to_string(),
                                    AwaitingContext::PassCheck,
                                    opts.wait_for_operator,
                                )
                            )? {
                                AwaitResolution::PassStep => {
                                    match race_cancel!(
                                        self,
                                        cancel,
                                        state,
                                        step_idx,
                                        step,
                                        self.complete_step_pass(
                                            &mut state,
                                            &mut step_state,
                                            step,
                                            mode,
                                            cycles,
                                            opts.wait_for_operator,
                                        )
                                    )? {
                                        StepPassOutcome::FinishedRun(outcome) => {
                                            return Ok(outcome);
                                        }
                                        StepPassOutcome::NextStep => break,
                                        StepPassOutcome::ContinueRound => continue,
                                    }
                                }
                                AwaitResolution::ContinueOrchestrator => continue,
                                AwaitResolution::HaltRun(outcome) => return Ok(outcome),
                            }
                        }
                    },
                    other => {
                        let msg = format!("unexpected orchestrator status {other:?}");
                        if let Some(outcome) = race_cancel!(
                            self,
                            cancel,
                            state,
                            step_idx,
                            step,
                            self.record_violation(
                                &mut state,
                                &mut step_state,
                                &msg,
                                opts.wait_for_operator,
                            )
                        )? {
                            return Ok(outcome);
                        }
                        continue;
                    }
                }
            }
        }
    }

    async fn handle_cancel(
        &self,
        state: &mut WorkflowCheckpoint,
        step_idx: usize,
        step: &PlanStep,
    ) -> Result<RunOutcome> {
        if let Some(lease_arc) = self.current_lease.lock().await.take() {
            match Arc::try_unwrap(lease_arc) {
                Ok(lease) => self.ctx.resources.end_stage(lease).await?,
                Err(_) => {
                    return Err(Error::Engine {
                        step_number: step.number,
                        message: "stage lease still shared on cancel".into(),
                    });
                }
            }
        } else {
            for id in self
                .ctx
                .project
                .stages
                .first()
                .map(|s| s.agent_ids.clone())
                .unwrap_or_default()
            {
                if matches!(self.find_agent(&id)?.backend, AgentBackend::AgentCli { .. }) {
                    self.ctx
                        .resources
                        .remove_cli_agent(&state.run_id, &id)
                        .await?;
                }
            }
        }

        if state.step_statuses[step_idx] == StepStatus::AwaitingOperator {
            state.phase = EnginePhase::AwaitingOperator;
            state.active_stage_index = None;
            self.ctx.checkpoints.save_state(state)?;
            self.emit_event(EngineEvent::RunFinished(RunOutcome::Interrupted));
            return Ok(RunOutcome::Interrupted);
        }

        state.step_statuses[step_idx] = StepStatus::InProgress;
        state.phase = EnginePhase::OrchestratorTurn;
        state.active_stage_index = None;
        state.awaiting_reason = None;
        self.ctx.checkpoints.save_state(state)?;

        self.run_halt_hook(state, step).await?;
        if state.phase != EnginePhase::OrchestratorTurn {
            state.phase = EnginePhase::OrchestratorTurn;
            self.ctx.checkpoints.save_state(state)?;
        }

        self.emit_event(EngineEvent::RunFinished(RunOutcome::Interrupted));
        Ok(RunOutcome::Interrupted)
    }

    fn emit_event(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

    async fn record_violation(
        &self,
        state: &mut WorkflowCheckpoint,
        step_state: &mut StepRunState,
        msg: &str,
        wait_for_operator: bool,
    ) -> Result<Option<RunOutcome>> {
        step_state.consecutive_violations += 1;
        let reason = format!("protocol violation: {msg}");
        if step_state.consecutive_violations < 2 {
            step_state.notes.push(reason);
            return Ok(None);
        }

        match self
            .enter_awaiting_operator(
                state,
                step_state,
                reason,
                AwaitingContext::HaltOrEscalation,
                wait_for_operator,
            )
            .await?
        {
            AwaitResolution::HaltRun(outcome) => Ok(Some(outcome)),
            AwaitResolution::ContinueOrchestrator | AwaitResolution::PassStep => Ok(None),
        }
    }

    async fn enter_awaiting_operator(
        &self,
        state: &mut WorkflowCheckpoint,
        step_state: &mut StepRunState,
        reason: String,
        context: AwaitingContext,
        wait_for_operator: bool,
    ) -> Result<AwaitResolution> {
        {
            let mut rx = self.operator_rx.lock().await;
            while rx.try_recv().is_ok() {}
        }

        let step = &self.ctx.plan.steps[step_state.step_idx];
        if state.step_statuses[step_state.step_idx] != StepStatus::AwaitingOperator {
            self.run_halt_hook(state, step).await?;
        }

        state.phase = EnginePhase::AwaitingOperator;
        state.step_statuses[step_state.step_idx] = StepStatus::AwaitingOperator;
        state.active_stage_index = None;
        state.awaiting_reason = Some(reason.clone());
        self.emit_event(EngineEvent::PhaseChanged(EnginePhase::AwaitingOperator));
        self.emit_event(EngineEvent::StepStatusChanged {
            step_number: step_state.step_number,
            status: StepStatus::AwaitingOperator,
        });
        self.emit_event(EngineEvent::AwaitingOperator {
            step_number: step_state.step_number,
            reason: reason.clone(),
        });
        self.ctx.checkpoints.save_state(state)?;

        if !wait_for_operator {
            let outcome = RunOutcome::AwaitingOperator {
                step_number: step_state.step_number,
                reason,
            };
            self.emit_event(EngineEvent::RunFinished(outcome.clone()));
            return Ok(AwaitResolution::HaltRun(outcome));
        }

        let decision = {
            let mut rx = self.operator_rx.lock().await;
            rx.recv().await.ok_or_else(|| Error::Engine {
                step_number: step_state.step_number,
                message: "operator channel closed while awaiting decision".to_string(),
            })?
        };

        self.apply_operator_decision(state, step_state, context, decision)
    }

    fn apply_operator_decision(
        &self,
        state: &mut WorkflowCheckpoint,
        step_state: &mut StepRunState,
        context: AwaitingContext,
        decision: OperatorDecision,
    ) -> Result<AwaitResolution> {
        state.operator_decision = None;
        state.awaiting_reason = None;
        step_state.consecutive_violations = 0;

        match (context, decision) {
            (
                AwaitingContext::PassCheck | AwaitingContext::StepPassHook,
                OperatorDecision::Approve,
            ) => Ok(AwaitResolution::PassStep),
            (AwaitingContext::PassCheck, OperatorDecision::Reject { feedback }) => {
                state.retry_count += 1;
                step_state.last_stage_run = None;
                step_state
                    .notes
                    .push(format!("operator rejected: {feedback}"));
                state.step_statuses[step_state.step_idx] = StepStatus::InProgress;
                self.emit_event(EngineEvent::StepStatusChanged {
                    step_number: step_state.step_number,
                    status: StepStatus::InProgress,
                });
                self.ctx.checkpoints.save_state(state)?;
                Ok(AwaitResolution::ContinueOrchestrator)
            }
            (AwaitingContext::HaltOrEscalation, OperatorDecision::Approve) => {
                state.retry_count = 0;
                state.step_statuses[step_state.step_idx] = StepStatus::InProgress;
                self.emit_event(EngineEvent::StepStatusChanged {
                    step_number: step_state.step_number,
                    status: StepStatus::InProgress,
                });
                self.ctx.checkpoints.save_state(state)?;
                Ok(AwaitResolution::ContinueOrchestrator)
            }
            (
                AwaitingContext::StepPassHook | AwaitingContext::HaltOrEscalation,
                OperatorDecision::Reject { feedback },
            ) => {
                state.retry_count = 0;
                step_state
                    .notes
                    .push(format!("operator rejected: {feedback}"));
                state.step_statuses[step_state.step_idx] = StepStatus::InProgress;
                self.emit_event(EngineEvent::StepStatusChanged {
                    step_number: step_state.step_number,
                    status: StepStatus::InProgress,
                });
                self.ctx.checkpoints.save_state(state)?;
                Ok(AwaitResolution::ContinueOrchestrator)
            }
        }
    }

    async fn complete_step_pass(
        &self,
        state: &mut WorkflowCheckpoint,
        step_state: &mut StepRunState,
        step: &PlanStep,
        mode: ExecutionMode,
        cycles: usize,
        wait_for_operator: bool,
    ) -> Result<StepPassOutcome> {
        loop {
            state.step_statuses[step_state.step_idx] = StepStatus::Passed;
            state.active_stage_index = None;
            state.awaiting_reason = None;
            self.ctx.checkpoints.save_state(state)?;
            self.emit_event(EngineEvent::StepStatusChanged {
                step_number: step.number,
                status: StepStatus::Passed,
            });

            if let HookOutcome::Halt(_) = self.run_hook(state, step, HookEvent::StepPass).await? {
                match self
                    .enter_awaiting_operator(
                        state,
                        step_state,
                        STEP_PASS_HOOK_AWAITING_REASON.to_string(),
                        AwaitingContext::StepPassHook,
                        wait_for_operator,
                    )
                    .await?
                {
                    AwaitResolution::PassStep => continue,
                    AwaitResolution::ContinueOrchestrator => {
                        return Ok(StepPassOutcome::ContinueRound);
                    }
                    AwaitResolution::HaltRun(outcome) => {
                        return Ok(StepPassOutcome::FinishedRun(outcome));
                    }
                }
            }

            state.steps_passed_this_run += 1;

            let should_stop = match mode {
                ExecutionMode::Run => true,
                ExecutionMode::Loop => {
                    (cycles > 0 && state.steps_passed_this_run >= cycles)
                        || !state.step_statuses.contains(&StepStatus::Pending)
                }
            };
            if should_stop {
                state.phase = EnginePhase::Done;
            }

            self.ctx.checkpoints.save_state(state)?;

            if should_stop {
                let outcome = RunOutcome::Completed {
                    steps_passed: state.steps_passed_this_run,
                };
                self.emit_event(EngineEvent::PhaseChanged(EnginePhase::Done));
                self.emit_event(EngineEvent::RunFinished(outcome.clone()));
                return Ok(StepPassOutcome::FinishedRun(outcome));
            }

            return Ok(StepPassOutcome::NextStep);
        }
    }

    async fn run_halt_hook(&self, state: &mut WorkflowCheckpoint, step: &PlanStep) -> Result<()> {
        let _ = self.run_hook(state, step, HookEvent::Halt).await?;
        Ok(())
    }

    async fn run_hook(
        &self,
        state: &mut WorkflowCheckpoint,
        step: &PlanStep,
        event: HookEvent,
    ) -> Result<HookOutcome> {
        let Some(hook_cfg) = self.ctx.project.hooks.get(event).cloned() else {
            return Ok(HookOutcome::Skipped);
        };

        state.phase = EnginePhase::Hook;
        state.active_stage_index = None;
        self.emit_event(EngineEvent::PhaseChanged(EnginePhase::Hook));

        let field_name = match event {
            HookEvent::StepStart => "hooks.on_step_start.tool",
            HookEvent::StepPass => "hooks.on_step_pass.tool",
            HookEvent::StepRetry => "hooks.on_step_retry.tool",
            HookEvent::Halt => "hooks.on_halt.tool",
        };
        let (server_id, tool) = validate_concrete_tool_ref(&hook_cfg.tool, field_name)?;
        let hook_role = build_hook_role(&self.ctx.project.hooks);
        let event_str = hook_event_name(event);

        let args = serde_json::json!({
            "orrery": {
                "project_name": self.ctx.project.project_name,
                "run_id": state.run_id.to_string(),
                "step_number": step.number,
                "step_title": step.title,
                "plan_step_sha256": step.sha256_hex,
                "event": event_str,
            }
        });

        let exec_res = self
            .ctx
            .gatekeeper
            .execute_for_role(&hook_role, &WriteScope::anywhere(), server_id, tool, args)
            .await;

        let (success, raw_output) = match exec_res {
            Ok(val) if !tool_result_is_error(&val) => (true, tool_result_text(&val)),
            Ok(val) => {
                let text = tool_result_text(&val);
                let detail = if text.is_empty() {
                    "tool returned isError=true".to_string()
                } else {
                    text
                };
                (false, detail)
            }
            Err(err) => {
                let detail = match std::error::Error::source(&err) {
                    Some(src) => format!("{err}: {src}"),
                    None => err.to_string(),
                };
                (false, detail)
            }
        };

        let redacted = self.ctx.stream.redactor().redact(&raw_output)?;
        let output = truncate_utf8(redacted, MAX_HOOK_OUTPUT_BYTES);

        state.push_hook_result(HookResult {
            event,
            step_number: step.number,
            tool: hook_cfg.tool.clone(),
            success,
            output: output.clone(),
            recorded_at: Utc::now(),
        });
        self.ctx.checkpoints.save_state(state)?;

        if success {
            return Ok(HookOutcome::Completed);
        }

        match hook_cfg.on_failure {
            HookFailure::Halt => Ok(HookOutcome::Halt(format!("hook {event_str} failed"))),
            HookFailure::Warn => {
                let notice_text = if output.is_empty() {
                    format!("hook {event_str} ({}) failed", hook_cfg.tool)
                } else {
                    format!("hook {event_str} ({}) failed: {output}", hook_cfg.tool)
                };
                self.ctx.stream.emit(StreamEvent {
                    timestamp: Utc::now(),
                    step_number: Some(step.number),
                    stage_index: None,
                    agent_id: None,
                    kind: StreamEventKind::SystemNotice,
                    text: notice_text,
                })?;
                Ok(HookOutcome::Completed)
            }
        }
    }

    async fn run_gates(
        &self,
        state: &mut WorkflowCheckpoint,
        step: &PlanStep,
    ) -> Result<(bool, Option<String>)> {
        if self.ctx.project.gate_tools.is_empty() {
            return Ok((true, None));
        }

        state.phase = EnginePhase::Gate;
        state.active_stage_index = None;
        self.emit_event(EngineEvent::PhaseChanged(EnginePhase::Gate));

        let gate_role = RoleConfig {
            role_id: "__orrery_gate".to_string(),
            display_name: "__orrery_gate".to_string(),
            kind: RoleKind::Validator,
            rules_file: String::new(),
            allowed_mcp_tools: self.ctx.project.gate_tools.clone(),
        };
        let args = serde_json::json!({
            "orrery": {
                "project_name": self.ctx.project.project_name,
                "run_id": state.run_id.to_string(),
                "step_number": step.number,
                "step_title": step.title,
                "plan_step_sha256": step.sha256_hex,
                "event": "gate",
            }
        });

        for (idx, tool_ref) in self.ctx.project.gate_tools.iter().enumerate() {
            let (server_id, tool) =
                validate_concrete_tool_ref(tool_ref, &format!("gate_tools[{idx}]"))?;
            let exec_res = self
                .ctx
                .gatekeeper
                .execute_for_role(
                    &gate_role,
                    &WriteScope::anywhere(),
                    server_id,
                    tool,
                    args.clone(),
                )
                .await;

            match exec_res {
                Ok(val) if !tool_result_is_error(&val) => {
                    let text = tool_result_text(&val);
                    let event_text = if text.is_empty() {
                        format!("{server_id}:{tool} passed")
                    } else {
                        format!("{server_id}:{tool} passed: {text}")
                    };
                    self.ctx.stream.emit(StreamEvent {
                        timestamp: Utc::now(),
                        step_number: Some(step.number),
                        stage_index: None,
                        agent_id: None,
                        kind: StreamEventKind::GateResult,
                        text: event_text,
                    })?;
                }
                Ok(val) => {
                    let text = tool_result_text(&val);
                    let detail = if text.is_empty() {
                        "tool returned isError=true".to_string()
                    } else {
                        text
                    };
                    let msg = format!("gate '{server_id}:{tool}' failed: {detail}");
                    self.ctx.stream.emit(StreamEvent {
                        timestamp: Utc::now(),
                        step_number: Some(step.number),
                        stage_index: None,
                        agent_id: None,
                        kind: StreamEventKind::GateResult,
                        text: msg.clone(),
                    })?;
                    return Ok((false, Some(msg)));
                }
                Err(err) => {
                    let detail = match std::error::Error::source(&err) {
                        Some(src) => format!("{err}: {src}"),
                        None => err.to_string(),
                    };
                    let msg = format!("gate '{server_id}:{tool}' failed: {detail}");
                    self.ctx.stream.emit(StreamEvent {
                        timestamp: Utc::now(),
                        step_number: Some(step.number),
                        stage_index: None,
                        agent_id: None,
                        kind: StreamEventKind::GateResult,
                        text: msg.clone(),
                    })?;
                    return Ok((false, Some(msg)));
                }
            }
        }

        Ok((true, None))
    }

    fn reconstruct_step_history(
        &self,
        step_number: usize,
    ) -> Result<(Vec<StageTurnRecord>, Option<usize>, bool)> {
        let reports = self.ctx.checkpoints.load_reports(step_number, None)?;
        let last_is_step_passed = reports
            .last()
            .is_some_and(|r| r.stage_index == 0 && r.report.status == ReportStatus::StepPassed);
        let mut history: Vec<StageTurnRecord> = Vec::new();
        let mut current_group: Option<(usize, StageTurnRecord)> = None;

        for stamped in reports {
            if stamped.stage_index == 0 {
                continue;
            }
            let same_group = current_group.as_ref().is_some_and(|(r, rec)| {
                *r == stamped.round && rec.stage_index == stamped.stage_index
            });
            if !same_group {
                if let Some((_, record)) = current_group.take() {
                    history.push(record);
                }
                let Some(stage_cfg) = self.ctx.project.stages.get(stamped.stage_index) else {
                    continue;
                };
                let role = self.find_role(&stage_cfg.role_id)?;
                current_group = Some((
                    stamped.round,
                    StageTurnRecord {
                        stage_index: stamped.stage_index,
                        kind: role.kind,
                        expected_agents: stage_cfg.agent_ids.clone(),
                        statuses: vec![(stamped.agent_id, stamped.report.status)],
                    },
                ));
            } else if let Some((_, ref mut record)) = current_group {
                record
                    .statuses
                    .push((stamped.agent_id, stamped.report.status));
            }
        }

        if let Some((_, record)) = current_group {
            history.push(record);
        }

        let last_stage_run = history.last().map(|r| r.stage_index);
        Ok((history, last_stage_run, last_is_step_passed))
    }

    fn pick_next_step_index(
        &self,
        state: &WorkflowCheckpoint,
        start_step: Option<usize>,
    ) -> Result<Option<usize>> {
        if let Some(step_number) = start_step {
            let Some(idx) = self
                .ctx
                .plan
                .steps
                .iter()
                .position(|s| s.number == step_number)
            else {
                return Err(Error::Engine {
                    step_number,
                    message: format!("step {step_number} not found in plan"),
                });
            };
            if state.step_statuses.get(idx) != Some(&StepStatus::Pending) {
                return Err(Error::Engine {
                    step_number,
                    message: format!("step {step_number} is not Pending"),
                });
            }
            return Ok(Some(idx));
        }

        if matches!(
            state.step_statuses.get(state.current_step_index),
            Some(StepStatus::InProgress | StepStatus::AwaitingOperator)
        ) {
            return Ok(Some(state.current_step_index));
        }
        if let Some(idx) = state.step_statuses.iter().position(|&status| {
            matches!(
                status,
                StepStatus::InProgress | StepStatus::AwaitingOperator
            )
        }) {
            return Ok(Some(idx));
        }

        Ok(state
            .step_statuses
            .iter()
            .position(|&status| status == StepStatus::Pending))
    }

    fn build_stage_summaries(&self) -> Result<Vec<StageSummary>> {
        let mut summaries = Vec::with_capacity(self.ctx.project.stages.len());
        for (index, stage) in self.ctx.project.stages.iter().enumerate() {
            let role = self.find_role(&stage.role_id)?;
            summaries.push(StageSummary {
                index,
                role_id: stage.role_id.clone(),
                kind: role.kind,
                agent_ids: stage.agent_ids.clone(),
            });
        }
        Ok(summaries)
    }

    fn find_role(&self, role_id: &str) -> Result<RoleConfig> {
        self.ctx
            .roles
            .iter()
            .find(|r| r.role_id == role_id)
            .cloned()
            .ok_or_else(|| Error::Validation {
                field: "role_id".to_string(),
                message: format!("unknown role_id '{role_id}'"),
            })
    }

    fn find_agent(&self, agent_id: &str) -> Result<AgentConfig> {
        self.ctx
            .agents
            .iter()
            .find(|a| a.agent_id == agent_id)
            .cloned()
            .ok_or_else(|| Error::Validation {
                field: "agent_id".to_string(),
                message: format!("unknown agent_id '{agent_id}'"),
            })
    }

    fn validate_stage_allocations(
        &self,
        target_stage: usize,
        allocations: &[SubtaskAllocation],
    ) -> std::result::Result<(), String> {
        if allocations.is_empty() {
            return Err("Dispatch report requires non-empty allocations".to_string());
        }
        let stage_cfg = &self.ctx.project.stages[target_stage];
        for (idx, alloc) in allocations.iter().enumerate() {
            if !stage_cfg.agent_ids.contains(&alloc.target_agent_id) {
                return Err(format!(
                    "target_agent_id '{}' is not in stage {target_stage}",
                    alloc.target_agent_id
                ));
            }
            if allocations[..idx]
                .iter()
                .any(|prior| prior.target_agent_id == alloc.target_agent_id)
            {
                let id = &alloc.target_agent_id;
                return Err(format!(
                    "agent '{id}' is allocated more than once in one stage turn"
                ));
            }
            if !alloc.write_scope.is_empty()
                && let Err(err) = WriteScope::new(alloc.write_scope.clone())
            {
                return Err(format!("invalid write_scope: {err}"));
            }
        }
        Ok(())
    }

    async fn run_orchestrator_turn(
        &self,
        state: &WorkflowCheckpoint,
        step: &PlanStep,
        stage_summaries: &[StageSummary],
        notes: &[String],
    ) -> Result<AgentReport> {
        let stage_cfg = &self.ctx.project.stages[0];
        let role = self.find_role(&stage_cfg.role_id)?;
        let agent = self.find_agent(&stage_cfg.agent_ids[0])?;
        let rules = self
            .ctx
            .role_rules
            .get(&role.role_id)
            .map(String::as_str)
            .unwrap_or_default();

        let prior = self.ctx.checkpoints.load_reports(step.number, None)?;
        let system_prompt = build_system_prompt(&role, rules, &agent.backend);
        let user_prompt = build_orchestrator_prompt(step, stage_summaries, &prior, notes);
        let tools = self.ctx.gatekeeper.tools_for_role(&role);

        let inv = AgentInvocation {
            run_id: state.run_id,
            step_number: step.number,
            stage_index: 0,
            stage_count: self.ctx.project.stages.len(),
            round: state.round,
            role: role.clone(),
            agent: agent.clone(),
            system_prompt,
            user_prompt,
            tools,
            write_scope: WriteScope::anywhere(),
        };

        let driver = self.ctx.drivers.driver_for(&agent.backend);
        let report = driver
            .run_turn(&inv, &self.ctx.gatekeeper, &self.ctx.stream)
            .await?;

        self.stamp_and_save_report(
            step,
            state.round,
            0,
            &role.role_id,
            &agent.agent_id,
            &report,
        )?;
        Ok(report)
    }

    async fn run_stage_turn(
        &self,
        state: &WorkflowCheckpoint,
        step: &PlanStep,
        stage_index: usize,
        allocations: &[SubtaskAllocation],
    ) -> Result<StageTurnOutcome> {
        let stage_cfg = &self.ctx.project.stages[stage_index];
        let role = self.find_role(&stage_cfg.role_id)?;
        let rules = self
            .ctx
            .role_rules
            .get(&role.role_id)
            .map(String::as_str)
            .unwrap_or_default();
        let tools = self.ctx.gatekeeper.tools_for_role(&role);

        let mut stage_agents = Vec::with_capacity(stage_cfg.agent_ids.len());
        for agent_id in &stage_cfg.agent_ids {
            stage_agents.push(self.find_agent(agent_id)?);
        }
        let writer = role.is_writer(&self.ctx.servers, &stage_agents);

        let effective_allocations: Vec<SubtaskAllocation> = if role.kind == RoleKind::Validator {
            let first_instructions = allocations
                .first()
                .map(|a| a.instructions.clone())
                .unwrap_or_default();
            stage_cfg
                .agent_ids
                .iter()
                .map(|agent_id| {
                    if let Some(existing) =
                        allocations.iter().find(|a| &a.target_agent_id == agent_id)
                    {
                        existing.clone()
                    } else {
                        SubtaskAllocation {
                            target_agent_id: agent_id.clone(),
                            instructions: first_instructions.clone(),
                            write_scope: Vec::new(),
                        }
                    }
                })
                .collect()
        } else {
            allocations.to_vec()
        };

        let prior = self.ctx.checkpoints.load_reports(step.number, None)?;

        let mut invocations = Vec::with_capacity(effective_allocations.len());
        for alloc in &effective_allocations {
            let agent = self.find_agent(&alloc.target_agent_id)?;
            let write_scope = if alloc.write_scope.is_empty() {
                WriteScope::anywhere()
            } else {
                WriteScope::new(alloc.write_scope.clone())?
            };
            let system_prompt = build_system_prompt(&role, rules, &agent.backend);
            let user_prompt = build_user_prompt(step, Some(alloc), &prior, &[]);
            invocations.push(AgentInvocation {
                run_id: state.run_id,
                step_number: step.number,
                stage_index,
                stage_count: self.ctx.project.stages.len(),
                round: state.round,
                role: role.clone(),
                agent,
                system_prompt,
                user_prompt,
                tools: tools.clone(),
                write_scope,
            });
        }

        let lease = Arc::new(
            self.ctx
                .resources
                .begin_stage(state.run_id, &stage_agents, writer, &effective_allocations)
                .await?,
        );
        *self.current_lease.lock().await = Some(Arc::clone(&lease));

        let turn_results = {
            let lease_ref = &lease;
            let futures = invocations.iter().map(|inv| async move {
                let permit = lease_ref.acquire_slot().await?;
                let driver = self.ctx.drivers.driver_for(&inv.agent.backend);
                let res = driver
                    .run_turn(inv, &self.ctx.gatekeeper, &self.ctx.stream)
                    .await;
                drop(permit);
                res.map(|report| (inv.agent.agent_id.clone(), report))
            });
            futures_util::future::join_all(futures).await
        };
        drop(lease);

        let mut statuses = Vec::with_capacity(turn_results.len());
        let mut turn_error: Option<Error> = None;

        for res in turn_results {
            match res {
                Ok((agent_id, report)) => {
                    if let Err(err) = self.stamp_and_save_report(
                        step,
                        state.round,
                        stage_index,
                        &role.role_id,
                        &agent_id,
                        &report,
                    ) {
                        turn_error = Some(err);
                        break;
                    }
                    statuses.push((agent_id, report.status));
                }
                Err(err) => {
                    turn_error = Some(err);
                    break;
                }
            }
        }

        if let Some(lease_arc) = self.current_lease.lock().await.take() {
            match Arc::try_unwrap(lease_arc) {
                Ok(lease) => self.ctx.resources.end_stage(lease).await?,
                Err(_) => {
                    return Err(Error::Engine {
                        step_number: step.number,
                        message: "stage lease still shared at end of stage turn".into(),
                    });
                }
            }
        }

        match turn_error {
            None => Ok(StageTurnOutcome::Completed(StageTurnRecord {
                stage_index,
                kind: role.kind,
                expected_agents: stage_cfg.agent_ids.clone(),
                statuses,
            })),
            Some(Error::Report { message }) => Ok(StageTurnOutcome::ProtocolViolation(message)),
            Some(err) => Err(err),
        }
    }

    fn stamp_and_save_report(
        &self,
        step: &PlanStep,
        round: usize,
        stage_index: usize,
        role_id: &str,
        agent_id: &str,
        report: &AgentReport,
    ) -> Result<()> {
        let recorded_at = Utc::now();
        let stamped = StampedReport {
            step_number: step.number,
            plan_step_sha256: step.sha256_hex.clone(),
            round,
            stage_index,
            role_id: role_id.to_string(),
            agent_id: agent_id.to_string(),
            recorded_at,
            report: report.clone(),
        };
        self.ctx.checkpoints.save_report(&stamped)?;
        self.ctx.stream.emit(StreamEvent {
            timestamp: recorded_at,
            step_number: Some(step.number),
            stage_index: Some(stage_index),
            agent_id: Some(agent_id.to_string()),
            kind: StreamEventKind::ReportSaved,
            text: format!("{:?}: {}", report.status, report.summary),
        })?;
        Ok(())
    }
}

pub struct Backends {
    pub secrets: Arc<dyn SecretStore>,
    pub containers: Arc<dyn ContainerBackend>,
    pub inference_control: Arc<dyn InferenceControl>,
    pub connector: Arc<dyn McpConnector>,
    pub drivers: DriverSet,
}

impl Backends {
    pub fn real(project_root: &Path) -> Result<Self> {
        let store = ConfigStore::new(project_root, None)?;
        let limits = store.load_project()?.limits;
        let inference = store.load_inference()?;
        let secrets: Arc<dyn SecretStore> = Arc::new(OsKeyringStore::new()?);
        let containers: Arc<dyn ContainerBackend> = Arc::new(BollardContainerBackend::connect()?);
        let inference_control: Arc<dyn InferenceControl> = Arc::new(DockerInferenceControl::new(
            Arc::clone(&containers),
            Arc::new(DockerComposeCli),
        ));
        let connector: Arc<dyn McpConnector> = Arc::new(RmcpConnector {
            secrets: Arc::clone(&secrets),
            project_root: store.project_root().to_path_buf(),
            search_path: std::env::var_os("PATH"),
            stream: std::sync::OnceLock::new(),
        });
        let drivers = DriverSet {
            chat: Arc::new(
                HttpChatDriver::new(Arc::clone(&secrets), inference)
                    .with_max_tool_calls(limits.max_tool_calls)
                    .with_idle_timeout(Duration::from_secs(limits.model_idle_timeout_secs)),
            ),
            cli: Arc::new(CliContainerDriver::new(
                Arc::clone(&containers),
                Arc::clone(&secrets),
                store.project_root().to_path_buf(),
            )),
        };
        Ok(Self {
            secrets,
            containers,
            inference_control,
            connector,
            drivers,
        })
    }
}

pub fn referenced_mcp_servers(project: &ProjectConfig, roles: &[RoleConfig]) -> BTreeSet<String> {
    let mut servers = BTreeSet::new();
    for role in roles {
        if project.stages.iter().any(|s| s.role_id == role.role_id) {
            for entry in &role.allowed_mcp_tools {
                if let Some((server_id, _)) = entry.split_once(':') {
                    servers.insert(server_id.to_string());
                }
            }
        }
    }
    for entry in &project.gate_tools {
        if let Some((server_id, _)) = entry.split_once(':') {
            servers.insert(server_id.to_string());
        }
    }
    for hook in [
        &project.hooks.on_step_start,
        &project.hooks.on_step_pass,
        &project.hooks.on_step_retry,
        &project.hooks.on_halt,
    ]
    .into_iter()
    .flatten()
    {
        if let Some((server_id, _)) = hook.tool.split_once(':') {
            servers.insert(server_id.to_string());
        }
    }
    servers
}

pub async fn build_engine(store: &ConfigStore, backends: Backends) -> Result<WorkflowEngine> {
    let project = store.load_project()?;
    let roles = store.load_roles()?;
    let mut role_rules = HashMap::with_capacity(roles.len());
    for role in &roles {
        let rules = store.load_role_rules(&role.role_id)?;
        role_rules.insert(role.role_id.clone(), rules);
    }
    let agents = store.load_agents()?;
    let servers = store.load_mcp_servers()?;
    let inference = store.load_inference()?;

    let plan_path = store.resolve_safe_path(&project.plan_path)?;
    let plan = ImplementationPlan::from_file(&plan_path, &project.step_marker)?;

    let referenced = referenced_mcp_servers(&project, &roles);
    let mut keychain_notices: Vec<String> = Vec::new();
    let redactor = Arc::new(SecretRedactor::new());
    for agent in &agents {
        let on_stage = project
            .stages
            .iter()
            .any(|s| s.agent_ids.contains(&agent.agent_id));
        let pair = match &agent.backend {
            AgentBackend::LocalModel { .. } => None,
            AgentBackend::AgentCli {
                keychain_service,
                keychain_account,
                ..
            } => keychain_service.as_deref().zip(keychain_account.as_deref()),
            AgentBackend::CloudApi {
                keychain_service,
                keychain_account,
                ..
            } => Some((keychain_service.as_str(), keychain_account.as_str())),
        };
        if let Some((service, account)) = pair {
            match backends.secrets.get(service, account) {
                Ok(secret) => redactor.register(&secret)?,
                Err(err) if on_stage => keychain_notices.push(format!(
                    "keychain entry {service}/{account} for agent '{}' could not be read ({err}); it will not be redacted",
                    agent.agent_id
                )),
                Err(_) => {}
            }
        }
    }
    for server in &servers {
        if !referenced.contains(&server.server_id) {
            continue;
        }
        if let McpTransportConfig::StreamableHttp {
            keychain_service: Some(service),
            keychain_account: Some(account),
            ..
        } = &server.transport
        {
            match backends.secrets.get(service, account) {
                Ok(secret) => redactor.register(&secret)?,
                Err(err) => keychain_notices.push(format!(
                    "keychain entry {service}/{account} for MCP server '{}' could not be read ({err}); it will not be redacted",
                    server.server_id
                )),
            }
        }
    }

    let orrery_dir = store.orrery_dir();
    let stream = Arc::new(LiveStreamHub::new(
        orrery_dir.join("logs"),
        LiveStreamConfig {
            ring_capacity: project.limits.live_stream_ring_capacity,
            max_file_bytes: project.limits.live_stream_max_file_bytes,
            max_files: project.limits.live_stream_max_files,
        },
        redactor,
    )?);
    for notice in keychain_notices {
        stream.emit(StreamEvent::notice(notice))?;
    }
    backends.connector.attach_stream(Arc::clone(&stream));

    let mut gatekeeper = McpGatekeeper::new();
    gatekeeper.set_project_root(store.project_root().to_path_buf());
    gatekeeper.set_call_timeout(std::time::Duration::from_secs(
        project.limits.mcp_call_timeout_secs,
    ));
    for server in &servers {
        if referenced.contains(&server.server_id) {
            let transport = backends.connector.connect(server).await?;
            gatekeeper.register(
                server.server_id.clone(),
                transport,
                server.write_tools.clone(),
            );
        } else {
            stream.emit(StreamEvent::notice(format!(
                "MCP server '{}' is not used by this workflow; not connected",
                server.server_id
            )))?;
        }
    }
    for role in &roles {
        for pattern in &role.allowed_mcp_tools {
            if let Some((server_id, _)) = pattern.split_once(':')
                && !servers.iter().any(|s| s.server_id == server_id)
            {
                return Err(Error::Validation {
                    field: "allowed_mcp_tools".to_string(),
                    message: format!(
                        "role '{}' references unknown MCP server '{server_id}'",
                        role.role_id
                    ),
                });
            }
        }
    }
    gatekeeper.refresh_catalog().await?;
    for tool in gatekeeper.catalog() {
        if format_mapped_tool_name(&tool.server_id, &tool.name).is_err() {
            stream.emit(StreamEvent::notice(format!(
                "MCP tool '{}:{}' is hidden: its name can't map to a function name",
                tool.server_id, tool.name
            )))?;
        }
    }

    let resources = Arc::new(ResourceManager::new(
        backends.containers,
        backends.inference_control,
        inference,
        project.max_concurrent_agents,
        store.project_root().to_path_buf(),
    ));
    let checkpoints = CheckpointManager::new(orrery_dir.join("checkpoints"));

    let ctx = EngineContext {
        project_root: store.project_root().to_path_buf(),
        project,
        roles,
        role_rules,
        agents,
        servers,
        plan,
        checkpoints,
        stream,
        gatekeeper: Arc::new(gatekeeper),
        resources,
        drivers: backends.drivers,
    };

    WorkflowEngine::new(ctx)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use tokio_util::sync::CancellationToken;

    use super::{
        Backends, DispatchKind, DriverSet, EngineContext, EngineEvent, RunOptions, RunOutcome,
        StageTurnRecord, WorkflowEngine, agents_pass_rule, build_engine, build_hook_role,
        check_dispatch,
    };
    use crate::Error;
    use crate::checkpoint::{
        AgentReport, CheckpointManager, EnginePhase, OperatorDecision, ReportStatus, StampedReport,
        StepStatus, SubtaskAllocation,
    };
    use crate::config::{
        AgentBackend, AgentConfig, ConfigStore, ExecutionMode, HookConfig, HookEvent, HookFailure,
        HooksConfig, InferenceConfig, InferenceLifecycle, InferenceMode, McpServerConfig,
        McpTransportConfig, ProjectConfig, RoleConfig, RoleKind, StageConfig, WorkspaceMount,
        WriteToolSpec,
    };
    use crate::docker::{
        MockCall, MockContainerBackend, MockInferenceControl, ResourceManager, cli_container_name,
    };
    use crate::llm::{AgentDriver, AgentInvocation, MockAgentDriver};
    use crate::mcp::{McpGatekeeper, MockMcpConnector, MockMcpTransport, WriteScope};
    use crate::plan::ImplementationPlan;
    use crate::security::{InMemorySecretStore, SecretRedactor, SecretStore, SecretValue};
    use crate::stream_log::{LiveStreamConfig, LiveStreamHub, StreamEvent, StreamEventKind};

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
                LiveStreamConfig {
                    ring_capacity: project.limits.live_stream_ring_capacity,
                    max_file_bytes: project.limits.live_stream_max_file_bytes,
                    max_files: project.limits.live_stream_max_files,
                },
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

    fn attach_mock_driver(ctx: &mut EngineContext) -> Arc<MockAgentDriver> {
        let mock = Arc::new(MockAgentDriver::new());
        let dyn_driver: Arc<dyn AgentDriver> = Arc::clone(&mock) as Arc<dyn AgentDriver>;
        ctx.drivers = DriverSet {
            chat: Arc::clone(&dyn_driver),
            cli: dyn_driver,
        };
        mock
    }

    fn dispatch_report(target_stage: usize, agent_id: &str, instructions: &str) -> AgentReport {
        AgentReport {
            status: ReportStatus::Dispatch,
            summary: format!("Dispatching to stage {target_stage} ({agent_id})"),
            target_stage: Some(target_stage),
            allocations: vec![SubtaskAllocation {
                target_agent_id: agent_id.to_string(),
                instructions: instructions.to_string(),
                write_scope: Vec::new(),
            }],
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn step_passed_report() -> AgentReport {
        AgentReport {
            status: ReportStatus::StepPassed,
            summary: "Step passed".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn complete_report() -> AgentReport {
        AgentReport {
            status: ReportStatus::Complete,
            summary: "Implementation complete".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: vec!["src/lib.rs".to_string()],
        }
    }

    fn confirmed_report() -> AgentReport {
        AgentReport {
            status: ReportStatus::Confirmed,
            summary: "Validation confirmed".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn denied_report(feedback: &str) -> AgentReport {
        AgentReport {
            status: ReportStatus::Denied,
            summary: "Validation denied".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: Some(feedback.to_string()),
            artifacts: Vec::new(),
        }
    }

    fn escalate_report(feedback: &str) -> AgentReport {
        AgentReport {
            status: ReportStatus::Escalate,
            summary: "Escalating to operator".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: Some(feedback.to_string()),
            artifacts: Vec::new(),
        }
    }

    fn script_passing_step(mock: &MockAgentDriver, orch: &str, imp: &str, val: &str) {
        mock.push(orch, Ok(dispatch_report(1, imp, "Implement step")));
        mock.push(imp, Ok(complete_report()));
        mock.push(orch, Ok(dispatch_report(2, val, "Validate step")));
        mock.push(val, Ok(confirmed_report()));
        mock.push(orch, Ok(step_passed_report()));
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
        let run_err = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(run_err, Error::Agent { .. }));
    }

    #[tokio::test]
    async fn run_mode_passes_one_step_and_stops() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let checkpoints = ctx.checkpoints.clone();
        let engine = WorkflowEngine::new(ctx).unwrap();
        let mut events_rx = engine.subscribe();

        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
        assert_eq!(state.step_statuses[1], StepStatus::Pending);
        assert_eq!(state.step_statuses[2], StepStatus::Pending);
        assert_eq!(state.steps_passed_this_run, 1);

        let mut saw_finished = false;
        while let Ok(ev) = events_rx.try_recv() {
            if ev == EngineEvent::RunFinished(RunOutcome::Completed { steps_passed: 1 }) {
                saw_finished = true;
            }
        }
        assert!(saw_finished);
    }

    #[tokio::test]
    async fn loop_cycles_three_passes_three() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.plan = ImplementationPlan::parse(
            "## Step 1: One\nBody 1\n## Step 2: Two\nBody 2\n## Step 3: Three\nBody 3\n## Step 4: Four\nBody 4\n",
            "## Step ",
        )
        .unwrap();

        let mock = attach_mock_driver(&mut ctx);
        for _ in 0..3 {
            script_passing_step(&mock, "orch", "imp", "val");
        }

        let checkpoints = ctx.checkpoints.clone();
        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    mode: Some(ExecutionMode::Loop),
                    cycles: Some(3),
                    start_step: None,
                    wait_for_operator: false,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 3 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(
            state.step_statuses,
            vec![
                StepStatus::Passed,
                StepStatus::Passed,
                StepStatus::Passed,
                StepStatus::Pending,
            ]
        );
        assert_eq!(state.steps_passed_this_run, 3);
    }

    #[tokio::test]
    async fn loop_cycles_zero_passes_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);
        for _ in 0..3 {
            script_passing_step(&mock, "orch", "imp", "val");
        }

        let checkpoints = ctx.checkpoints.clone();
        let engine = WorkflowEngine::new(ctx).unwrap();
        let opts = RunOptions {
            mode: Some(ExecutionMode::Loop),
            cycles: Some(0),
            start_step: None,
            wait_for_operator: false,
        };

        let outcome = engine
            .run(opts.clone(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 3 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(
            state.step_statuses,
            vec![StepStatus::Passed, StepStatus::Passed, StepStatus::Passed]
        );

        let second_outcome = engine.run(opts, CancellationToken::new()).await.unwrap();
        assert_eq!(second_outcome, RunOutcome::NoPendingSteps);
    }

    #[tokio::test]
    async fn orchestrator_runs_between_every_stage_turn() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let stage_indices: Vec<usize> = mock.invocations().iter().map(|i| i.stage_index).collect();
        assert_eq!(stage_indices, vec![0, 1, 0, 2, 0]);
        for (idx, &stage_idx) in stage_indices.iter().enumerate() {
            if idx % 2 == 0 {
                assert_eq!(stage_idx, 0);
            } else {
                assert_ne!(stage_idx, 0);
            }
        }
    }

    #[tokio::test]
    async fn step_passed_while_validator_denied_is_violation() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Initial work")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate work")));
        mock.push("val", Ok(denied_report("Missing edge-case test")));
        // Premature StepPassed while validator denied -> protocol violation
        mock.push("orch", Ok(step_passed_report()));
        // Next orchestrator turn gets protocol violation note and dispatches back to stage 1
        mock.push("orch", Ok(dispatch_report(1, "imp", "Fix edge-case test")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Re-validate")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let orch_prompts: Vec<String> = mock
            .invocations()
            .into_iter()
            .filter(|inv| inv.stage_index == 0)
            .map(|inv| inv.user_prompt)
            .collect();
        assert_eq!(orch_prompts.len(), 6);
        assert!(orch_prompts[3].contains("protocol violation"));
    }

    #[tokio::test]
    async fn dispatch_skipping_stage_is_violation() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);

        // Skipping stage 1 directly to stage 2 -> protocol violation
        mock.push("orch", Ok(dispatch_report(2, "val", "Skip implementer")));
        // Redo orchestrator turn properly
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let orch_prompts: Vec<String> = mock
            .invocations()
            .into_iter()
            .filter(|inv| inv.stage_index == 0)
            .map(|inv| inv.user_prompt)
            .collect();
        assert_eq!(orch_prompts.len(), 4);
        assert!(orch_prompts[1].contains("protocol violation"));

        // Second consecutive violation enters AwaitingOperator
        let dir2 = tempfile::tempdir().unwrap();
        let mut ctx2 = test_context(
            dir2.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock2 = attach_mock_driver(&mut ctx2);
        mock2.push("orch", Ok(dispatch_report(2, "val", "Skip 1")));
        mock2.push("orch", Ok(dispatch_report(2, "val", "Skip 2")));

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            outcome2,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));
    }

    #[tokio::test]
    async fn checkpoint_saved_after_every_round() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let checkpoints = ctx.checkpoints.clone();
        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let orch_turns = mock
            .invocations()
            .iter()
            .filter(|inv| inv.stage_index == 0)
            .count();
        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.round, orch_turns);
        assert_eq!(state.round, 3);
    }

    #[tokio::test]
    async fn reports_stamped_with_step_hash_and_round() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let step1_hash = ctx.plan.steps[0].sha256_hex.clone();
        let checkpoints = ctx.checkpoints.clone();
        let stream = Arc::clone(&ctx.stream);
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let reports = checkpoints.load_reports(1, None).unwrap();
        assert_eq!(reports.len(), 5);
        for stamped in &reports {
            assert_eq!(stamped.step_number, 1);
            assert_eq!(stamped.plan_step_sha256, step1_hash);
        }

        let summary: Vec<(usize, usize, &str)> = reports
            .iter()
            .map(|r| (r.round, r.stage_index, r.agent_id.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (1, 0, "orch"),
                (1, 1, "imp"),
                (2, 0, "orch"),
                (2, 2, "val"),
                (3, 0, "orch"),
            ]
        );

        let saved_events = stream
            .recent(50)
            .unwrap()
            .into_iter()
            .filter(|ev| ev.kind == StreamEventKind::ReportSaved)
            .count();
        assert_eq!(saved_events, 5);
    }

    #[tokio::test]
    async fn stage_lease_closed_after_run() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let resources = Arc::clone(&ctx.resources);
        let agents = ctx.agents.clone();
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let lease = resources
            .begin_stage(uuid::Uuid::new_v4(), &agents, false, &[])
            .await
            .expect("fresh begin_stage after run must succeed");
        resources.end_stage(lease).await.unwrap();
    }

    #[tokio::test]
    async fn protocol_violation_variants_and_cli_driver_routing() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );

        // Configure implementer as AgentCli to verify DriverSet routing to cli driver
        ctx.agents[1].backend = AgentBackend::AgentCli {
            image: "test-cli:1.0.0".to_string(),
            command: vec!["run".to_string()],
            workspace_mount: crate::config::WorkspaceMount::ReadOnly,
            memory_limit_mb: 512,
            timeout_secs: 30,
            keychain_service: None,
            keychain_account: None,
            secret_env: None,
        };

        let chat_mock = Arc::new(MockAgentDriver::new());
        let cli_mock = Arc::new(MockAgentDriver::new());
        ctx.drivers = DriverSet {
            chat: Arc::clone(&chat_mock) as Arc<dyn AgentDriver>,
            cli: Arc::clone(&cli_mock) as Arc<dyn AgentDriver>,
        };

        // Round 1: orchestrator returns Error::Report -> violation 1
        chat_mock.push(
            "orch",
            Err(Error::Report {
                message: "malformed JSON".to_string(),
            }),
        );
        // Round 2: orchestrator dispatches to stage 1 and stage 1 completes (resets consecutive_violations)
        chat_mock.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::Dispatch,
                summary: "Dispatch with write scope".to_string(),
                target_stage: Some(1),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "imp".to_string(),
                    instructions: "Implement".to_string(),
                    write_scope: vec!["src".to_string()],
                }],
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        cli_mock.push("imp", Ok(complete_report()));
        // Round 3: orchestrator dispatches to stage 2 with an unknown target_agent_id -> violation 1
        chat_mock.push("orch", Ok(dispatch_report(2, "ghost-agent", "Validate")));
        // Round 4: orchestrator dispatches to stage 2 and validator denies (resets consecutive_violations)
        chat_mock.push("orch", Ok(dispatch_report(2, "val", "Validate")));
        chat_mock.push("val", Ok(denied_report("Needs fix")));
        // Round 5: orchestrator retries stage 1, stage agent returns Error::Report -> violation 1
        chat_mock.push("orch", Ok(dispatch_report(1, "imp", "Retry implement")));
        cli_mock.push(
            "imp",
            Err(Error::Report {
                message: "invalid report.toml".to_string(),
            }),
        );
        // Round 6: orchestrator retries stage 1, stage agent succeeds (resets consecutive_violations)
        chat_mock.push("orch", Ok(dispatch_report(1, "imp", "Implement cleanly")));
        cli_mock.push("imp", Ok(complete_report()));
        // Round 7: orchestrator dispatches to stage 2, validator confirms
        chat_mock.push("orch", Ok(dispatch_report(2, "val", "Re-validate")));
        chat_mock.push("val", Ok(confirmed_report()));
        // Round 8: StepPassed
        chat_mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    mode: None,
                    cycles: None,
                    start_step: Some(1),
                    wait_for_operator: false,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });
        assert_eq!(cli_mock.invocations().len(), 3);
        assert_eq!(
            cli_mock.invocations()[0].write_scope.prefixes(),
            &["src".to_string()]
        );

        // Two consecutive stage Error::Report violations enter AwaitingOperator
        let dir2 = tempfile::tempdir().unwrap();
        let mut ctx2 = test_context(
            dir2.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock2 = attach_mock_driver(&mut ctx2);
        mock2.push("orch", Ok(dispatch_report(1, "imp", "Attempt 1")));
        mock2.push(
            "imp",
            Err(Error::Report {
                message: "bad report 1".to_string(),
            }),
        );
        mock2.push("orch", Ok(dispatch_report(1, "imp", "Attempt 2")));
        mock2.push(
            "imp",
            Err(Error::Report {
                message: "bad report 2".to_string(),
            }),
        );

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            outcome2,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));
    }

    #[tokio::test]
    async fn deny_revise_confirm_passes() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Initial implement")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Initial validate")));
        mock.push("val", Ok(denied_report("Missing test")));
        mock.push(
            "orch",
            Ok(dispatch_report(1, "imp", "Revise implementation")),
        );
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Re-validate")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
        assert_eq!(state.retry_count, 1);
    }

    #[tokio::test]
    async fn retries_exhausted_awaits_operator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.max_step_retries = 1;
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Attempt 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate 1")));
        mock.push("val", Ok(denied_report("Denial 1")));
        // Retry 1 (allowed, retry_count == 1 <= max_step_retries)
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate 2")));
        mock.push("val", Ok(denied_report("Denial 2")));
        // Retry 2 (exceeds max_step_retries == 1 -> AwaitingOperator)
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 2")));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);
        assert_eq!(state.phase, EnginePhase::AwaitingOperator);
        assert!(state.awaiting_reason.is_some());
        assert_eq!(state.retry_count, 2);
    }

    #[tokio::test]
    async fn escalate_awaits_operator_headless() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push(
            "orch",
            Ok(escalate_report("Requirement conflict in step 1")),
        );

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome,
            RunOutcome::AwaitingOperator {
                step_number: 1,
                reason: "Requirement conflict in step 1".to_string(),
            }
        );

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);
        assert_eq!(state.phase, EnginePhase::AwaitingOperator);
        assert_eq!(
            state.awaiting_reason.as_deref(),
            Some("Requirement conflict in step 1")
        );
    }

    #[tokio::test]
    async fn stored_approve_resumes_after_escalation() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        // First run: denial -> retry (retry_count = 1) -> Escalate -> AwaitingOperator
        mock.push("orch", Ok(dispatch_report(1, "imp", "Implement")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate")));
        mock.push("val", Ok(denied_report("Needs clarification")));
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(escalate_report("Need operator decision")));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let first_outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            first_outcome,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));

        let mut saved = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(saved.retry_count, 1);
        assert_eq!(saved.step_statuses[0], StepStatus::AwaitingOperator);
        saved.operator_decision = Some(OperatorDecision::Approve);
        checkpoints.save_state(&saved).unwrap();

        let invocations_before = mock.invocations().len();

        // Second run: stored Approve resets retry_count to 0 and runs orchestrator turn
        mock.push(
            "orch",
            Ok(dispatch_report(2, "val", "Validate after approve")),
        );
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let second_outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(second_outcome, RunOutcome::Completed { steps_passed: 1 });
        assert!(mock.invocations().len() > invocations_before);
        assert_eq!(mock.invocations()[invocations_before].stage_index, 0);

        let final_state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(final_state.retry_count, 0);
        assert_eq!(final_state.operator_decision, None);
        assert_eq!(final_state.step_statuses[0], StepStatus::Passed);

        // Also verify stored Reject after escalation resets retry_count to 0 and feeds feedback note,
        // and stored Approve in GateAndOperator mode passes the step on resume.
        let dir2 = tempfile::tempdir().unwrap();
        let mut ctx2 = test_context(
            dir2.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
            ],
        );
        let cp2 = ctx2.checkpoints.clone();
        let mock2 = attach_mock_driver(&mut ctx2);
        mock2.push("orch", Ok(escalate_report("Escalated in 2-stage")));

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let out_esc = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(out_esc, RunOutcome::AwaitingOperator { .. }));

        let mut st2 = cp2.load_state().unwrap().unwrap();
        st2.retry_count = 2;
        st2.operator_decision = Some(OperatorDecision::Reject {
            feedback: "try approach B".to_string(),
        });
        cp2.save_state(&st2).unwrap();

        mock2.push("orch", Ok(dispatch_report(1, "imp", "Implement B")));
        mock2.push("imp", Ok(complete_report()));
        mock2.push("orch", Ok(step_passed_report()));

        let out_gate = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(out_gate, RunOutcome::AwaitingOperator { .. }));
        assert!(
            mock2.invocations()[1]
                .user_prompt
                .contains("try approach B")
        );

        let mut st3 = cp2.load_state().unwrap().unwrap();
        assert_eq!(st3.retry_count, 0);
        st3.operator_decision = Some(OperatorDecision::Approve);
        cp2.save_state(&st3).unwrap();

        let out_done = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out_done, RunOutcome::Completed { steps_passed: 1 });

        // Verify that in Agents mode, escalating with PASS_CHECK_AWAITING_REASON after a validator
        // denial cannot spoof AwaitingContext::PassCheck on resume and still enforces validator confirmation.
        let dir3 = tempfile::tempdir().unwrap();
        let mut ctx3 = test_context(
            dir3.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let cp3 = ctx3.checkpoints.clone();
        let mock3 = attach_mock_driver(&mut ctx3);
        mock3.push("orch", Ok(dispatch_report(1, "imp", "Implement")));
        mock3.push("imp", Ok(complete_report()));
        mock3.push("orch", Ok(dispatch_report(2, "val", "Validate")));
        mock3.push("val", Ok(denied_report("Still broken")));
        mock3.push(
            "orch",
            Ok(escalate_report(super::PASS_CHECK_AWAITING_REASON)),
        );

        let engine3 = WorkflowEngine::new(ctx3).unwrap();
        let out3_esc = engine3
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(out3_esc, RunOutcome::AwaitingOperator { .. }));

        let mut st3_res = cp3.load_state().unwrap().unwrap();
        st3_res.operator_decision = Some(OperatorDecision::Approve);
        cp3.save_state(&st3_res).unwrap();

        // Premature StepPassed after resume is rejected because reconstructed history has Denied
        mock3.push("orch", Ok(step_passed_report()));
        mock3.push("orch", Ok(dispatch_report(1, "imp", "Fix after denial")));
        mock3.push("imp", Ok(complete_report()));
        mock3.push("orch", Ok(dispatch_report(2, "val", "Re-validate")));
        mock3.push("val", Ok(confirmed_report()));
        mock3.push("orch", Ok(step_passed_report()));

        let out3_done = engine3
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out3_done, RunOutcome::Completed { steps_passed: 1 });
        assert_eq!(cp3.load_state().unwrap().unwrap().retry_count, 1);
    }

    #[tokio::test]
    async fn gate_failure_blocks_pass() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.gate_tools = vec!["cargo:test".to_string(), "cargo:clippy".to_string()];

        let transport = Arc::new(
            MockMcpTransport::new("cargo")
                .with_failing_tool("test", "assertion failed in unit test")
                .with_tool(
                    "clippy",
                    serde_json::json!({ "content": [{ "type": "text", "text": "clean" }] }),
                ),
        );
        let mut gk = McpGatekeeper::new();
        gk.register("cargo", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let stream = Arc::clone(&ctx.stream);
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Implement")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate")));
        mock.push("val", Ok(confirmed_report()));
        // StepPassed triggers gate -> first gate tool fails -> stops at first failure and returns to orchestrator with gate note
        mock.push("orch", Ok(step_passed_report()));
        mock.push(
            "orch",
            Ok(escalate_report("Gate failed; awaiting operator")),
        );

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));

        // Stopped at the first failure: cargo:clippy was never called
        assert_eq!(transport.calls().len(), 1);
        assert_eq!(transport.calls()[0].0, "test");

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_ne!(state.step_statuses[0], StepStatus::Passed);

        let orch_prompts: Vec<String> = mock
            .invocations()
            .into_iter()
            .filter(|inv| inv.stage_index == 0)
            .map(|inv| inv.user_prompt)
            .collect();
        assert_eq!(orch_prompts.len(), 4);
        assert!(orch_prompts[3].contains("gate"));
        assert!(orch_prompts[3].contains("assertion failed in unit test"));

        let gate_events: Vec<_> = stream
            .recent(50)
            .unwrap()
            .into_iter()
            .filter(|ev| ev.kind == StreamEventKind::GateResult)
            .collect();
        assert_eq!(gate_events.len(), 1);
        assert!(gate_events[0].text.contains("cargo:test"));

        // Also verify isError=true gate result in GateAndOperator mode blocks pass and increments retry on Retry dispatch
        let dir2 = tempfile::tempdir().unwrap();
        let mut ctx2 = test_context(
            dir2.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
            ],
        );
        ctx2.project.gate_tools = vec!["cargo:lint".to_string()];
        let mut gk2 = McpGatekeeper::new();
        gk2.register(
            "cargo",
            Arc::new(MockMcpTransport::new("cargo").with_tool(
                "lint",
                serde_json::json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "unused variable" }]
                }),
            )),
            Vec::new(),
        );
        ctx2.gatekeeper = Arc::new(gk2);
        let cp2 = ctx2.checkpoints.clone();
        let mock2 = attach_mock_driver(&mut ctx2);
        mock2.push("orch", Ok(dispatch_report(1, "imp", "Implement")));
        mock2.push("imp", Ok(complete_report()));
        mock2.push("orch", Ok(step_passed_report()));
        mock2.push("orch", Ok(dispatch_report(1, "imp", "Fix lint")));
        mock2.push("imp", Ok(complete_report()));
        mock2.push("orch", Ok(escalate_report("Stop after retry")));

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let _ = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        let st2 = cp2.load_state().unwrap().unwrap();
        assert_eq!(st2.retry_count, 1);
    }

    #[tokio::test]
    async fn gate_and_operator_approve_passes() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
            ],
        );
        ctx.project.gate_tools = vec!["cargo:check".to_string()];

        let mut gk = McpGatekeeper::new();
        gk.register(
            "cargo",
            Arc::new(MockMcpTransport::new("cargo").with_tool(
                "check",
                serde_json::json!({
                    "content": [{ "type": "text", "text": "ok" }]
                }),
            )),
            Vec::new(),
        );
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Implement step")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = Arc::new(WorkflowEngine::new(ctx).unwrap());
        let mut events_rx = engine.subscribe();
        let op_tx = engine.operator_sender();
        let run_handle = {
            let engine = Arc::clone(&engine);
            tokio::spawn(async move {
                engine
                    .run(
                        RunOptions {
                            wait_for_operator: true,
                            ..Default::default()
                        },
                        CancellationToken::new(),
                    )
                    .await
            })
        };

        loop {
            if let EngineEvent::AwaitingOperator { .. } = events_rx.recv().await.unwrap() {
                break;
            }
        }
        op_tx.send(OperatorDecision::Approve).await.unwrap();

        let outcome = run_handle.await.unwrap().unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
    }

    #[tokio::test]
    async fn gate_and_operator_reject_feeds_feedback() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Initial work")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(step_passed_report()));
        // After operator Reject { feedback: "fix the README" }, orchestrator runs again
        mock.push("orch", Ok(dispatch_report(1, "imp", "Update README")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = Arc::new(WorkflowEngine::new(ctx).unwrap());
        let mut events_rx = engine.subscribe();
        let tx = engine.operator_sender();
        let run_handle = {
            let engine = Arc::clone(&engine);
            tokio::spawn(async move {
                engine
                    .run(
                        RunOptions {
                            wait_for_operator: true,
                            ..Default::default()
                        },
                        CancellationToken::new(),
                    )
                    .await
            })
        };

        loop {
            if let EngineEvent::AwaitingOperator { .. } = events_rx.recv().await.unwrap() {
                break;
            }
        }
        tx.send(OperatorDecision::Reject {
            feedback: "fix the README".to_string(),
        })
        .await
        .unwrap();

        loop {
            if let EngineEvent::AwaitingOperator { .. } = events_rx.recv().await.unwrap() {
                break;
            }
        }
        tx.send(OperatorDecision::Approve).await.unwrap();

        let outcome = run_handle.await.unwrap().unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let orch_prompts: Vec<String> = mock
            .invocations()
            .into_iter()
            .filter(|inv| inv.stage_index == 0)
            .map(|inv| inv.user_prompt)
            .collect();
        assert_eq!(orch_prompts.len(), 4);
        assert!(orch_prompts[2].contains("fix the README"));

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.retry_count, 1);
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
    }

    #[tokio::test]
    async fn protocol_violation_twice_awaits_operator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(2, "val", "Violation 1")));
        mock.push("orch", Ok(dispatch_report(2, "val", "Violation 2")));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        match outcome {
            RunOutcome::AwaitingOperator {
                step_number,
                reason,
            } => {
                assert_eq!(step_number, 1);
                assert!(reason.contains("protocol violation"));
            }
            other => panic!("expected AwaitingOperator, got {other:?}"),
        }

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);
        assert_eq!(state.phase, EnginePhase::AwaitingOperator);
    }

    #[tokio::test]
    async fn retry_without_denial_counts() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.max_step_retries = 1;
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Attempt 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 2")));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);
        assert_eq!(state.retry_count, 2);
    }

    #[tokio::test]
    async fn repeated_failed_gate_without_work_awaits_operator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.gate_tools = vec!["cargo:test".to_string()];

        let mut gk = McpGatekeeper::new();
        gk.register(
            "cargo",
            Arc::new(MockMcpTransport::new("cargo").with_failing_tool("test", "unit test failure")),
            Vec::new(),
        );
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Implement")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));
        mock.push("orch", Ok(step_passed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        match outcome {
            RunOutcome::AwaitingOperator {
                step_number,
                reason,
            } => {
                assert_eq!(step_number, 1);
                assert!(reason.contains("without new work"));
            }
            other => panic!("expected AwaitingOperator, got {other:?}"),
        }

        let orch_prompts: Vec<String> = mock
            .invocations()
            .into_iter()
            .filter(|inv| inv.stage_index == 0)
            .map(|inv| inv.user_prompt)
            .collect();
        assert_eq!(orch_prompts.len(), 5);
        assert!(orch_prompts[4].contains("without new work"));

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);

        // Also cover GateAndOperator repeated failed gate without new work
        let dir2 = tempfile::tempdir().unwrap();
        let mut ctx2 = test_context(
            dir2.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
            ],
        );
        ctx2.project.gate_tools = vec!["cargo:test".to_string()];
        let mut gk2 = McpGatekeeper::new();
        gk2.register(
            "cargo",
            Arc::new(MockMcpTransport::new("cargo").with_failing_tool("test", "unit test failure")),
            Vec::new(),
        );
        ctx2.gatekeeper = Arc::new(gk2);
        let mock2 = attach_mock_driver(&mut ctx2);
        mock2.push("orch", Ok(dispatch_report(1, "imp", "Implement")));
        mock2.push("imp", Ok(complete_report()));
        mock2.push("orch", Ok(step_passed_report()));
        mock2.push("orch", Ok(step_passed_report()));
        mock2.push("orch", Ok(step_passed_report()));

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(matches!(
            outcome2,
            RunOutcome::AwaitingOperator { step_number: 1, .. }
        ));
    }

    #[tokio::test]
    async fn hook_args_contain_orrery_object() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_step_start = Some(HookConfig {
            tool: "git:status".to_string(),
            on_failure: HookFailure::Halt,
        });

        let transport = Arc::new(MockMcpTransport::new("git").with_tool(
            "status",
            serde_json::json!({
                "content": [{ "type": "text", "text": "clean tree" }]
            }),
        ));
        let mut gk = McpGatekeeper::new();
        gk.register("git", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let expected_step_hash = ctx.plan.steps[0].sha256_hex.clone();
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "status");

        let orrery_obj = calls[0]
            .1
            .get("orrery")
            .and_then(|v| v.as_object())
            .unwrap();
        assert_eq!(orrery_obj.len(), 6);
        assert_eq!(
            orrery_obj.get("project_name").and_then(|v| v.as_str()),
            Some("test-project")
        );
        assert_eq!(
            orrery_obj.get("run_id").and_then(|v| v.as_str()),
            Some(state.run_id.to_string().as_str())
        );
        assert_eq!(
            orrery_obj.get("step_number").and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            orrery_obj.get("step_title").and_then(|v| v.as_str()),
            Some("1: First")
        );
        assert_eq!(
            orrery_obj.get("plan_step_sha256").and_then(|v| v.as_str()),
            Some(expected_step_hash.as_str())
        );
        assert_eq!(
            orrery_obj.get("event").and_then(|v| v.as_str()),
            Some("step_start")
        );

        assert_eq!(state.hook_results.len(), 1);
        assert_eq!(state.hook_results[0].event, HookEvent::StepStart);
        assert_eq!(state.hook_results[0].step_number, 1);
        assert_eq!(state.hook_results[0].tool, "git:status");
        assert!(state.hook_results[0].success);
        assert_eq!(state.hook_results[0].output, "clean tree");
    }

    #[tokio::test]
    async fn step_retry_hook_runs_before_each_retry() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_step_retry = Some(HookConfig {
            tool: "git:reset".to_string(),
            on_failure: HookFailure::Halt,
        });

        let transport = Arc::new(MockMcpTransport::new("git").with_tool(
            "reset",
            serde_json::json!({
                "content": [{ "type": "text", "text": "reset ok" }]
            }),
        ));
        let mut gk = McpGatekeeper::new();
        gk.register("git", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        // Initial pass attempt -> denied
        mock.push("orch", Ok(dispatch_report(1, "imp", "Attempt 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate 1")));
        mock.push("val", Ok(denied_report("Denial 1")));
        // Retry 1 -> denied
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 1")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate 2")));
        mock.push("val", Ok(denied_report("Denial 2")));
        // Retry 2 -> confirmed -> pass
        mock.push("orch", Ok(dispatch_report(1, "imp", "Retry 2")));
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Validate 3")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let mut events_rx = engine.subscribe();

        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let calls = transport.calls();
        assert_eq!(calls.len(), 2);
        for (name, args) in &calls {
            assert_eq!(name, "reset");
            assert_eq!(args["orrery"]["event"], "step_retry");
        }

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.hook_results.len(), 2);
        assert_eq!(state.hook_results[0].event, HookEvent::StepRetry);
        assert_eq!(state.hook_results[1].event, HookEvent::StepRetry);

        // Verify each retry hook ran before the retried stage's invocation
        let reports = checkpoints.load_reports(1, None).unwrap();
        let retry1_stage_report = reports
            .iter()
            .find(|r| r.round == 3 && r.stage_index == 1)
            .unwrap();
        let retry2_stage_report = reports
            .iter()
            .find(|r| r.round == 5 && r.stage_index == 1)
            .unwrap();
        assert!(state.hook_results[0].recorded_at <= retry1_stage_report.recorded_at);
        assert!(state.hook_results[1].recorded_at <= retry2_stage_report.recorded_at);

        let mut phases = Vec::new();
        while let Ok(ev) = events_rx.try_recv() {
            if let EngineEvent::PhaseChanged(phase) = ev {
                phases.push(phase);
            }
        }
        let hook_positions: Vec<usize> = phases
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| (p == EnginePhase::Hook).then_some(i))
            .collect();
        assert_eq!(hook_positions.len(), 2);
        for pos in hook_positions {
            assert_eq!(phases.get(pos + 1), Some(&EnginePhase::StageTurn));
        }
    }

    #[tokio::test]
    async fn step_pass_hook_runs_after_pass() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:tag".to_string(),
            on_failure: HookFailure::Halt,
        });

        let transport = Arc::new(MockMcpTransport::new("git").with_tool(
            "tag",
            serde_json::json!({
                "content": [{ "type": "text", "text": "tagged step-1" }]
            }),
        ));
        let mut gk = McpGatekeeper::new();
        gk.register("git", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let mut events_rx = engine.subscribe();

        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "tag");
        assert_eq!(calls[0].1["orrery"]["event"], "step_pass");

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
        assert_eq!(state.hook_results.len(), 1);
        assert_eq!(state.hook_results[0].event, HookEvent::StepPass);
        assert!(state.hook_results[0].success);

        let mut events = Vec::new();
        while let Ok(ev) = events_rx.try_recv() {
            events.push(ev);
        }
        let passed_idx = events
            .iter()
            .position(|ev| {
                *ev == EngineEvent::StepStatusChanged {
                    step_number: 1,
                    status: StepStatus::Passed,
                }
            })
            .expect("StepStatusChanged(Passed) event must be emitted");
        let hook_idx = events
            .iter()
            .position(|ev| *ev == EngineEvent::PhaseChanged(EnginePhase::Hook))
            .expect("PhaseChanged(Hook) event must be emitted");
        let done_idx = events
            .iter()
            .position(|ev| *ev == EngineEvent::PhaseChanged(EnginePhase::Done))
            .expect("PhaseChanged(Done) event must be emitted");
        assert!(passed_idx < hook_idx);
        assert!(hook_idx < done_idx);
    }

    #[tokio::test]
    async fn halt_hook_failure_awaits_operator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_step_start = Some(HookConfig {
            tool: "git:check".to_string(),
            on_failure: HookFailure::Halt,
        });
        ctx.project.hooks.on_halt = Some(HookConfig {
            tool: "git:notify_halt".to_string(),
            on_failure: HookFailure::Warn,
        });

        let transport = Arc::new(
            MockMcpTransport::new("git")
                .with_failing_tool("check", "uncommitted changes")
                .with_tool(
                    "notify_halt",
                    serde_json::json!({
                        "content": [{ "type": "text", "text": "halt notified" }]
                    }),
                ),
        );
        let mut gk = McpGatekeeper::new();
        gk.register("git", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome,
            RunOutcome::AwaitingOperator {
                step_number: 1,
                reason: "hook step_start failed".to_string(),
            }
        );
        assert!(mock.invocations().is_empty());

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);
        assert_eq!(state.phase, EnginePhase::AwaitingOperator);
        assert_eq!(
            state.awaiting_reason.as_deref(),
            Some("hook step_start failed")
        );
        assert_eq!(state.hook_results.len(), 2);
        assert_eq!(state.hook_results[0].event, HookEvent::StepStart);
        assert!(!state.hook_results[0].success);
        assert_eq!(state.hook_results[1].event, HookEvent::Halt);
        assert!(state.hook_results[1].success);

        // Also cover isError=true hook failure on step_retry and step_pass with HookFailure::Halt
        let dir2 = tempfile::tempdir().unwrap();
        let mut ctx2 = test_context(
            dir2.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx2.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        });
        let mut gk2 = McpGatekeeper::new();
        gk2.register(
            "git",
            Arc::new(MockMcpTransport::new("git").with_tool(
                "commit",
                serde_json::json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "nothing to commit" }]
                }),
            )),
            Vec::new(),
        );
        ctx2.gatekeeper = Arc::new(gk2);
        let mock2 = attach_mock_driver(&mut ctx2);
        script_passing_step(&mock2, "orch", "imp", "val");

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            outcome2,
            RunOutcome::AwaitingOperator {
                step_number: 1,
                reason: "hook step_pass failed".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn warn_hook_failure_continues_with_notice() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_step_start = Some(HookConfig {
            tool: "git:notify".to_string(),
            on_failure: HookFailure::Warn,
        });

        let mut gk = McpGatekeeper::new();
        gk.register(
            "git",
            Arc::new(MockMcpTransport::new("git").with_failing_tool("notify", "webhook offline")),
            Vec::new(),
        );
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let stream = Arc::clone(&ctx.stream);
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
        assert_eq!(state.hook_results.len(), 1);
        assert_eq!(state.hook_results[0].event, HookEvent::StepStart);
        assert!(!state.hook_results[0].success);
        assert!(state.hook_results[0].output.contains("webhook offline"));

        let notices: Vec<_> = stream
            .recent(50)
            .unwrap()
            .into_iter()
            .filter(|ev| ev.kind == StreamEventKind::SystemNotice)
            .collect();
        assert_eq!(notices.len(), 1);
        assert!(notices[0].text.contains("step_start"));
        assert!(notices[0].text.contains("webhook offline"));
    }

    #[tokio::test]
    async fn hook_role_can_call_only_hook_tools() {
        let mut hooks = HooksConfig::default();
        hooks.set(
            HookEvent::StepPass,
            HookConfig {
                tool: "git:commit".to_string(),
                on_failure: HookFailure::Halt,
            },
        );
        let role = build_hook_role(&hooks);
        assert_eq!(role.role_id, "__orrery_hooks");
        assert!(McpGatekeeper::is_tool_allowed(&role, "git", "commit"));
        assert!(!McpGatekeeper::is_tool_allowed(&role, "git", "push"));

        let transport = Arc::new(
            MockMcpTransport::new("git")
                .with_tool(
                    "commit",
                    serde_json::json!({ "content": [{ "type": "text", "text": "committed" }] }),
                )
                .with_tool(
                    "push",
                    serde_json::json!({ "content": [{ "type": "text", "text": "pushed" }] }),
                ),
        );
        let mut gk = McpGatekeeper::new();
        gk.register("git", Arc::clone(&transport) as _, Vec::new());

        let err = gk
            .execute_for_role(
                &role,
                &WriteScope::anywhere(),
                "git",
                "push",
                serde_json::json!({}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Security { .. }));
        assert!(transport.calls().is_empty());

        let ok = gk
            .execute_for_role(
                &role,
                &WriteScope::anywhere(),
                "git",
                "commit",
                serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(ok["content"][0]["text"], "committed");
        assert_eq!(transport.calls().len(), 1);
    }

    #[tokio::test]
    async fn hook_output_redacted_and_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );

        let secret = SecretValue::new("s3cr3t-hook-token-value-9999");
        ctx.stream.redactor().register(&secret).unwrap();

        // Build > 5,000 chars of output containing the registered secret and multi-byte UTF-8 chars
        let long_text = format!("token=s3cr3t-hook-token-value-9999; {}", "é".repeat(5_000));
        assert!(long_text.chars().count() > 5_000);

        ctx.project.hooks.on_step_start = Some(HookConfig {
            tool: "git:log".to_string(),
            on_failure: HookFailure::Halt,
        });

        let mut gk = McpGatekeeper::new();
        gk.register(
            "git",
            Arc::new(MockMcpTransport::new("git").with_tool(
                "log",
                serde_json::json!({
                    "content": [{ "type": "text", "text": long_text }]
                }),
            )),
            Vec::new(),
        );
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.hook_results.len(), 1);
        let stored_output = &state.hook_results[0].output;
        assert!(stored_output.contains("[REDACTED]"));
        assert!(!stored_output.contains("s3cr3t-hook-token-value-9999"));
        assert!(stored_output.len() <= 4096);
        assert!(stored_output.len() >= 4094);
    }

    fn set_max_concurrency(ctx: &mut EngineContext, max_concurrent_agents: usize) {
        ctx.project.max_concurrent_agents = max_concurrent_agents;
        ctx.resources = Arc::new(ResourceManager::new(
            Arc::new(MockContainerBackend::new()),
            Arc::new(MockInferenceControl::new()),
            ctx.project.inference.clone(),
            max_concurrent_agents,
            ctx.project_root.clone(),
        ));
    }

    fn mark_implementer_as_writer(ctx: &mut EngineContext) {
        ctx.servers.push(McpServerConfig {
            server_id: "fs".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "fs-server".to_string(),
                args: Vec::new(),
            },
            write_tools: vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        });
        if let Some(role) = ctx
            .roles
            .iter_mut()
            .find(|r| r.kind == RoleKind::Implementer)
        {
            role.allowed_mcp_tools.push("fs:write_file".to_string());
        }
    }

    fn dispatch_two_implementers(scope_a: Vec<String>, scope_b: Vec<String>) -> AgentReport {
        AgentReport {
            status: ReportStatus::Dispatch,
            summary: "Dispatching two implementers".to_string(),
            target_stage: Some(1),
            allocations: vec![
                SubtaskAllocation {
                    target_agent_id: "imp-1".to_string(),
                    instructions: "Implement part 1".to_string(),
                    write_scope: scope_a,
                },
                SubtaskAllocation {
                    target_agent_id: "imp-2".to_string(),
                    instructions: "Implement part 2".to_string(),
                    write_scope: scope_b,
                },
            ],
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    #[tokio::test]
    async fn non_writer_stage_runs_in_parallel() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp-1", "imp-2"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        set_max_concurrency(&mut ctx, 2);

        let mock = attach_mock_driver(&mut ctx);
        mock.set_delay(Duration::from_millis(50));

        mock.push(
            "orch",
            Ok(dispatch_two_implementers(Vec::new(), Vec::new())),
        );
        mock.push("imp-1", Ok(complete_report()));
        mock.push("imp-2", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Verify step 1")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });
        assert_eq!(mock.max_in_flight(), 2);
    }

    #[tokio::test]
    async fn writer_stage_serialized_without_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp-1", "imp-2"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        set_max_concurrency(&mut ctx, 2);
        mark_implementer_as_writer(&mut ctx);

        let mock = attach_mock_driver(&mut ctx);
        mock.set_delay(Duration::from_millis(50));

        mock.push(
            "orch",
            Ok(dispatch_two_implementers(Vec::new(), Vec::new())),
        );
        mock.push("imp-1", Ok(complete_report()));
        mock.push("imp-2", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Verify step 1")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });
        assert_eq!(mock.max_in_flight(), 1);

        let invocations = mock.invocations();
        let imp1_inv = invocations
            .iter()
            .find(|inv| inv.agent.agent_id == "imp-1")
            .unwrap();
        assert_eq!(imp1_inv.write_scope, WriteScope::anywhere());
    }

    #[tokio::test]
    async fn writer_stage_parallel_with_disjoint_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp-1", "imp-2"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        set_max_concurrency(&mut ctx, 2);
        mark_implementer_as_writer(&mut ctx);

        let mock = attach_mock_driver(&mut ctx);
        mock.set_delay(Duration::from_millis(50));

        mock.push(
            "orch",
            Ok(dispatch_two_implementers(
                vec!["src/a.rs".to_string()],
                vec!["src/b.rs".to_string()],
            )),
        );
        mock.push("imp-1", Ok(complete_report()));
        mock.push("imp-2", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Verify step 1")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });
        assert_eq!(mock.max_in_flight(), 2);

        let invocations = mock.invocations();
        let imp1_inv = invocations
            .iter()
            .find(|inv| inv.agent.agent_id == "imp-1")
            .unwrap();
        let imp2_inv = invocations
            .iter()
            .find(|inv| inv.agent.agent_id == "imp-2")
            .unwrap();
        assert_eq!(
            imp1_inv.write_scope,
            WriteScope::new(vec!["src/a.rs".to_string()]).unwrap()
        );
        assert_eq!(
            imp2_inv.write_scope,
            WriteScope::new(vec!["src/b.rs".to_string()]).unwrap()
        );
    }

    #[tokio::test]
    async fn writer_stage_serialized_with_overlapping_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp-1", "imp-2"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        set_max_concurrency(&mut ctx, 2);
        mark_implementer_as_writer(&mut ctx);

        let mock = attach_mock_driver(&mut ctx);
        mock.set_delay(Duration::from_millis(50));

        mock.push(
            "orch",
            Ok(dispatch_two_implementers(
                vec!["src".to_string()],
                vec!["src/lib.rs".to_string()],
            )),
        );
        mock.push("imp-1", Ok(complete_report()));
        mock.push("imp-2", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val", "Verify step 1")));
        mock.push("val", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });
        assert_eq!(mock.max_in_flight(), 1);
    }

    #[tokio::test]
    async fn validator_broadcast_runs_every_validator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val-1", "val-2"]),
            ],
        );

        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Implement step 1")));
        mock.push("imp", Ok(complete_report()));
        // Dispatch to validator stage with only 1 allocation (for val-1)
        mock.push(
            "orch",
            Ok(dispatch_report(2, "val-1", "Check broadcast instructions")),
        );
        mock.push("val-1", Ok(confirmed_report()));
        mock.push("val-2", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let invocations = mock.invocations();
        let val1_inv = invocations
            .iter()
            .find(|inv| inv.agent.agent_id == "val-1")
            .expect("val-1 should be invoked");
        let val2_inv = invocations
            .iter()
            .find(|inv| inv.agent.agent_id == "val-2")
            .expect("val-2 should be invoked via broadcast");
        assert!(
            val1_inv
                .user_prompt
                .contains("Check broadcast instructions")
        );
        assert!(
            val2_inv
                .user_prompt
                .contains("Check broadcast instructions")
        );
        assert_eq!(val1_inv.user_prompt, val2_inv.user_prompt);
    }

    #[tokio::test]
    async fn pass_requires_every_broadcast_validator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val-1", "val-2"]),
            ],
        );

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);

        mock.push("orch", Ok(dispatch_report(1, "imp", "Implement step 1")));
        mock.push("imp", Ok(complete_report()));
        // Only allocate val-1; val-2 runs via broadcast and denies
        mock.push("orch", Ok(dispatch_report(2, "val-1", "Validate step 1")));
        mock.push("val-1", Ok(confirmed_report()));
        mock.push("val-2", Ok(denied_report("val-2 rejected")));
        // Orchestrator tries to pass -> protocol violation because val-2 denied
        mock.push("orch", Ok(step_passed_report()));
        // Orchestrator retries implementer, then both validators confirm
        mock.push(
            "orch",
            Ok(dispatch_report(1, "imp", "Fix issue from val-2")),
        );
        mock.push("imp", Ok(complete_report()));
        mock.push("orch", Ok(dispatch_report(2, "val-1", "Re-validate")));
        mock.push("val-1", Ok(confirmed_report()));
        mock.push("val-2", Ok(confirmed_report()));
        mock.push("orch", Ok(step_passed_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::Passed);
        assert_eq!(state.retry_count, 1);

        let orch_invocations: Vec<_> = mock
            .invocations()
            .into_iter()
            .filter(|inv| inv.agent.agent_id == "orch")
            .collect();
        // Round 4 orchestrator turn (after premature StepPassed in Round 3) must contain the violation note
        assert!(
            orch_invocations[3]
                .user_prompt
                .contains("protocol violation")
        );
        assert!(orch_invocations[3].user_prompt.contains("val-2"));
    }

    #[tokio::test]
    async fn duplicate_allocation_is_violation() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );

        let mock = attach_mock_driver(&mut ctx);

        // Round 1: orchestrator dispatches two allocations targeting the same implementer "imp"
        mock.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::Dispatch,
                summary: "Duplicate allocations for imp".to_string(),
                target_stage: Some(1),
                allocations: vec![
                    SubtaskAllocation {
                        target_agent_id: "imp".to_string(),
                        instructions: "Task A".to_string(),
                        write_scope: Vec::new(),
                    },
                    SubtaskAllocation {
                        target_agent_id: "imp".to_string(),
                        instructions: "Task B".to_string(),
                        write_scope: Vec::new(),
                    },
                ],
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        // Round 2: orchestrator fixes its dispatch and completes the step
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let invocations = mock.invocations();
        // Invocations 0 and 1 are both "orch" (Round 1 and Round 2), proving "imp" had zero invocations in Round 1
        assert_eq!(invocations[0].agent.agent_id, "orch");
        assert_eq!(invocations[0].round, 1);
        assert_eq!(invocations[1].agent.agent_id, "orch");
        assert_eq!(invocations[1].round, 2);
        assert!(invocations[1].user_prompt.contains(
            "protocol violation: agent 'imp' is allocated more than once in one stage turn"
        ));
        assert_eq!(
            invocations
                .iter()
                .filter(|inv| inv.agent.agent_id == "imp" && inv.round == 1)
                .count(),
            0
        );
    }

    struct StageDelayedMockDriver {
        orch: Arc<MockAgentDriver>,
        stages: Arc<MockAgentDriver>,
    }

    #[async_trait::async_trait]
    impl AgentDriver for StageDelayedMockDriver {
        async fn run_turn(
            &self,
            inv: &AgentInvocation,
            mcp: &McpGatekeeper,
            stream: &LiveStreamHub,
        ) -> crate::Result<AgentReport> {
            if inv.stage_index == 0 {
                self.orch.run_turn(inv, mcp, stream).await
            } else {
                self.stages.run_turn(inv, mcp, stream).await
            }
        }
    }

    fn attach_stage_delayed_driver(
        ctx: &mut EngineContext,
        stage_delay: Duration,
    ) -> (Arc<MockAgentDriver>, Arc<MockAgentDriver>) {
        let orch = Arc::new(MockAgentDriver::new());
        let stages = Arc::new(MockAgentDriver::new());
        stages.set_delay(stage_delay);
        let combined: Arc<dyn AgentDriver> = Arc::new(StageDelayedMockDriver {
            orch: Arc::clone(&orch),
            stages: Arc::clone(&stages),
        });
        ctx.drivers = DriverSet {
            chat: Arc::clone(&combined),
            cli: combined,
        };
        (orch, stages)
    }

    #[tokio::test]
    async fn cancel_mid_stage_saves_checkpoint_and_closes_lease() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let resources = Arc::clone(&ctx.resources);
        let stage_agents = vec![ctx.agents[1].clone()];

        let (orch_mock, stage_mock) =
            attach_stage_delayed_driver(&mut ctx, Duration::from_millis(500));
        orch_mock.push("orch", Ok(dispatch_report(1, "imp", "Implement step 1")));
        stage_mock.push("imp", Ok(complete_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let cancel = CancellationToken::new();
        let cancel_trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_trigger.cancel();
        });

        let outcome = engine.run(RunOptions::default(), cancel).await.unwrap();
        assert_eq!(outcome, RunOutcome::Interrupted);

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::InProgress);
        assert_eq!(state.phase, EnginePhase::OrchestratorTurn);
        assert_eq!(state.round, 1);

        let new_lease = resources
            .begin_stage(state.run_id, &stage_agents, true, &[])
            .await
            .expect("new begin_stage must succeed after cancel closed the previous lease");
        resources.end_stage(new_lease).await.unwrap();
    }

    #[tokio::test]
    async fn cancel_runs_halt_hook() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_halt = Some(HookConfig {
            tool: "notify:halt".to_string(),
            on_failure: HookFailure::Warn,
        });

        let transport = Arc::new(MockMcpTransport::new("notify").with_tool(
            "halt",
            serde_json::json!({
                "content": [{ "type": "text", "text": "halted on cancel" }]
            }),
        ));
        let mut gk = McpGatekeeper::new();
        gk.register("notify", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let (orch_mock, stage_mock) =
            attach_stage_delayed_driver(&mut ctx, Duration::from_millis(500));
        orch_mock.push("orch", Ok(dispatch_report(1, "imp", "Implement step 1")));
        stage_mock.push("imp", Ok(complete_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let cancel = CancellationToken::new();
        let cancel_trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_trigger.cancel();
        });

        let outcome = engine.run(RunOptions::default(), cancel).await.unwrap();
        assert_eq!(outcome, RunOutcome::Interrupted);

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "halt");
        assert_eq!(calls[0].1["orrery"]["event"], "halt");
        assert_eq!(calls[0].1["orrery"]["step_number"], 1);

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::InProgress);
        assert_eq!(state.phase, EnginePhase::OrchestratorTurn);
        assert_eq!(state.hook_results.len(), 1);
        assert_eq!(state.hook_results[0].event, HookEvent::Halt);
        assert_eq!(state.hook_results[0].output, "halted on cancel");
    }

    #[tokio::test]
    async fn resume_restarts_interrupted_round_orchestrator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();

        let (orch_mock, stage_mock) =
            attach_stage_delayed_driver(&mut ctx, Duration::from_millis(500));
        orch_mock.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::Dispatch,
                summary: "Stale round 1 dispatch before interrupt".to_string(),
                target_stage: Some(1),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "imp".to_string(),
                    instructions: "Initial interrupted task".to_string(),
                    write_scope: Vec::new(),
                }],
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        stage_mock.push("imp", Ok(complete_report()));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let cancel = CancellationToken::new();
        let cancel_trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_trigger.cancel();
        });

        let first_outcome = engine.run(RunOptions::default(), cancel).await.unwrap();
        assert_eq!(first_outcome, RunOutcome::Interrupted);

        // Before resume, round 1's stale orchestrator report exists on disk
        let stale_reports = checkpoints.load_reports(1, Some(1)).unwrap();
        assert_eq!(stale_reports.len(), 1);
        assert_eq!(
            stale_reports[0].report.summary,
            "Stale round 1 dispatch before interrupt"
        );

        // Clear the delay and script a clean pass for the resumed run
        stage_mock.set_delay(Duration::ZERO);
        orch_mock.push(
            "orch",
            Ok(dispatch_report(1, "imp", "Fresh round 1 dispatch")),
        );
        stage_mock.push("imp", Ok(complete_report()));
        orch_mock.push("orch", Ok(dispatch_report(2, "val", "Validate step 1")));
        stage_mock.push("val", Ok(confirmed_report()));
        orch_mock.push("orch", Ok(step_passed_report()));

        let orch_count_before = orch_mock.invocations().len();
        assert_eq!(orch_count_before, 1);

        let second_outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(second_outcome, RunOutcome::Completed { steps_passed: 1 });

        let orch_invocations = orch_mock.invocations();
        let resumed_first = &orch_invocations[orch_count_before];
        assert_eq!(resumed_first.stage_index, 0);
        assert_eq!(resumed_first.agent.agent_id, "orch");
        assert_eq!(resumed_first.round, 1);
        assert!(
            !resumed_first
                .user_prompt
                .contains("Stale round 1 dispatch before interrupt")
        );

        let round1_reports = checkpoints.load_reports(1, Some(1)).unwrap();
        assert!(
            round1_reports
                .iter()
                .all(|r| r.report.summary != "Stale round 1 dispatch before interrupt")
        );
    }

    #[tokio::test]
    async fn edited_passed_step_blocks_start() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx1 = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock1 = attach_mock_driver(&mut ctx1);
        script_passing_step(&mock1, "orch", "imp", "val");

        let engine1 = WorkflowEngine::new(ctx1).unwrap();
        let outcome1 = engine1
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome1, RunOutcome::Completed { steps_passed: 1 });

        let mut ctx2 = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx2.plan = ImplementationPlan::parse(
            "## Step 1: First\nEdited step 1 body\n## Step 2: Second\nStep 2 body\n## Step 3: Third\nStep 3 body\n",
            "## Step ",
        )
        .unwrap();
        let mock2 = attach_mock_driver(&mut ctx2);

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let err = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap_err();
        match err {
            Error::Engine {
                step_number,
                message,
            } => {
                assert_eq!(step_number, 1);
                assert!(message.contains("orrery reset --step 1"));
            }
            other => panic!("expected Error::Engine for step 1, got {other:?}"),
        }
        assert!(mock2.invocations().is_empty());
    }

    #[tokio::test]
    async fn appended_step_accepted_on_start() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx1 = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock1 = attach_mock_driver(&mut ctx1);
        script_passing_step(&mock1, "orch", "imp", "val");

        let engine1 = WorkflowEngine::new(ctx1).unwrap();
        let outcome1 = engine1
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome1, RunOutcome::Completed { steps_passed: 1 });

        let mut ctx2 = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx2.plan = ImplementationPlan::parse(
            "## Step 1: First\nStep 1 body\n## Step 2: Second\nEdited step 2 body\n## Step 3: Third\nStep 3 body\n## Step 4: Fourth\nStep 4 body\n",
            "## Step ",
        )
        .unwrap();
        let checkpoints = ctx2.checkpoints.clone();
        let mock2 = attach_mock_driver(&mut ctx2);
        script_passing_step(&mock2, "orch", "imp", "val");

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome2, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses.len(), 4);
        assert_eq!(
            state.step_statuses,
            vec![
                StepStatus::Passed,
                StepStatus::Passed,
                StepStatus::Pending,
                StepStatus::Pending,
            ]
        );
    }

    #[tokio::test]
    async fn start_step_must_be_pending() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    start_step: Some(2),
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.current_step_index, 1);
        assert_eq!(
            state.step_statuses,
            vec![StepStatus::Pending, StepStatus::Passed, StepStatus::Pending]
        );

        let invocations_before = mock.invocations().len();
        let err = engine
            .run(
                RunOptions {
                    start_step: Some(2),
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        match err {
            Error::Engine { step_number, .. } => assert_eq!(step_number, 2),
            other => panic!("expected Error::Engine for non-Pending step 2, got {other:?}"),
        }
        assert_eq!(mock.invocations().len(), invocations_before);

        let missing_err = engine
            .run(
                RunOptions {
                    start_step: Some(99),
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        match missing_err {
            Error::Engine { step_number, .. } => assert_eq!(step_number, 99),
            other => panic!("expected Error::Engine for missing step 99, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_mid_cli_stage_removes_container() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );

        let imp_agent = ctx
            .agents
            .iter_mut()
            .find(|a| a.agent_id == "imp")
            .expect("stage-1 agent imp must exist");
        imp_agent.backend = AgentBackend::AgentCli {
            image: "alpine:3.20".to_string(),
            command: vec!["sh".to_string(), "-c".to_string(), "true".to_string()],
            workspace_mount: WorkspaceMount::ReadWrite,
            timeout_secs: 60,
            memory_limit_mb: 512,
            secret_env: None,
            keychain_service: None,
            keychain_account: None,
        };

        let container_backend = Arc::new(MockContainerBackend::new());
        ctx.resources = Arc::new(ResourceManager::new(
            Arc::clone(&container_backend) as _,
            Arc::new(MockInferenceControl::new()),
            ctx.project.inference.clone(),
            ctx.project.max_concurrent_agents,
            dir.path().to_path_buf(),
        ));

        let chat_mock = Arc::new(MockAgentDriver::new());
        chat_mock.push("orch", Ok(dispatch_report(1, "imp", "Run CLI implementer")));

        let cli_mock = Arc::new(MockAgentDriver::new());
        cli_mock.set_delay(Duration::from_millis(500));
        cli_mock.push("imp", Ok(complete_report()));

        ctx.drivers = DriverSet {
            chat: chat_mock,
            cli: cli_mock,
        };

        let checkpoints = ctx.checkpoints.clone();
        let engine = WorkflowEngine::new(ctx).unwrap();

        let cancel = CancellationToken::new();
        let cancel_trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_trigger.cancel();
        });

        let outcome = engine.run(RunOptions::default(), cancel).await.unwrap();
        assert_eq!(outcome, RunOutcome::Interrupted);

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::InProgress);
        assert!(
            container_backend
                .calls()
                .contains(&MockCall::Remove(cli_container_name(&state.run_id, "imp")))
        );
    }

    #[tokio::test]
    async fn cancel_while_awaiting_operator_keeps_awaiting() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_halt = Some(HookConfig {
            tool: "notify:halt".to_string(),
            on_failure: HookFailure::Warn,
        });

        let transport = Arc::new(MockMcpTransport::new("notify").with_tool(
            "halt",
            serde_json::json!({
                "content": [{ "type": "text", "text": "halted on escalate" }]
            }),
        ));
        let mut gk = McpGatekeeper::new();
        gk.register("notify", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let checkpoints = ctx.checkpoints.clone();
        let mock = attach_mock_driver(&mut ctx);
        mock.push("orch", Ok(escalate_report("Need operator guidance")));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let cancel = CancellationToken::new();
        let cancel_trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_trigger.cancel();
        });

        let first_outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: true,
                    ..Default::default()
                },
                cancel,
            )
            .await
            .unwrap();
        assert_eq!(first_outcome, RunOutcome::Interrupted);

        let state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(state.step_statuses[0], StepStatus::AwaitingOperator);
        assert_eq!(state.phase, EnginePhase::AwaitingOperator);
        assert_eq!(
            state.awaiting_reason.as_deref(),
            Some("Need operator guidance")
        );
        assert_eq!(state.hook_results.len(), 1);
        assert_eq!(state.hook_results[0].event, HookEvent::Halt);
        assert_eq!(transport.calls().len(), 1);

        let invocations_after_first = mock.invocations().len();
        assert_eq!(invocations_after_first, 1);

        let second_outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            second_outcome,
            RunOutcome::AwaitingOperator {
                step_number: 1,
                reason: "Need operator guidance".to_string(),
            }
        );
        assert_eq!(mock.invocations().len(), invocations_after_first);
        assert_eq!(transport.calls().len(), 1);
    }

    fn init_project_with_agents(
        dir: &Path,
        write_plan: bool,
    ) -> (
        ConfigStore,
        Backends,
        Arc<MockAgentDriver>,
        Arc<InMemorySecretStore>,
    ) {
        let root = dir.join("project");
        let xdg = dir.join("xdg");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&xdg).unwrap();

        let store = ConfigStore::new(&root, Some(xdg)).unwrap();
        store.init_project("test-project").unwrap();

        store
            .save_inference(&InferenceConfig {
                inference_id: "router".to_string(),
                endpoint_url: "http://192.0.2.1:8080".to_string(),
                mode: InferenceMode::Router,
                lifecycle: InferenceLifecycle::External,
                unload_after_stage: true,
                startup_timeout_secs: 30,
            })
            .unwrap();
        let mut project = store.load_project().unwrap();

        for (stage_idx, id) in ["orch", "imp", "val"].into_iter().enumerate() {
            store
                .save_agent(&AgentConfig {
                    agent_id: id.to_string(),
                    display_name: id.to_string(),
                    backend: AgentBackend::LocalModel {
                        inference_id: "router".to_string(),
                        model: "test-model".to_string(),
                        temperature: 0.0,
                        max_context_tokens: 4096,
                    },
                })
                .unwrap();
            project.assign_agent(stage_idx, id).unwrap();
        }
        store.save_project(&project).unwrap();

        if write_plan {
            std::fs::write(
                store.project_root().join(&project.plan_path),
                "## Step 1: First\nStep 1 body\n## Step 2: Second\nStep 2 body\n",
            )
            .unwrap();
        }

        let secrets = Arc::new(InMemorySecretStore::new());
        let mock_driver = Arc::new(MockAgentDriver::new());
        let dyn_driver: Arc<dyn AgentDriver> = Arc::clone(&mock_driver) as Arc<dyn AgentDriver>;
        let backends = Backends {
            secrets: Arc::clone(&secrets) as Arc<dyn SecretStore>,
            containers: Arc::new(MockContainerBackend::new()),
            inference_control: Arc::new(MockInferenceControl::new()),
            connector: Arc::new(MockMcpConnector::default()),
            drivers: DriverSet {
                chat: Arc::clone(&dyn_driver),
                cli: dyn_driver,
            },
        };

        (store, backends, mock_driver, secrets)
    }

    #[tokio::test]
    async fn build_engine_from_initialized_project() {
        let dir = tempfile::tempdir().unwrap();
        let (store, backends, mock_driver, _secrets) = init_project_with_agents(dir.path(), true);

        script_passing_step(&mock_driver, "orch", "imp", "val");

        let engine = build_engine(&store, backends).await.unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let state = engine.ctx.checkpoints.load_state().unwrap().unwrap();
        assert_eq!(
            state.step_statuses,
            vec![StepStatus::Passed, StepStatus::Pending]
        );
    }

    #[tokio::test]
    async fn build_engine_registers_secrets_with_redactor() {
        let dir = tempfile::tempdir().unwrap();
        let (store, backends, _mock_driver, secrets) = init_project_with_agents(dir.path(), true);

        let cloud_secret = "s3cr3t-cloud-api-token-987654";
        secrets
            .set(
                "orrery.cloud",
                "testuser",
                &SecretValue::new(cloud_secret.to_string()),
            )
            .unwrap();

        store
            .save_agent(&AgentConfig {
                agent_id: "imp".to_string(),
                display_name: "Cloud Implementer".to_string(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "cloud-model".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 8192,
                    keychain_service: "orrery.cloud".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            })
            .unwrap();

        let engine = build_engine(&store, backends).await.unwrap();
        engine
            .ctx
            .stream
            .emit(StreamEvent {
                timestamp: Utc::now(),
                step_number: Some(1),
                stage_index: Some(1),
                agent_id: Some("imp".to_string()),
                kind: StreamEventKind::Output,
                text: format!("leaked={cloud_secret}"),
            })
            .unwrap();

        let events = engine.ctx.stream.recent(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "leaked=[REDACTED]");
        assert!(!events[0].text.contains(cloud_secret));
    }

    #[tokio::test]
    async fn build_engine_missing_plan_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let (store, backends, _mock_driver, _secrets) = init_project_with_agents(dir.path(), false);

        let err = match build_engine(&store, backends).await {
            Ok(_) => panic!("expected Error::Io for missing plan file"),
            Err(err) => err,
        };
        assert!(
            matches!(err, Error::Io { .. }),
            "expected Error::Io, got {err:?}"
        );
    }

    #[tokio::test]
    async fn build_engine_unknown_mcp_server_errors() {
        let dir = tempfile::tempdir().unwrap();
        let (store, backends, _mock_driver, _secrets) = init_project_with_agents(dir.path(), true);

        let mut coder_role = store
            .load_roles()
            .unwrap()
            .into_iter()
            .find(|r| r.role_id == "coder")
            .unwrap();
        let coder_rules = store.load_role_rules("coder").unwrap();
        coder_role.allowed_mcp_tools = vec!["unknown_srv:*".to_string()];
        store.save_role(&coder_role, &coder_rules).unwrap();

        store
            .save_mcp_server(&McpServerConfig {
                server_id: "unknown_srv".to_string(),
                transport: McpTransportConfig::Stdio {
                    command: "fake-mcp".to_string(),
                    args: Vec::new(),
                },
                write_tools: Vec::new(),
            })
            .unwrap();

        let err = match build_engine(&store, backends).await {
            Ok(_) => panic!("expected error when MCP connector does not know server"),
            Err(err) => err,
        };
        match err {
            Error::Mcp { server_id, .. } => assert_eq!(server_id, "unknown_srv"),
            other => panic!("expected Error::Mcp, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn build_engine_attaches_stream_to_connector() {
        use crate::mcp::{McpConnector, McpTransport};
        use std::sync::atomic::{AtomicBool, Ordering};

        struct RecordingConnector {
            inner: MockMcpConnector,
            attached: Arc<AtomicBool>,
        }

        #[async_trait::async_trait]
        impl McpConnector for RecordingConnector {
            async fn connect(
                &self,
                server: &McpServerConfig,
            ) -> crate::Result<Arc<dyn McpTransport>> {
                self.inner.connect(server).await
            }

            fn attach_stream(&self, _stream: Arc<LiveStreamHub>) {
                self.attached.store(true, Ordering::SeqCst);
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let (store, mut backends, _mock_driver, _secrets) =
            init_project_with_agents(dir.path(), true);
        let attached = Arc::new(AtomicBool::new(false));
        backends.connector = Arc::new(RecordingConnector {
            inner: MockMcpConnector::default(),
            attached: Arc::clone(&attached),
        });

        let _engine = build_engine(&store, backends).await.unwrap();
        assert!(attached.load(Ordering::SeqCst));
    }

    #[test]
    fn referenced_servers_from_stages_gates_hooks() {
        let mut project = ProjectConfig::default_template("test-project");
        project.gate_tools = vec!["cargo:test".to_string()];
        project.hooks.on_step_pass = Some(HookConfig {
            tool: "notify:send".to_string(),
            on_failure: HookFailure::Warn,
        });

        let roles = vec![
            RoleConfig {
                role_id: "orchestrator".to_string(),
                display_name: "Orchestrator".to_string(),
                kind: RoleKind::Orchestrator,
                rules_file: "orchestrator.md".to_string(),
                allowed_mcp_tools: vec!["fs:read_file".to_string()],
            },
            RoleConfig {
                role_id: "coder".to_string(),
                display_name: "Coder".to_string(),
                kind: RoleKind::Implementer,
                rules_file: "coder.md".to_string(),
                allowed_mcp_tools: vec!["git:*".to_string()],
            },
            RoleConfig {
                role_id: "validator".to_string(),
                display_name: "Validator".to_string(),
                kind: RoleKind::Validator,
                rules_file: "validator.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "researcher".to_string(),
                display_name: "Researcher".to_string(),
                kind: RoleKind::Implementer,
                rules_file: "researcher.md".to_string(),
                allowed_mcp_tools: vec!["web:fetch".to_string()],
            },
        ];

        let referenced = super::referenced_mcp_servers(&project, &roles);
        let expected: BTreeSet<String> = ["cargo", "fs", "git", "notify"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(referenced, expected);
        assert!(!referenced.contains("web"));
    }

    #[tokio::test]
    async fn build_engine_skips_unused_server() {
        let dir = tempfile::tempdir().unwrap();
        let (store, mut backends, _mock_driver, _secrets) =
            init_project_with_agents(dir.path(), true);

        let mut coder_role = store
            .load_roles()
            .unwrap()
            .into_iter()
            .find(|r| r.role_id == "coder")
            .unwrap();
        let coder_rules = store.load_role_rules("coder").unwrap();
        coder_role.allowed_mcp_tools = vec!["fs:read_file".to_string()];
        store.save_role(&coder_role, &coder_rules).unwrap();

        for server_id in ["fs", "unused"] {
            store
                .save_mcp_server(&McpServerConfig {
                    server_id: server_id.to_string(),
                    transport: McpTransportConfig::Stdio {
                        command: "fake-mcp".to_string(),
                        args: Vec::new(),
                    },
                    write_tools: Vec::new(),
                })
                .unwrap();
        }

        let mut connector = MockMcpConnector::default();
        connector.transports.insert(
            "fs".to_string(),
            Arc::new(MockMcpTransport::new("fs").with_tool(
                "read_file",
                serde_json::json!({
                    "content": [{ "type": "text", "text": "ok" }]
                }),
            )),
        );
        backends.connector = Arc::new(connector);

        let engine = build_engine(&store, backends).await.unwrap();
        let events = engine.ctx.stream.recent(10).unwrap();
        assert!(events.iter().any(|ev| {
            ev.kind == StreamEventKind::SystemNotice
                && ev.text == "MCP server 'unused' is not used by this workflow; not connected"
        }));
    }

    #[tokio::test]
    async fn step_pass_hook_halt_approve_reruns_hook_only() {
        let dir = tempfile::tempdir().unwrap();
        let stages = &[
            (RoleKind::Orchestrator, &["orch"][..]),
            (RoleKind::Implementer, &["imp"][..]),
            (RoleKind::Validator, &["val"][..]),
        ];

        // Run 1: on_step_pass = git:commit (Halt) failing with isError: true
        let mut ctx1 = test_context(dir.path(), stages);
        ctx1.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        });
        let mut gk1 = McpGatekeeper::new();
        gk1.register(
            "git",
            Arc::new(MockMcpTransport::new("git").with_tool(
                "commit",
                serde_json::json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "commit failed" }]
                }),
            )),
            Vec::new(),
        );
        ctx1.gatekeeper = Arc::new(gk1);
        let checkpoints = ctx1.checkpoints.clone();
        let mock1 = attach_mock_driver(&mut ctx1);
        script_passing_step(&mock1, "orch", "imp", "val");

        let engine1 = WorkflowEngine::new(ctx1).unwrap();
        let outcome1 = engine1
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome1,
            RunOutcome::AwaitingOperator {
                step_number: 1,
                reason: "hook step_pass failed".to_string(),
            }
        );

        let mut state = checkpoints.load_state().unwrap().unwrap();
        state.operator_decision = Some(OperatorDecision::Approve);
        checkpoints.save_state(&state).unwrap();

        // Run 2: succeeding commit and fresh MockAgentDriver with no scripts
        let mut ctx2 = test_context(dir.path(), stages);
        ctx2.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        });
        let transport2 = Arc::new(MockMcpTransport::new("git").with_tool(
            "commit",
            serde_json::json!({
                "content": [{ "type": "text", "text": "committed" }]
            }),
        ));
        let mut gk2 = McpGatekeeper::new();
        gk2.register("git", Arc::clone(&transport2) as _, Vec::new());
        ctx2.gatekeeper = Arc::new(gk2);
        let mock2 = attach_mock_driver(&mut ctx2);

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome2, RunOutcome::Completed { steps_passed: 1 });
        assert!(mock2.invocations().is_empty());

        let calls = transport2.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "commit");

        let final_state = checkpoints.load_state().unwrap().unwrap();
        assert_eq!(final_state.step_statuses[0], StepStatus::Passed);
    }

    #[tokio::test]
    async fn step_pass_hook_halt_reject_reruns_orchestrator() {
        let dir = tempfile::tempdir().unwrap();
        let stages = &[
            (RoleKind::Orchestrator, &["orch"][..]),
            (RoleKind::Implementer, &["imp"][..]),
            (RoleKind::Validator, &["val"][..]),
        ];

        // Run 1: on_step_pass = git:commit (Halt) failing with isError: true
        let mut ctx1 = test_context(dir.path(), stages);
        ctx1.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        });
        let mut gk1 = McpGatekeeper::new();
        gk1.register(
            "git",
            Arc::new(MockMcpTransport::new("git").with_tool(
                "commit",
                serde_json::json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "commit failed" }]
                }),
            )),
            Vec::new(),
        );
        ctx1.gatekeeper = Arc::new(gk1);
        let checkpoints = ctx1.checkpoints.clone();
        let mock1 = attach_mock_driver(&mut ctx1);
        script_passing_step(&mock1, "orch", "imp", "val");

        let engine1 = WorkflowEngine::new(ctx1).unwrap();
        let outcome1 = engine1
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome1,
            RunOutcome::AwaitingOperator {
                step_number: 1,
                reason: "hook step_pass failed".to_string(),
            }
        );

        let mut state = checkpoints.load_state().unwrap().unwrap();
        state.operator_decision = Some(OperatorDecision::Reject {
            feedback: "tag missing".to_string(),
        });
        checkpoints.save_state(&state).unwrap();

        // Run 2: scripted with script_passing_step and succeeding commit
        let mut ctx2 = test_context(dir.path(), stages);
        ctx2.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        });
        let mut gk2 = McpGatekeeper::new();
        gk2.register(
            "git",
            Arc::new(MockMcpTransport::new("git").with_tool(
                "commit",
                serde_json::json!({
                    "content": [{ "type": "text", "text": "committed" }]
                }),
            )),
            Vec::new(),
        );
        ctx2.gatekeeper = Arc::new(gk2);
        let mock2 = attach_mock_driver(&mut ctx2);
        script_passing_step(&mock2, "orch", "imp", "val");

        let engine2 = WorkflowEngine::new(ctx2).unwrap();
        let outcome2 = engine2
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome2, RunOutcome::Completed { steps_passed: 1 });

        let invocations = mock2.invocations();
        assert!(!invocations.is_empty());
        assert_eq!(invocations[0].stage_index, 0);
        assert!(
            invocations[0]
                .user_prompt
                .contains("operator rejected: tag missing")
        );
    }

    #[tokio::test]
    async fn step_pass_hook_halt_approve_while_waiting() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.hooks.on_step_pass = Some(HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        });

        let transport = Arc::new(MockMcpTransport::new("git").with_tool(
            "commit",
            serde_json::json!({
                "isError": true,
                "content": [{ "type": "text", "text": "commit failed" }]
            }),
        ));
        let mut gk = McpGatekeeper::new();
        gk.register("git", Arc::clone(&transport) as _, Vec::new());
        ctx.gatekeeper = Arc::new(gk);

        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = Arc::new(WorkflowEngine::new(ctx).unwrap());
        let mut events_rx = engine.subscribe();
        let op_tx = engine.operator_sender();
        let cancel = CancellationToken::new();

        let run_handle = {
            let engine = Arc::clone(&engine);
            let cancel = cancel.clone();
            tokio::spawn(async move {
                engine
                    .run(
                        RunOptions {
                            wait_for_operator: true,
                            ..Default::default()
                        },
                        cancel,
                    )
                    .await
            })
        };

        // Wait for first AwaitingOperator
        loop {
            if let EngineEvent::AwaitingOperator {
                step_number,
                reason,
            } = events_rx.recv().await.unwrap()
            {
                assert_eq!(step_number, 1);
                assert_eq!(reason, "hook step_pass failed");
                break;
            }
        }

        let inv_count_at_first = mock.invocations().len();
        assert_eq!(
            transport
                .calls()
                .iter()
                .filter(|(t, _)| t == "commit")
                .count(),
            1
        );

        op_tx.send(OperatorDecision::Approve).await.unwrap();

        // Wait for second AwaitingOperator
        loop {
            if let EngineEvent::AwaitingOperator {
                step_number,
                reason,
            } = events_rx.recv().await.unwrap()
            {
                assert_eq!(step_number, 1);
                assert_eq!(reason, "hook step_pass failed");
                break;
            }
        }

        assert_eq!(
            transport
                .calls()
                .iter()
                .filter(|(t, _)| t == "commit")
                .count(),
            2
        );
        assert_eq!(mock.invocations().len(), inv_count_at_first);

        cancel.cancel();
        let outcome = run_handle.await.unwrap().unwrap();
        assert_eq!(outcome, RunOutcome::Interrupted);
    }

    #[tokio::test]
    async fn build_engine_reports_hidden_tool() {
        let dir = tempfile::tempdir().unwrap();
        let (store, mut backends, _mock_driver, _secrets) =
            init_project_with_agents(dir.path(), true);

        let mut coder_role = store
            .load_roles()
            .unwrap()
            .into_iter()
            .find(|r| r.role_id == "coder")
            .unwrap();
        let coder_rules = store.load_role_rules("coder").unwrap();
        coder_role.allowed_mcp_tools = vec!["fs:*".to_string()];
        store.save_role(&coder_role, &coder_rules).unwrap();

        store
            .save_mcp_server(&McpServerConfig {
                server_id: "fs".to_string(),
                transport: McpTransportConfig::Stdio {
                    command: "fake-mcp".to_string(),
                    args: Vec::new(),
                },
                write_tools: Vec::new(),
            })
            .unwrap();

        let mut connector = MockMcpConnector::default();
        connector.transports.insert(
            "fs".to_string(),
            Arc::new(
                MockMcpTransport::new("fs")
                    .with_tool(
                        "read_file",
                        serde_json::json!({
                            "content": [{ "type": "text", "text": "ok" }]
                        }),
                    )
                    .with_tool(
                        "read.file",
                        serde_json::json!({
                            "content": [{ "type": "text", "text": "ok" }]
                        }),
                    ),
            ),
        );
        backends.connector = Arc::new(connector);

        let engine = build_engine(&store, backends).await.unwrap();
        let events = engine.ctx.stream.recent(20).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|ev| {
                    ev.kind == StreamEventKind::SystemNotice
                        && ev.text
                            == "MCP tool 'fs:read.file' is hidden: its name can't map to a function name"
                })
                .count(),
            1
        );
        assert!(
            !events
                .iter()
                .any(|ev| ev.text.contains("'fs:read_file' is hidden"))
        );
    }

    #[tokio::test]
    async fn run_stops_started_inference_on_exit() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.project.inference[0].lifecycle = InferenceLifecycle::Container {
            name: "llama-test".into(),
        };
        ctx.project.inference[0].mode = InferenceMode::Single;
        ctx.project.inference[0].unload_after_stage = false;

        let handle = Arc::new(MockInferenceControl::new());
        ctx.resources = Arc::new(ResourceManager::new(
            Arc::new(MockContainerBackend::new()),
            Arc::clone(&handle) as _,
            ctx.project.inference.clone(),
            ctx.project.max_concurrent_agents,
            ctx.project_root.clone(),
        ));

        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(
                RunOptions {
                    wait_for_operator: false,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });
        assert!(
            handle
                .calls()
                .contains(&crate::docker::InferenceCall::Stop("router".into()))
        );
    }

    #[tokio::test]
    async fn stale_operator_decision_is_drained() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        let mock = attach_mock_driver(&mut ctx);
        mock.push("orch", Ok(escalate_report("need guidance")));
        mock.push("orch", Ok(escalate_report("still need guidance")));

        let engine = Arc::new(WorkflowEngine::new(ctx).unwrap());
        let mut events_rx = engine.subscribe();
        let op_tx = engine.operator_sender();

        op_tx.send(OperatorDecision::Approve).await.unwrap();

        let cancel = CancellationToken::new();
        let mut handle = {
            let engine = Arc::clone(&engine);
            let cancel = cancel.clone();
            tokio::spawn(async move {
                engine
                    .run(
                        RunOptions {
                            wait_for_operator: true,
                            ..Default::default()
                        },
                        cancel,
                    )
                    .await
            })
        };

        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let EngineEvent::AwaitingOperator { .. } = events_rx.recv().await.unwrap() {
                    break;
                }
            }
        })
        .await
        .expect("timed out waiting for AwaitingOperator");

        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut handle)
                .await
                .is_err()
        );

        op_tx
            .send(OperatorDecision::Reject {
                feedback: "x".into(),
            })
            .await
            .unwrap();

        cancel.cancel();
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn fresh_step_start_clears_old_reports() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );
        ctx.checkpoints
            .save_report(&StampedReport {
                step_number: 1,
                plan_step_sha256: ctx.plan.steps[0].sha256_hex.clone(),
                round: 2,
                stage_index: 0,
                role_id: "orchestrator".to_string(),
                agent_id: "orch".to_string(),
                recorded_at: Utc::now(),
                report: AgentReport {
                    status: ReportStatus::Complete,
                    summary: "stale-from-before-reset".to_string(),
                    target_stage: None,
                    allocations: Vec::new(),
                    feedback: None,
                    artifacts: Vec::new(),
                },
            })
            .unwrap();

        let mock = attach_mock_driver(&mut ctx);
        script_passing_step(&mock, "orch", "imp", "val");

        let engine = WorkflowEngine::new(ctx).unwrap();
        let outcome = engine
            .run(RunOptions::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(outcome, RunOutcome::Completed { steps_passed: 1 });

        let reports = engine.ctx.checkpoints.load_reports(1, None).unwrap();
        assert!(!reports.is_empty());
        assert!(
            !reports
                .iter()
                .any(|r| r.report.summary == "stale-from-before-reset")
        );
    }

    #[tokio::test]
    async fn build_engine_warns_on_unreadable_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let (store, backends, _mock_driver, _secrets) = init_project_with_agents(dir.path(), true);

        store
            .save_agent(&AgentConfig {
                agent_id: "cloud-missing".to_string(),
                display_name: "Cloud Missing".to_string(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "cloud-model".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 8192,
                    keychain_service: "orrery.test.missing".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            })
            .unwrap();
        let mut project = store.load_project().unwrap();
        project.assign_agent(1, "cloud-missing").unwrap();
        store.save_project(&project).unwrap();

        let engine = build_engine(&store, backends).await.unwrap();
        let events = engine.ctx.stream.recent(50).unwrap();
        assert!(events.iter().any(|ev| {
            ev.kind == StreamEventKind::SystemNotice
                && ev.text.contains(
                    "orrery.test.missing/testuser for agent 'cloud-missing' could not be read",
                )
                && ev.text.contains("it will not be redacted")
        }));
    }

    #[tokio::test]
    async fn cancel_without_lease_removes_cli_orchestrator() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = test_context(
            dir.path(),
            &[
                (RoleKind::Orchestrator, &["orch"]),
                (RoleKind::Implementer, &["imp"]),
                (RoleKind::Validator, &["val"]),
            ],
        );

        let orch_agent = ctx
            .agents
            .iter_mut()
            .find(|a| a.agent_id == "orch")
            .expect("stage-0 agent orch must exist");
        orch_agent.backend = AgentBackend::AgentCli {
            image: format!("busybox@sha256:{}", "0".repeat(64)),
            command: vec!["sh".to_string()],
            workspace_mount: WorkspaceMount::None,
            memory_limit_mb: 256,
            timeout_secs: 60,
            keychain_service: None,
            keychain_account: None,
            secret_env: None,
        };

        let container_backend = Arc::new(MockContainerBackend::new());
        ctx.resources = Arc::new(ResourceManager::new(
            Arc::clone(&container_backend) as _,
            Arc::new(MockInferenceControl::new()),
            ctx.project.inference.clone(),
            ctx.project.max_concurrent_agents,
            dir.path().to_path_buf(),
        ));

        let engine = WorkflowEngine::new(ctx).unwrap();
        let mut state = crate::checkpoint::WorkflowCheckpoint::new(&engine.ctx.plan);
        let step = engine.ctx.plan.steps[0].clone();

        let outcome = engine.handle_cancel(&mut state, 0, &step).await.unwrap();
        assert_eq!(outcome, RunOutcome::Interrupted);
        assert!(
            container_backend
                .calls()
                .contains(&MockCall::Remove(cli_container_name(&state.run_id, "orch")))
        );
    }
}
