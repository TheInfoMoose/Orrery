use std::collections::VecDeque;
use std::path::PathBuf;

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use orrery_core::checkpoint::{EnginePhase, StepStatus};
use orrery_core::config::{
    AgentConfig, ExecutionMode, MAX_STAGE_AGENTS, MAX_STAGES, MIN_STAGES, ProjectConfig,
    RoleConfig, ValidationMode,
};
use orrery_core::engine::{EngineEvent, RunOutcome};
use orrery_core::plan::ImplementationPlan;
use orrery_core::stream_log::StreamEvent;

use crate::layout::{UiLayoutRegions, UiRegion};

const STREAM_CAPACITY: usize = 2_000;
const STREAM_PAGE_STEP: usize = 10;
const REMOVE_OPTION: &str = "(remove)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    TopMenu,
    StepsPanel,
    StageCanvas,
    LiveStream,
    FooterMenu,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::TopMenu => Self::StepsPanel,
            Self::StepsPanel => Self::StageCanvas,
            Self::StageCanvas => Self::LiveStream,
            Self::LiveStream => Self::FooterMenu,
            Self::FooterMenu => Self::TopMenu,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::TopMenu => Self::FooterMenu,
            Self::StepsPanel => Self::TopMenu,
            Self::StageCanvas => Self::StepsPanel,
            Self::LiveStream => Self::StageCanvas,
            Self::FooterMenu => Self::LiveStream,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RunStatus {
    #[default]
    Idle,
    Running,
    AwaitingOperator,
    Passed,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorKind {
    Role,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub stage: usize,
    pub slot: usize,
    pub kind: SelectorKind,
    pub options: Vec<String>,
    pub index: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Modal {
    #[default]
    None,
    Ingest {
        input: String,
    },
    Settings,
    Help,
    QuitConfirm,
    RejectFeedback {
        input: String,
    },
    Selector(Selector),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Run,
    Interrupt,
    Approve,
    Reject(String),
    IngestPlan(PathBuf),
    SetMode(ExecutionMode),
    SetCycles(usize),
    SetStageCount(usize),
    SetStageRole { stage: usize, role_id: String },
    AssignAgent { stage: usize, agent_id: String },
    UnassignAgent { stage: usize, agent_id: String },
    SetMouseCapture(bool),
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub project_root: PathBuf,
    pub config: ProjectConfig,
    pub roles: Vec<RoleConfig>,
    pub agents: Vec<AgentConfig>,
    pub plan: Option<ImplementationPlan>,
    pub step_statuses: Vec<StepStatus>,
    pub focus: Focus,
    pub modal: Modal,
    pub top_index: usize,
    pub footer_index: usize,
    pub step_selected: usize,
    pub stage_selected: usize,
    pub slot_selected: usize,
    pub stage_scroll: usize,
    pub slot_scroll: usize,
    pub stream: VecDeque<StreamEvent>,
    pub stream_scroll: usize,
    pub mouse_capture: bool,
    pub status: RunStatus,
    pub awaiting_operator: bool,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        project_root: PathBuf,
        config: ProjectConfig,
        roles: Vec<RoleConfig>,
        agents: Vec<AgentConfig>,
    ) -> Self {
        Self {
            project_root,
            config,
            roles,
            agents,
            plan: None,
            step_statuses: Vec::new(),
            focus: Focus::TopMenu,
            modal: Modal::None,
            top_index: 0,
            footer_index: 0,
            step_selected: 0,
            stage_selected: 0,
            slot_selected: 0,
            stage_scroll: 0,
            slot_scroll: 0,
            stream: VecDeque::new(),
            stream_scroll: 0,
            mouse_capture: true,
            status: RunStatus::Idle,
            awaiting_operator: false,
            status_message: None,
            should_quit: false,
        }
    }

    fn is_running(&self) -> bool {
        matches!(
            self.status,
            RunStatus::Running | RunStatus::AwaitingOperator
        )
    }

    pub fn unlocked_slots(&self, stage: usize) -> usize {
        let Some(stage_cfg) = self.config.stages.get(stage) else {
            return 0;
        };
        if stage_cfg.role_id.trim().is_empty() {
            return 0;
        }
        let is_orchestrator = self
            .roles
            .iter()
            .find(|r| r.role_id == stage_cfg.role_id)
            .map_or(
                stage_cfg.role_id == "orchestrator",
                RoleConfig::is_orchestrator,
            );
        if is_orchestrator {
            1
        } else {
            MAX_STAGE_AGENTS.min(stage_cfg.agent_ids.len() + 1)
        }
    }

    pub fn approve_visible(&self) -> bool {
        self.awaiting_operator
            && self.config.effective_validation_mode(&self.roles) == ValidationMode::GateAndOperator
    }

    pub fn push_stream_event(&mut self, event: StreamEvent) {
        while self.stream.len() >= STREAM_CAPACITY {
            self.stream.pop_front();
        }
        self.stream.push_back(event);
    }

    fn set_step_status(&mut self, step_number: usize, status: StepStatus) {
        if let Some(plan) = &self.plan
            && let Some(pos) = plan.steps.iter().position(|s| s.number == step_number)
            && let Some(slot) = self.step_statuses.get_mut(pos)
        {
            *slot = status;
            return;
        }
        if step_number > 0
            && let Some(slot) = self.step_statuses.get_mut(step_number - 1)
        {
            *slot = status;
        }
    }

    pub fn apply_engine_event(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::PhaseChanged(phase) => match phase {
                EnginePhase::Idle => {
                    self.status = RunStatus::Idle;
                    self.awaiting_operator = false;
                }
                EnginePhase::OrchestratorTurn
                | EnginePhase::StageTurn
                | EnginePhase::Gate
                | EnginePhase::Hook => {
                    self.status = RunStatus::Running;
                    self.awaiting_operator = false;
                }
                EnginePhase::AwaitingOperator => {
                    self.status = RunStatus::AwaitingOperator;
                    self.awaiting_operator = true;
                }
                EnginePhase::Done => {
                    self.status = RunStatus::Passed;
                    self.awaiting_operator = false;
                }
            },
            EngineEvent::StepStatusChanged {
                step_number,
                status,
            } => {
                self.set_step_status(*step_number, *status);
                match status {
                    StepStatus::AwaitingOperator => {
                        self.status = RunStatus::AwaitingOperator;
                        self.awaiting_operator = true;
                    }
                    StepStatus::Failed => {
                        self.status = RunStatus::Error;
                        self.awaiting_operator = false;
                    }
                    StepStatus::InProgress => {
                        self.status = RunStatus::Running;
                        self.awaiting_operator = false;
                    }
                    StepStatus::Pending | StepStatus::Passed => {}
                }
            }
            EngineEvent::RoundStarted { .. } => {
                self.status = RunStatus::Running;
                self.awaiting_operator = false;
            }
            EngineEvent::AwaitingOperator {
                step_number,
                reason,
            } => {
                self.status = RunStatus::AwaitingOperator;
                self.awaiting_operator = true;
                self.status_message = Some(reason.clone());
                self.set_step_status(*step_number, StepStatus::AwaitingOperator);
            }
            EngineEvent::RunFinished(outcome) => match outcome {
                RunOutcome::Completed { .. } | RunOutcome::NoPendingSteps => {
                    self.status = RunStatus::Passed;
                    self.awaiting_operator = false;
                }
                RunOutcome::AwaitingOperator {
                    step_number,
                    reason,
                } => {
                    self.status = RunStatus::AwaitingOperator;
                    self.awaiting_operator = true;
                    self.status_message = Some(reason.clone());
                    self.set_step_status(*step_number, StepStatus::AwaitingOperator);
                }
                RunOutcome::Interrupted => {
                    self.status = RunStatus::Idle;
                    self.awaiting_operator = false;
                }
            },
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Vec<AppAction> {
        if key.kind == KeyEventKind::Release {
            return Vec::new();
        }

        // Row 1: Ctrl+C -> running: [Interrupt]; idle: []. Any other key with CONTROL or ALT -> [], no state change.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('c' | 'C'))
        {
            return if self.is_running() {
                vec![AppAction::Interrupt]
            } else {
                Vec::new()
            };
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Vec::new();
        }

        // Rows 2-5: Modal handling.
        match &mut self.modal {
            // Row 2: Text modal (Ingest, RejectFeedback)
            Modal::Ingest { input } => {
                return match key.code {
                    KeyCode::Char(c) if !c.is_control() => {
                        input.push(c);
                        Vec::new()
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        Vec::new()
                    }
                    KeyCode::Enter => {
                        let path = PathBuf::from(std::mem::take(input));
                        self.modal = Modal::None;
                        vec![AppAction::IngestPlan(path)]
                    }
                    KeyCode::Esc => {
                        self.modal = Modal::None;
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
            }
            Modal::RejectFeedback { input } => {
                return match key.code {
                    KeyCode::Char(c) if !c.is_control() => {
                        input.push(c);
                        Vec::new()
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        Vec::new()
                    }
                    KeyCode::Enter => {
                        let text = std::mem::take(input);
                        self.modal = Modal::None;
                        vec![AppAction::Reject(text)]
                    }
                    KeyCode::Esc => {
                        self.modal = Modal::None;
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
            }
            // Row 3: Selector
            Modal::Selector(selector) => {
                return match key.code {
                    KeyCode::Up => {
                        selector.index = selector.index.saturating_sub(1);
                        Vec::new()
                    }
                    KeyCode::Down => {
                        if selector.index + 1 < selector.options.len() {
                            selector.index += 1;
                        }
                        Vec::new()
                    }
                    KeyCode::Esc => {
                        self.modal = Modal::None;
                        Vec::new()
                    }
                    KeyCode::Enter => {
                        let stage = selector.stage;
                        let slot = selector.slot;
                        let kind = selector.kind;
                        let chosen = selector.options.get(selector.index).cloned();
                        self.modal = Modal::None;
                        if let Some(option) = chosen {
                            self.apply_selector_choice(stage, slot, kind, option)
                        } else {
                            Vec::new()
                        }
                    }
                    _ => Vec::new(),
                };
            }
            // Row 4: QuitConfirm
            Modal::QuitConfirm => {
                return match key.code {
                    KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                        self.modal = Modal::None;
                        self.should_quit = true;
                        vec![AppAction::Interrupt, AppAction::Quit]
                    }
                    KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                        self.modal = Modal::None;
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
            }
            // Row 5: Help / Settings
            Modal::Help | Modal::Settings => {
                return match key.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                        self.modal = Modal::None;
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
            }
            Modal::None => {}
        }

        // Row 6: No modal global keys.
        match key.code {
            KeyCode::Tab => {
                self.focus = self.focus.next();
                return Vec::new();
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                return Vec::new();
            }
            KeyCode::Char('q') => {
                return self.activate_quit();
            }
            KeyCode::Char('?') => {
                self.modal = Modal::Help;
                return Vec::new();
            }
            KeyCode::Char('m') => {
                return self.toggle_mouse_capture();
            }
            _ => {}
        }

        // Rows 7-11: Panel focus keys.
        match self.focus {
            Focus::TopMenu => self.handle_top_menu_key(key.code),
            Focus::StepsPanel => self.handle_steps_panel_key(key.code),
            Focus::StageCanvas => self.handle_stage_canvas_key(key.code),
            Focus::LiveStream => self.handle_live_stream_key(key.code),
            Focus::FooterMenu => self.handle_footer_menu_key(key.code),
        }
    }

    fn activate_quit(&mut self) -> Vec<AppAction> {
        if self.is_running() {
            self.modal = Modal::QuitConfirm;
            Vec::new()
        } else {
            self.should_quit = true;
            vec![AppAction::Quit]
        }
    }

    fn toggle_mouse_capture(&mut self) -> Vec<AppAction> {
        self.mouse_capture = !self.mouse_capture;
        vec![AppAction::SetMouseCapture(self.mouse_capture)]
    }

    fn apply_selector_choice(
        &mut self,
        stage: usize,
        slot: usize,
        kind: SelectorKind,
        option: String,
    ) -> Vec<AppAction> {
        match kind {
            SelectorKind::Role => match self.config.set_stage_role(stage, option.clone()) {
                Ok(()) => {
                    let max_slot = self.unlocked_slots(self.stage_selected);
                    self.slot_selected = self.slot_selected.min(max_slot);
                    vec![AppAction::SetStageRole {
                        stage,
                        role_id: option,
                    }]
                }
                Err(err) => {
                    self.status_message = Some(err.to_string());
                    Vec::new()
                }
            },
            SelectorKind::Agent => {
                if option == REMOVE_OPTION {
                    let existing = self
                        .config
                        .stages
                        .get(stage)
                        .and_then(|s| s.agent_ids.get(slot).cloned());
                    let Some(agent_id) = existing else {
                        return Vec::new();
                    };
                    match self.config.remove_agent(stage, &agent_id) {
                        Ok(()) => {
                            let max_slot = self.unlocked_slots(self.stage_selected);
                            self.slot_selected = self.slot_selected.min(max_slot);
                            vec![AppAction::UnassignAgent { stage, agent_id }]
                        }
                        Err(err) => {
                            self.status_message = Some(err.to_string());
                            Vec::new()
                        }
                    }
                } else {
                    match self.config.assign_agent(stage, option.clone()) {
                        Ok(()) => vec![AppAction::AssignAgent {
                            stage,
                            agent_id: option,
                        }],
                        Err(err) => {
                            self.status_message = Some(err.to_string());
                            Vec::new()
                        }
                    }
                }
            }
        }
    }

    fn change_stage_count(&mut self, delta: isize) -> Vec<AppAction> {
        let cur = self.config.stages.len();
        let next = if delta > 0 {
            cur.saturating_add(delta.cast_unsigned())
                .clamp(MIN_STAGES, MAX_STAGES)
        } else {
            cur.saturating_sub(delta.unsigned_abs())
                .clamp(MIN_STAGES, MAX_STAGES)
        };
        if next == cur {
            return Vec::new();
        }
        match self.config.set_stage_count(next) {
            Ok(()) => {
                if self.stage_selected >= next {
                    self.stage_selected = next - 1;
                }
                if self.stage_scroll >= next {
                    self.stage_scroll = next - 1;
                }
                let max_slot = self.unlocked_slots(self.stage_selected);
                self.slot_selected = self.slot_selected.min(max_slot);
                vec![AppAction::SetStageCount(next)]
            }
            Err(err) => {
                self.status_message = Some(err.to_string());
                Vec::new()
            }
        }
    }

    // Row 7: Focus TopMenu
    fn handle_top_menu_key(&mut self, code: KeyCode) -> Vec<AppAction> {
        match code {
            KeyCode::Left => {
                self.top_index = self.top_index.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Right => {
                if self.top_index < 5 {
                    self.top_index += 1;
                }
                Vec::new()
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.top_index {
                0 => {
                    self.modal = Modal::Ingest {
                        input: String::new(),
                    };
                    Vec::new()
                }
                1 => {
                    let toggled = match self.config.execution_mode {
                        ExecutionMode::Run => ExecutionMode::Loop,
                        ExecutionMode::Loop => ExecutionMode::Run,
                    };
                    self.config.execution_mode = toggled;
                    vec![AppAction::SetMode(toggled)]
                }
                4 => self.change_stage_count(-1),
                5 => self.change_stage_count(1),
                _ => Vec::new(),
            },
            KeyCode::Char('+' | '-') => match self.top_index {
                2 => {
                    if self.config.execution_mode == ExecutionMode::Run {
                        return Vec::new();
                    }
                    let next = if code == KeyCode::Char('+') {
                        self.config.cycle_count.saturating_add(1)
                    } else {
                        self.config.cycle_count.saturating_sub(1)
                    };
                    self.config.cycle_count = next;
                    vec![AppAction::SetCycles(next)]
                }
                4 | 5 => {
                    let delta = if code == KeyCode::Char('+') { 1 } else { -1 };
                    self.change_stage_count(delta)
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    // Row 8: Focus StepsPanel
    fn handle_steps_panel_key(&mut self, code: KeyCode) -> Vec<AppAction> {
        match code {
            KeyCode::Up => {
                self.step_selected = self.step_selected.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Down => {
                let len = self
                    .plan
                    .as_ref()
                    .map_or(0, |p| p.steps.len())
                    .max(self.step_statuses.len());
                if len > 0 && self.step_selected + 1 < len {
                    self.step_selected += 1;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    // Row 9: Focus StageCanvas
    fn handle_stage_canvas_key(&mut self, code: KeyCode) -> Vec<AppAction> {
        match code {
            KeyCode::Left => {
                if self.stage_selected > 0 {
                    self.stage_selected -= 1;
                    if self.stage_selected < self.stage_scroll {
                        self.stage_scroll = self.stage_selected;
                    }
                    let max_slot = self.unlocked_slots(self.stage_selected);
                    self.slot_selected = self.slot_selected.min(max_slot);
                }
                Vec::new()
            }
            KeyCode::Right => {
                if self.stage_selected + 1 < self.config.stages.len() {
                    self.stage_selected += 1;
                    let max_slot = self.unlocked_slots(self.stage_selected);
                    self.slot_selected = self.slot_selected.min(max_slot);
                }
                Vec::new()
            }
            KeyCode::Up => {
                self.slot_selected = self.slot_selected.saturating_sub(1);
                if self.slot_selected > 0 && self.slot_selected - 1 < self.slot_scroll {
                    self.slot_scroll = self.slot_selected - 1;
                }
                Vec::new()
            }
            KeyCode::Down => {
                let max_slot = self.unlocked_slots(self.stage_selected);
                if self.slot_selected < max_slot {
                    self.slot_selected += 1;
                }
                Vec::new()
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.open_stage_canvas_selector(),
            KeyCode::Delete => {
                if self.slot_selected == 0 {
                    return Vec::new();
                }
                let slot_idx = self.slot_selected - 1;
                let Some(agent_id) = self
                    .config
                    .stages
                    .get(self.stage_selected)
                    .and_then(|s| s.agent_ids.get(slot_idx))
                    .cloned()
                else {
                    return Vec::new();
                };
                match self.config.remove_agent(self.stage_selected, &agent_id) {
                    Ok(()) => {
                        let max_slot = self.unlocked_slots(self.stage_selected);
                        self.slot_selected = self.slot_selected.min(max_slot);
                        vec![AppAction::UnassignAgent {
                            stage: self.stage_selected,
                            agent_id,
                        }]
                    }
                    Err(err) => {
                        self.status_message = Some(err.to_string());
                        Vec::new()
                    }
                }
            }
            _ => Vec::new(),
        }
    }

    fn open_stage_canvas_selector(&mut self) -> Vec<AppAction> {
        let stage = self.stage_selected;
        if self.slot_selected == 0 {
            let options = self.roles.iter().map(|r| r.role_id.clone()).collect();
            self.modal = Modal::Selector(Selector {
                stage,
                slot: 0,
                kind: SelectorKind::Role,
                options,
                index: 0,
            });
            return Vec::new();
        }
        let max_slot = self.unlocked_slots(stage);
        if self.slot_selected > max_slot {
            return Vec::new();
        }
        let slot_idx = self.slot_selected - 1;
        let mut options: Vec<String> = self.agents.iter().map(|a| a.agent_id.clone()).collect();
        let holds_agent = self
            .config
            .stages
            .get(stage)
            .is_some_and(|s| slot_idx < s.agent_ids.len());
        if holds_agent {
            options.push(REMOVE_OPTION.to_string());
        }
        self.modal = Modal::Selector(Selector {
            stage,
            slot: slot_idx,
            kind: SelectorKind::Agent,
            options,
            index: 0,
        });
        Vec::new()
    }

    // Row 10: Focus LiveStream
    fn handle_live_stream_key(&mut self, code: KeyCode) -> Vec<AppAction> {
        match code {
            KeyCode::Up => {
                self.stream_scroll = self.stream_scroll.saturating_add(1);
                Vec::new()
            }
            KeyCode::Down => {
                self.stream_scroll = self.stream_scroll.saturating_sub(1);
                Vec::new()
            }
            KeyCode::PageUp => {
                self.stream_scroll = self.stream_scroll.saturating_add(STREAM_PAGE_STEP);
                Vec::new()
            }
            KeyCode::PageDown => {
                self.stream_scroll = self.stream_scroll.saturating_sub(STREAM_PAGE_STEP);
                Vec::new()
            }
            KeyCode::End => {
                self.stream_scroll = 0;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    // Row 11: Focus FooterMenu
    fn handle_footer_menu_key(&mut self, code: KeyCode) -> Vec<AppAction> {
        match code {
            KeyCode::Left => {
                self.footer_index = self.footer_index.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Right => {
                if self.footer_index < 6 {
                    self.footer_index += 1;
                }
                Vec::new()
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.footer_index {
                0 => {
                    if self.is_running() {
                        vec![AppAction::Interrupt]
                    } else {
                        vec![AppAction::Run]
                    }
                }
                1 => {
                    if self.approve_visible() {
                        vec![AppAction::Approve]
                    } else {
                        Vec::new()
                    }
                }
                2 => {
                    if self.approve_visible() {
                        self.modal = Modal::RejectFeedback {
                            input: String::new(),
                        };
                    }
                    Vec::new()
                }
                3 => {
                    self.modal = Modal::Settings;
                    Vec::new()
                }
                4 => {
                    self.modal = Modal::Help;
                    Vec::new()
                }
                5 => self.toggle_mouse_capture(),
                6 => self.activate_quit(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    pub fn keep_selection_visible(&mut self, regions: &UiLayoutRegions) {
        if regions.too_small || regions.stage_columns.is_empty() {
            return;
        }
        let visible_stages = regions.stage_columns.len();
        if self.stage_selected < self.stage_scroll {
            self.stage_scroll = self.stage_selected;
        } else if self.stage_selected >= self.stage_scroll + visible_stages {
            self.stage_scroll = self.stage_selected + 1 - visible_stages;
        }

        if self.slot_selected > 0 {
            let slot_idx = self.slot_selected - 1;
            let visible_slots = regions.stage_columns[0].slots.len();
            if visible_slots > 0 {
                if slot_idx < self.slot_scroll {
                    self.slot_scroll = slot_idx;
                } else if slot_idx >= self.slot_scroll + visible_slots {
                    self.slot_scroll = slot_idx + 1 - visible_slots;
                }
            }
        }
    }

    pub fn handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        regions: &UiLayoutRegions,
    ) -> Vec<AppAction> {
        if !self.mouse_capture || !matches!(self.modal, Modal::None) {
            return Vec::new();
        }
        let Some(region) = regions.region_at(mouse.column, mouse.row) else {
            return Vec::new();
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => match region {
                UiRegion::IngestButton => {
                    self.focus = Focus::TopMenu;
                    self.top_index = 0;
                    self.handle_top_menu_key(KeyCode::Enter)
                }
                UiRegion::ModeToggle => {
                    self.focus = Focus::TopMenu;
                    self.top_index = 1;
                    self.handle_top_menu_key(KeyCode::Enter)
                }
                UiRegion::CyclesControl => {
                    self.focus = Focus::TopMenu;
                    self.top_index = 2;
                    self.handle_top_menu_key(KeyCode::Enter)
                }
                UiRegion::PassedCounter => {
                    self.focus = Focus::TopMenu;
                    self.top_index = 3;
                    self.handle_top_menu_key(KeyCode::Enter)
                }
                UiRegion::StagesMinus => {
                    self.focus = Focus::TopMenu;
                    self.top_index = 4;
                    self.handle_top_menu_key(KeyCode::Enter)
                }
                UiRegion::StagesPlus => {
                    self.focus = Focus::TopMenu;
                    self.top_index = 5;
                    self.handle_top_menu_key(KeyCode::Enter)
                }
                UiRegion::StepsList => {
                    self.focus = Focus::StepsPanel;
                    let inner_y = regions.steps_panel.y.saturating_add(1);
                    let clicked_row = usize::from(mouse.row.saturating_sub(inner_y));
                    let len = self
                        .plan
                        .as_ref()
                        .map_or(0, |p| p.steps.len())
                        .max(self.step_statuses.len());
                    if clicked_row < len {
                        self.step_selected = clicked_row;
                    }
                    self.handle_steps_panel_key(KeyCode::Enter)
                }
                UiRegion::StageHeader(stage) => {
                    if stage < self.config.stages.len() {
                        self.focus = Focus::StageCanvas;
                        self.stage_selected = stage;
                        self.slot_selected = 0;
                        self.handle_stage_canvas_key(KeyCode::Enter)
                    } else {
                        Vec::new()
                    }
                }
                UiRegion::AgentSlot { stage, slot } => {
                    if stage < self.config.stages.len() {
                        self.focus = Focus::StageCanvas;
                        self.stage_selected = stage;
                        let unlocked = self.unlocked_slots(stage);
                        if slot < unlocked {
                            self.slot_selected = slot + 1;
                            self.handle_stage_canvas_key(KeyCode::Enter)
                        } else {
                            self.slot_selected = self.slot_selected.min(unlocked);
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                }
                UiRegion::LiveStream => {
                    self.focus = Focus::LiveStream;
                    self.handle_live_stream_key(KeyCode::Enter)
                }
                UiRegion::RunStopButton => {
                    self.focus = Focus::FooterMenu;
                    self.footer_index = 0;
                    self.handle_footer_menu_key(KeyCode::Enter)
                }
                UiRegion::ApproveButton => {
                    if self.approve_visible() {
                        self.focus = Focus::FooterMenu;
                        self.footer_index = 1;
                        self.handle_footer_menu_key(KeyCode::Enter)
                    } else {
                        Vec::new()
                    }
                }
                UiRegion::RejectButton => {
                    if self.approve_visible() {
                        self.focus = Focus::FooterMenu;
                        self.footer_index = 2;
                        self.handle_footer_menu_key(KeyCode::Enter)
                    } else {
                        Vec::new()
                    }
                }
                UiRegion::SettingsButton => {
                    self.focus = Focus::FooterMenu;
                    self.footer_index = 3;
                    self.handle_footer_menu_key(KeyCode::Enter)
                }
                UiRegion::HelpButton => {
                    self.focus = Focus::FooterMenu;
                    self.footer_index = 4;
                    self.handle_footer_menu_key(KeyCode::Enter)
                }
                UiRegion::MouseToggle => {
                    self.focus = Focus::FooterMenu;
                    self.footer_index = 5;
                    self.handle_footer_menu_key(KeyCode::Enter)
                }
                UiRegion::QuitButton => {
                    self.focus = Focus::FooterMenu;
                    self.footer_index = 6;
                    self.handle_footer_menu_key(KeyCode::Enter)
                }
            },
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let code = if mouse.kind == MouseEventKind::ScrollUp {
                    KeyCode::Up
                } else {
                    KeyCode::Down
                };
                match region {
                    UiRegion::StepsList => {
                        self.focus = Focus::StepsPanel;
                        self.handle_steps_panel_key(code)
                    }
                    UiRegion::LiveStream => {
                        self.focus = Focus::LiveStream;
                        self.handle_live_stream_key(code)
                    }
                    UiRegion::StageHeader(stage) | UiRegion::AgentSlot { stage, .. } => {
                        if stage < self.config.stages.len() {
                            self.focus = Focus::StageCanvas;
                            self.stage_selected = stage;
                            let max_slot = self.unlocked_slots(stage);
                            self.slot_selected = self.slot_selected.min(max_slot);
                            self.handle_stage_canvas_key(code)
                        } else {
                            Vec::new()
                        }
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orrery_core::config::{AgentBackend, RoleKind};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn sample_roles() -> Vec<RoleConfig> {
        vec![
            RoleConfig {
                role_id: "orchestrator".to_string(),
                display_name: "Orchestrator".to_string(),
                kind: RoleKind::Orchestrator,
                rules_file: "roles/orchestrator.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "coder".to_string(),
                display_name: "Coder".to_string(),
                kind: RoleKind::Implementer,
                rules_file: "roles/coder.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "validator".to_string(),
                display_name: "Validator".to_string(),
                kind: RoleKind::Validator,
                rules_file: "roles/validator.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
        ]
    }

    fn sample_agents() -> Vec<AgentConfig> {
        vec![
            AgentConfig {
                agent_id: "orch_a".to_string(),
                display_name: "Orch A".to_string(),
                backend: AgentBackend::LocalModel {
                    inference_id: "inf".to_string(),
                    model: "m".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 4096,
                },
            },
            AgentConfig {
                agent_id: "coder_a".to_string(),
                display_name: "Coder A".to_string(),
                backend: AgentBackend::LocalModel {
                    inference_id: "inf".to_string(),
                    model: "m".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 4096,
                },
            },
            AgentConfig {
                agent_id: "coder_b".to_string(),
                display_name: "Coder B".to_string(),
                backend: AgentBackend::LocalModel {
                    inference_id: "inf".to_string(),
                    model: "m".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 4096,
                },
            },
        ]
    }

    fn make_app() -> App {
        App::new(
            PathBuf::from("."),
            ProjectConfig::default_template("demo"),
            sample_roles(),
            sample_agents(),
        )
    }

    #[test]
    fn ctrl_c_interrupts_only_when_running() {
        let mut app = make_app();
        assert_eq!(app.status, RunStatus::Idle);
        assert!(app.handle_key_event(ctrl('c')).is_empty());

        app.status = RunStatus::Running;
        assert_eq!(app.handle_key_event(ctrl('c')), vec![AppAction::Interrupt]);
    }

    #[test]
    fn other_modifier_chords_do_nothing() {
        let mut app = make_app();
        let before = app.clone();
        for evt in [
            ctrl('q'),
            ctrl('s'),
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL),
        ] {
            let actions = app.handle_key_event(evt);
            assert!(actions.is_empty());
            assert_eq!(app, before);
        }
    }

    #[test]
    fn tab_and_backtab_cycle_focus() {
        let mut app = make_app();
        assert_eq!(app.focus, Focus::TopMenu);

        let expected_forward = [
            Focus::StepsPanel,
            Focus::StageCanvas,
            Focus::LiveStream,
            Focus::FooterMenu,
            Focus::TopMenu,
        ];
        for expected in expected_forward {
            assert!(app.handle_key_event(key(KeyCode::Tab)).is_empty());
            assert_eq!(app.focus, expected);
        }

        let expected_backward = [
            Focus::FooterMenu,
            Focus::LiveStream,
            Focus::StageCanvas,
            Focus::StepsPanel,
            Focus::TopMenu,
        ];
        for expected in expected_backward {
            assert!(app.handle_key_event(key(KeyCode::BackTab)).is_empty());
            assert_eq!(app.focus, expected);
        }
    }

    #[test]
    fn q_quits_when_idle_confirms_when_running() {
        let mut app = make_app();
        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('q'))),
            vec![AppAction::Quit]
        );
        assert!(app.should_quit);
        assert_eq!(app.modal, Modal::None);

        let mut running_app = make_app();
        running_app.status = RunStatus::Running;
        assert!(
            running_app
                .handle_key_event(key(KeyCode::Char('q')))
                .is_empty()
        );
        assert!(!running_app.should_quit);
        assert_eq!(running_app.modal, Modal::QuitConfirm);

        assert!(
            running_app
                .handle_key_event(key(KeyCode::Char('n')))
                .is_empty()
        );
        assert_eq!(running_app.modal, Modal::None);
        assert!(!running_app.should_quit);
    }

    #[test]
    fn quit_confirm_enter_interrupts_and_quits() {
        let mut app = make_app();
        app.status = RunStatus::Running;
        app.handle_key_event(key(KeyCode::Char('q')));
        assert_eq!(app.modal, Modal::QuitConfirm);

        let actions = app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(actions, vec![AppAction::Interrupt, AppAction::Quit]);
        assert!(app.should_quit);
        assert_eq!(app.modal, Modal::None);
    }

    #[test]
    fn question_mark_opens_help_esc_closes() {
        let mut app = make_app();
        assert!(app.handle_key_event(key(KeyCode::Char('?'))).is_empty());
        assert_eq!(app.modal, Modal::Help);

        assert!(app.handle_key_event(key(KeyCode::Esc)).is_empty());
        assert_eq!(app.modal, Modal::None);
    }

    #[test]
    fn m_toggles_mouse_capture() {
        let mut app = make_app();
        assert!(app.mouse_capture);

        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('m'))),
            vec![AppAction::SetMouseCapture(false)]
        );
        assert!(!app.mouse_capture);

        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('m'))),
            vec![AppAction::SetMouseCapture(true)]
        );
        assert!(app.mouse_capture);
    }

    #[test]
    fn text_entry_keeps_q_and_m_as_text() {
        let mut app = make_app();
        app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(
            app.modal,
            Modal::Ingest {
                input: String::new()
            }
        );

        for c in ['q', 'm', '?'] {
            assert!(app.handle_key_event(key(KeyCode::Char(c))).is_empty());
        }
        assert!(!app.should_quit);
        assert!(app.mouse_capture);
        assert_eq!(
            app.modal,
            Modal::Ingest {
                input: "qm?".to_string()
            }
        );

        assert!(app.handle_key_event(key(KeyCode::Backspace)).is_empty());
        assert_eq!(
            app.modal,
            Modal::Ingest {
                input: "qm".to_string()
            }
        );
    }

    #[test]
    fn ingest_enter_emits_path() {
        let mut app = make_app();
        app.handle_key_event(key(KeyCode::Enter));
        for c in "PLAN.md".chars() {
            app.handle_key_event(key(KeyCode::Char(c)));
        }
        let actions = app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![AppAction::IngestPlan(PathBuf::from("PLAN.md"))]
        );
        assert_eq!(app.modal, Modal::None);
    }

    #[test]
    fn mode_toggle_emits_set_mode() {
        let mut app = make_app();
        app.handle_key_event(key(KeyCode::Right));
        assert_eq!(app.top_index, 1);

        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::SetMode(ExecutionMode::Loop)]
        );
        assert_eq!(app.config.execution_mode, ExecutionMode::Loop);

        assert_eq!(
            app.handle_key_event(key(KeyCode::Char(' '))),
            vec![AppAction::SetMode(ExecutionMode::Run)]
        );
        assert_eq!(app.config.execution_mode, ExecutionMode::Run);
    }

    #[test]
    fn cycles_plus_minus_and_dimmed_in_run() {
        let mut app = make_app();
        app.top_index = 2;
        assert_eq!(app.config.execution_mode, ExecutionMode::Run);
        assert!(app.handle_key_event(key(KeyCode::Char('+'))).is_empty());
        assert!(app.handle_key_event(key(KeyCode::Char('-'))).is_empty());
        assert_eq!(app.config.cycle_count, 0);

        app.config.execution_mode = ExecutionMode::Loop;
        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('+'))),
            vec![AppAction::SetCycles(1)]
        );
        assert_eq!(app.config.cycle_count, 1);
        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('+'))),
            vec![AppAction::SetCycles(2)]
        );
        assert_eq!(app.config.cycle_count, 2);
        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('-'))),
            vec![AppAction::SetCycles(1)]
        );
        assert_eq!(app.config.cycle_count, 1);
        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('-'))),
            vec![AppAction::SetCycles(0)]
        );
        assert_eq!(app.config.cycle_count, 0);
        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('-'))),
            vec![AppAction::SetCycles(0)]
        );
        assert_eq!(app.config.cycle_count, 0);
    }

    #[test]
    fn stage_count_clamps_2_to_15() {
        let mut app = make_app();
        app.top_index = 4;
        assert_eq!(app.config.stages.len(), 3);

        assert_eq!(
            app.handle_key_event(key(KeyCode::Char('-'))),
            vec![AppAction::SetStageCount(2)]
        );
        assert_eq!(app.config.stages.len(), 2);

        // At minimum (2), '-' produces no action.
        assert!(app.handle_key_event(key(KeyCode::Char('-'))).is_empty());
        assert_eq!(app.config.stages.len(), 2);

        app.top_index = 5;
        for expected in 3..=15 {
            assert_eq!(
                app.handle_key_event(key(KeyCode::Char('+'))),
                vec![AppAction::SetStageCount(expected)]
            );
            assert_eq!(app.config.stages.len(), expected);
        }

        // At maximum (15), '+' produces no action.
        assert!(app.handle_key_event(key(KeyCode::Char('+'))).is_empty());
        assert_eq!(app.config.stages.len(), 15);
    }

    #[test]
    fn role_selection_unlocks_first_slot() {
        let mut app = make_app();
        app.config.stages[1].role_id.clear();
        assert_eq!(app.unlocked_slots(1), 0);

        app.focus = Focus::StageCanvas;
        app.handle_key_event(key(KeyCode::Right));
        assert_eq!(app.stage_selected, 1);
        assert_eq!(app.slot_selected, 0);

        // Open role selector on stage 1 header and pick "coder" (index 1).
        app.handle_key_event(key(KeyCode::Enter));
        app.handle_key_event(key(KeyCode::Down));
        let actions = app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![AppAction::SetStageRole {
                stage: 1,
                role_id: "coder".to_string()
            }]
        );
        assert_eq!(app.unlocked_slots(1), 1);
    }

    #[test]
    fn orchestrator_has_one_slot() {
        let mut app = make_app();
        assert_eq!(app.unlocked_slots(0), 1);

        app.focus = Focus::StageCanvas;
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.slot_selected, 1);

        // Cannot move past slot 1 on orchestrator.
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.slot_selected, 1);

        // Assign an agent to stage 0 slot 0.
        app.handle_key_event(key(KeyCode::Enter));
        let actions = app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![AppAction::AssignAgent {
                stage: 0,
                agent_id: "orch_a".to_string()
            }]
        );
        assert_eq!(app.unlocked_slots(0), 1);
    }

    #[test]
    fn other_roles_unlock_up_to_15() {
        let mut app = make_app();
        assert_eq!(app.unlocked_slots(1), 1);

        for count in 1..=14 {
            app.config
                .assign_agent(1, format!("coder_{count}"))
                .unwrap();
            assert_eq!(app.unlocked_slots(1), count + 1);
        }

        app.config.assign_agent(1, "coder_15").unwrap();
        assert_eq!(app.unlocked_slots(1), 15);
    }

    #[test]
    fn delete_unassigns_agent() {
        let mut app = make_app();
        app.config.assign_agent(1, "coder_a").unwrap();
        app.config.assign_agent(1, "coder_b").unwrap();

        app.focus = Focus::StageCanvas;
        app.handle_key_event(key(KeyCode::Right));
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.stage_selected, 1);
        assert_eq!(app.slot_selected, 1);

        let actions = app.handle_key_event(key(KeyCode::Delete));
        assert_eq!(
            actions,
            vec![AppAction::UnassignAgent {
                stage: 1,
                agent_id: "coder_a".to_string()
            }]
        );
        assert_eq!(app.config.stages[1].agent_ids, vec!["coder_b".to_string()]);

        // Also test removing via Selector "(remove)" option.
        app.handle_key_event(key(KeyCode::Enter));
        if let Modal::Selector(ref mut sel) = app.modal {
            sel.index = sel.options.len() - 1;
        }
        let actions = app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![AppAction::UnassignAgent {
                stage: 1,
                agent_id: "coder_b".to_string()
            }]
        );
        assert!(app.config.stages[1].agent_ids.is_empty());
    }

    #[test]
    fn invalid_edit_sets_status_message_no_action() {
        let mut app = make_app();
        app.focus = Focus::StageCanvas;
        app.handle_key_event(key(KeyCode::Right));
        app.handle_key_event(key(KeyCode::Right));
        assert_eq!(app.stage_selected, 2);
        assert_eq!(app.slot_selected, 0);

        // Open role selector on stage 2 and select index 0 ("orchestrator").
        app.handle_key_event(key(KeyCode::Enter));
        let actions = app.handle_key_event(key(KeyCode::Enter));
        assert!(actions.is_empty());
        assert!(app.status_message.is_some());
        assert_eq!(app.config.stages[2].role_id, "validator");
    }

    #[test]
    fn live_stream_scroll_and_end_follows() {
        let mut app = make_app();
        for i in 0..25 {
            app.push_stream_event(StreamEvent::notice(format!("line {i}")));
        }
        app.focus = Focus::LiveStream;
        assert_eq!(app.stream_scroll, 0);

        app.handle_key_event(key(KeyCode::Up));
        assert_eq!(app.stream_scroll, 1);

        app.handle_key_event(key(KeyCode::PageUp));
        assert_eq!(app.stream_scroll, 11);

        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.stream_scroll, 10);

        app.handle_key_event(key(KeyCode::PageDown));
        assert_eq!(app.stream_scroll, 0);

        app.handle_key_event(key(KeyCode::PageUp));
        assert!(app.stream_scroll > 0);
        app.handle_key_event(key(KeyCode::End));
        assert_eq!(app.stream_scroll, 0);
    }

    #[test]
    fn approve_hidden_unless_gate_and_operator() {
        let mut app = make_app();
        app.awaiting_operator = true;
        // Default has validator stage + ValidationMode::Auto -> effective Agents.
        assert!(!app.approve_visible());

        app.focus = Focus::FooterMenu;
        app.footer_index = 1;
        assert!(app.handle_key_event(key(KeyCode::Enter)).is_empty());
        app.footer_index = 2;
        assert!(app.handle_key_event(key(KeyCode::Enter)).is_empty());
        assert_eq!(app.modal, Modal::None);

        // Switch to GateAndOperator while awaiting_operator is true.
        app.config.validation_mode = ValidationMode::GateAndOperator;
        assert!(app.approve_visible());

        app.footer_index = 1;
        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::Approve]
        );

        app.footer_index = 2;
        assert!(app.handle_key_event(key(KeyCode::Enter)).is_empty());
        assert_eq!(
            app.modal,
            Modal::RejectFeedback {
                input: String::new()
            }
        );
        for c in "needs fix".chars() {
            app.handle_key_event(key(KeyCode::Char(c)));
        }
        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::Reject("needs fix".to_string())]
        );

        app.awaiting_operator = false;
        assert!(!app.approve_visible());
    }

    #[test]
    fn engine_events_update_status() {
        let mut app = make_app();
        app.step_statuses = vec![StepStatus::Pending, StepStatus::Pending];

        app.apply_engine_event(&EngineEvent::PhaseChanged(EnginePhase::OrchestratorTurn));
        assert_eq!(app.status, RunStatus::Running);
        assert!(!app.awaiting_operator);

        app.apply_engine_event(&EngineEvent::StepStatusChanged {
            step_number: 1,
            status: StepStatus::InProgress,
        });
        assert_eq!(app.step_statuses[0], StepStatus::InProgress);

        app.apply_engine_event(&EngineEvent::AwaitingOperator {
            step_number: 1,
            reason: "gate review".to_string(),
        });
        assert_eq!(app.status, RunStatus::AwaitingOperator);
        assert!(app.awaiting_operator);
        assert_eq!(app.step_statuses[0], StepStatus::AwaitingOperator);

        app.apply_engine_event(&EngineEvent::RoundStarted {
            step_number: 1,
            round: 2,
        });
        assert_eq!(app.status, RunStatus::Running);
        assert!(!app.awaiting_operator);

        app.apply_engine_event(&EngineEvent::RunFinished(RunOutcome::Completed {
            steps_passed: 1,
        }));
        assert_eq!(app.status, RunStatus::Passed);
        assert!(!app.awaiting_operator);

        app.apply_engine_event(&EngineEvent::RunFinished(RunOutcome::Interrupted));
        assert_eq!(app.status, RunStatus::Idle);

        app.apply_engine_event(&EngineEvent::PhaseChanged(EnginePhase::Idle));
        assert_eq!(app.status, RunStatus::Idle);
        app.apply_engine_event(&EngineEvent::PhaseChanged(EnginePhase::AwaitingOperator));
        assert_eq!(app.status, RunStatus::AwaitingOperator);
        assert!(app.awaiting_operator);
        app.apply_engine_event(&EngineEvent::PhaseChanged(EnginePhase::Done));
        assert_eq!(app.status, RunStatus::Passed);
        app.apply_engine_event(&EngineEvent::StepStatusChanged {
            step_number: 2,
            status: StepStatus::Failed,
        });
        assert_eq!(app.status, RunStatus::Error);
        app.apply_engine_event(&EngineEvent::RunFinished(RunOutcome::AwaitingOperator {
            step_number: 2,
            reason: "retry check".to_string(),
        }));
        assert_eq!(app.status, RunStatus::AwaitingOperator);
    }

    #[test]
    fn steps_panel_footer_menu_and_stream_cap_edges() {
        let mut app = make_app();
        app.step_statuses = vec![
            StepStatus::Pending,
            StepStatus::Pending,
            StepStatus::Pending,
        ];
        app.focus = Focus::StepsPanel;
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.step_selected, 1);
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.step_selected, 2);
        app.handle_key_event(key(KeyCode::Down));
        assert_eq!(app.step_selected, 2);
        app.handle_key_event(key(KeyCode::Up));
        assert_eq!(app.step_selected, 1);

        // Stream cap at 2,000 events.
        for i in 0..2_005 {
            app.push_stream_event(StreamEvent::notice(format!("msg {i}")));
        }
        assert_eq!(app.stream.len(), 2_000);
        assert_eq!(app.stream.front().unwrap().text, "msg 5");
        assert_eq!(app.stream.back().unwrap().text, "msg 2004");

        // FooterMenu navigation & actions.
        app.focus = Focus::FooterMenu;
        assert_eq!(app.footer_index, 0);
        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::Run]
        );
        app.status = RunStatus::Running;
        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::Interrupt]
        );
        app.status = RunStatus::Idle;

        app.footer_index = 3;
        app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(app.modal, Modal::Settings);
        app.handle_key_event(key(KeyCode::Char('q')));
        assert_eq!(app.modal, Modal::None);

        app.footer_index = 4;
        app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(app.modal, Modal::Help);
        app.handle_key_event(key(KeyCode::Enter));
        assert_eq!(app.modal, Modal::None);

        app.footer_index = 5;
        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::SetMouseCapture(false)]
        );
        app.handle_key_event(key(KeyCode::Right));
        assert_eq!(app.footer_index, 6);
        assert_eq!(
            app.handle_key_event(key(KeyCode::Enter)),
            vec![AppAction::Quit]
        );
    }

    fn left_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn wheel_up(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn wheel_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn layout_120x40(stage_count: usize) -> UiLayoutRegions {
        UiLayoutRegions::compute(ratatui::layout::Rect::new(0, 0, 120, 40), stage_count, 0, 0)
    }

    fn find_region_rect(regions: &UiLayoutRegions, target: UiRegion) -> ratatui::layout::Rect {
        regions
            .top_items
            .iter()
            .chain(regions.footer_items.iter())
            .find_map(|&(region, rect)| (region == target).then_some(rect))
            .unwrap()
    }

    #[test]
    fn click_run_emits_run() {
        let mut app = make_app();
        let regions = layout_120x40(app.config.stages.len());
        let rect = find_region_rect(&regions, UiRegion::RunStopButton);
        let actions = app.handle_mouse_event(left_click(rect.x, rect.y), &regions);
        assert_eq!(actions, vec![AppAction::Run]);
        assert_eq!(app.focus, Focus::FooterMenu);
        assert_eq!(app.footer_index, 0);
    }

    #[test]
    fn click_ingest_opens_modal() {
        let mut app = make_app();
        let regions = layout_120x40(app.config.stages.len());
        let rect = find_region_rect(&regions, UiRegion::IngestButton);
        let actions = app.handle_mouse_event(left_click(rect.x, rect.y), &regions);
        assert!(actions.is_empty());
        assert_eq!(app.focus, Focus::TopMenu);
        assert_eq!(app.top_index, 0);
        assert_eq!(
            app.modal,
            Modal::Ingest {
                input: String::new()
            }
        );
    }

    #[test]
    fn click_stage_header_opens_role_selector() {
        let mut app = make_app();
        let regions = layout_120x40(app.config.stages.len());
        let col = &regions.stage_columns[1];
        let actions = app.handle_mouse_event(left_click(col.header.x, col.header.y), &regions);
        assert!(actions.is_empty());
        assert_eq!(app.focus, Focus::StageCanvas);
        assert_eq!(app.stage_selected, 1);
        assert_eq!(app.slot_selected, 0);
        assert!(matches!(
            app.modal,
            Modal::Selector(Selector {
                stage: 1,
                slot: 0,
                kind: SelectorKind::Role,
                ..
            })
        ));
    }

    #[test]
    fn click_locked_slot_does_nothing() {
        let mut app = make_app();
        let regions = layout_120x40(app.config.stages.len());
        // Stage 1 has 0 assigned agents -> unlocked_slots(1) == 1 (only slot 0 is unlocked, slot 1 is locked).
        assert_eq!(app.unlocked_slots(1), 1);
        let (_, locked_rect) = regions.stage_columns[1]
            .slots
            .iter()
            .find(|&&(idx, _)| idx == 1)
            .copied()
            .unwrap();
        let actions = app.handle_mouse_event(left_click(locked_rect.x, locked_rect.y), &regions);
        assert!(actions.is_empty());
        assert_eq!(app.modal, Modal::None);

        // Clicking unlocked slot 0 opens Agent selector.
        let (_, unlocked_rect) = regions.stage_columns[1]
            .slots
            .iter()
            .find(|&&(idx, _)| idx == 0)
            .copied()
            .unwrap();
        let actions =
            app.handle_mouse_event(left_click(unlocked_rect.x, unlocked_rect.y), &regions);
        assert!(actions.is_empty());
        assert_eq!(app.slot_selected, 1);
        assert!(matches!(
            app.modal,
            Modal::Selector(Selector {
                stage: 1,
                slot: 0,
                kind: SelectorKind::Agent,
                ..
            })
        ));
    }

    #[test]
    fn wheel_scrolls_live_stream() {
        let mut app = make_app();
        for i in 0..20 {
            app.push_stream_event(StreamEvent::notice(format!("line {i}")));
        }
        let regions = layout_120x40(app.config.stages.len());
        let col = regions.live_stream.x + 1;
        let row = regions.live_stream.y + 1;

        assert!(
            app.handle_mouse_event(wheel_up(col, row), &regions)
                .is_empty()
        );
        assert_eq!(app.focus, Focus::LiveStream);
        assert_eq!(app.stream_scroll, 1);

        assert!(
            app.handle_mouse_event(wheel_up(col, row), &regions)
                .is_empty()
        );
        assert_eq!(app.stream_scroll, 2);

        assert!(
            app.handle_mouse_event(wheel_down(col, row), &regions)
                .is_empty()
        );
        assert_eq!(app.stream_scroll, 1);

        // Also verify wheel over StepsList and StageCanvas column.
        app.step_statuses = vec![StepStatus::Pending, StepStatus::Pending];
        let step_col = regions.steps_panel.x + 1;
        let step_row = regions.steps_panel.y + 1;
        app.handle_mouse_event(wheel_down(step_col, step_row), &regions);
        assert_eq!(app.focus, Focus::StepsPanel);
        assert_eq!(app.step_selected, 1);
        app.handle_mouse_event(wheel_up(step_col, step_row), &regions);
        assert_eq!(app.step_selected, 0);

        let stage_col = regions.stage_columns[1].header.x;
        let stage_row = regions.stage_columns[1].header.y;
        app.handle_mouse_event(wheel_down(stage_col, stage_row), &regions);
        assert_eq!(app.focus, Focus::StageCanvas);
        assert_eq!(app.stage_selected, 1);
        assert_eq!(app.slot_selected, 1);
        app.handle_mouse_event(wheel_up(stage_col, stage_row), &regions);
        assert_eq!(app.slot_selected, 0);
    }

    #[test]
    fn mouse_ignored_when_capture_off() {
        let mut app = make_app();
        app.mouse_capture = false;
        let before = app.clone();
        let regions = layout_120x40(app.config.stages.len());
        let rect = find_region_rect(&regions, UiRegion::RunStopButton);
        let actions = app.handle_mouse_event(left_click(rect.x, rect.y), &regions);
        assert!(actions.is_empty());
        assert_eq!(app, before);
    }

    #[test]
    fn click_hidden_approve_does_nothing() {
        let mut app = make_app();
        let before = app.clone();
        let regions = layout_120x40(app.config.stages.len());
        let approve_rect = find_region_rect(&regions, UiRegion::ApproveButton);
        let reject_rect = find_region_rect(&regions, UiRegion::RejectButton);

        assert!(!app.approve_visible());
        assert!(
            app.handle_mouse_event(left_click(approve_rect.x, approve_rect.y), &regions)
                .is_empty()
        );
        assert!(
            app.handle_mouse_event(left_click(reject_rect.x, reject_rect.y), &regions)
                .is_empty()
        );
        assert_eq!(app, before);

        // When approve_visible() is true, clicking Approve emits Approve and clicking Reject opens modal.
        app.awaiting_operator = true;
        app.config.validation_mode = ValidationMode::GateAndOperator;
        assert!(app.approve_visible());
        assert_eq!(
            app.handle_mouse_event(left_click(approve_rect.x, approve_rect.y), &regions),
            vec![AppAction::Approve]
        );
        assert!(
            app.handle_mouse_event(left_click(reject_rect.x, reject_rect.y), &regions)
                .is_empty()
        );
        assert!(matches!(app.modal, Modal::RejectFeedback { .. }));
    }

    #[test]
    fn click_stage_plus_minus_changes_count() {
        let mut app = make_app();
        assert_eq!(app.config.stages.len(), 3);
        let regions = layout_120x40(app.config.stages.len());
        let plus_rect = find_region_rect(&regions, UiRegion::StagesPlus);
        let minus_rect = find_region_rect(&regions, UiRegion::StagesMinus);

        let actions = app.handle_mouse_event(left_click(plus_rect.x, plus_rect.y), &regions);
        assert_eq!(actions, vec![AppAction::SetStageCount(4)]);
        assert_eq!(app.config.stages.len(), 4);

        let actions = app.handle_mouse_event(left_click(minus_rect.x, minus_rect.y), &regions);
        assert_eq!(actions, vec![AppAction::SetStageCount(3)]);
        assert_eq!(app.config.stages.len(), 3);
    }

    #[test]
    fn keep_selection_visible_scrolls_slots_and_stages() {
        let mut app = make_app();
        for i in 0..8 {
            app.config.assign_agent(1, format!("coder_{i}")).unwrap();
        }
        app.stage_selected = 1;
        app.slot_selected = 8;

        let r_small = UiLayoutRegions::compute(
            ratatui::layout::Rect::new(0, 0, 80, 24),
            app.config.stages.len(),
            app.stage_scroll,
            app.slot_scroll,
        );
        app.keep_selection_visible(&r_small);
        let r_small_after = UiLayoutRegions::compute(
            ratatui::layout::Rect::new(0, 0, 80, 24),
            app.config.stages.len(),
            app.stage_scroll,
            app.slot_scroll,
        );
        assert!(
            r_small_after.stage_columns[1]
                .slots
                .iter()
                .any(|&(idx, _)| idx == 7),
            "slot index 7 should be visible after keep_selection_visible"
        );

        // At 80x24 with 10 stages, scrolling right adjusts stage_scroll, and left scrolls back.
        app.config.set_stage_count(10).unwrap();
        app.stage_scroll = 0;
        app.stage_selected = 5;
        let r_narrow = UiLayoutRegions::compute(
            ratatui::layout::Rect::new(0, 0, 80, 24),
            app.config.stages.len(),
            app.stage_scroll,
            app.slot_scroll,
        );
        app.keep_selection_visible(&r_narrow);
        assert!(app.stage_scroll > 0);
        app.stage_selected = 0;
        app.keep_selection_visible(&r_narrow);
        assert_eq!(app.stage_scroll, 0);

        // At 200x60 with 10 stages and stage_selected = 5 from stage_scroll = 0, stage_scroll stays 0 (all fit).
        app.stage_scroll = 0;
        app.stage_selected = 5;
        let r_wide = UiLayoutRegions::compute(
            ratatui::layout::Rect::new(0, 0, 200, 60),
            app.config.stages.len(),
            app.stage_scroll,
            app.slot_scroll,
        );
        app.keep_selection_visible(&r_wide);
        assert_eq!(app.stage_scroll, 0);
    }
}
