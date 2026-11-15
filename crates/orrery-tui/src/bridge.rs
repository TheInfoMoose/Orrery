use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEvent, MouseEvent,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use orrery_core::checkpoint::{CheckpointManager, OperatorDecision, StepStatus};
use orrery_core::config::ConfigStore;
use orrery_core::engine::{Backends, EngineEvent, RunOptions, RunOutcome, build_engine};
use orrery_core::plan::ImplementationPlan;
use orrery_core::stream_log::{StreamEvent, StreamEventKind};
use orrery_core::{Error, Result};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::app::{App, AppAction, RunStatus};
use crate::layout::UiLayoutRegions;
use crate::widgets::render;

const CHANNEL_CAPACITY: usize = 256;
const TICK_INTERVAL_MS: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeInput {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Engine(EngineEvent),
    Stream(StreamEvent),
    Tick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    pub actions: Vec<AppAction>,
    pub redraw: bool,
    pub quit: bool,
}

pub fn step(app: &mut App, regions: &UiLayoutRegions, input: BridgeInput) -> Effects {
    match input {
        BridgeInput::Key(key) => {
            let actions = app.handle_key_event(key);
            Effects {
                actions,
                redraw: true,
                quit: app.should_quit,
            }
        }
        BridgeInput::Mouse(mouse) => {
            let actions = app.handle_mouse_event(mouse, regions);
            Effects {
                actions,
                redraw: true,
                quit: app.should_quit,
            }
        }
        BridgeInput::Resize(_, _) => Effects {
            actions: Vec::new(),
            redraw: true,
            quit: app.should_quit,
        },
        BridgeInput::Engine(event) => {
            app.apply_engine_event(&event);
            Effects {
                actions: Vec::new(),
                redraw: true,
                quit: app.should_quit,
            }
        }
        BridgeInput::Stream(event) => {
            app.push_stream_event(event);
            Effects {
                actions: Vec::new(),
                redraw: true,
                quit: app.should_quit,
            }
        }
        BridgeInput::Tick => Effects {
            actions: Vec::new(),
            redraw: false,
            quit: app.should_quit,
        },
    }
}

pub struct EngineController {
    store: ConfigStore,
    make_backends: Box<dyn Fn() -> Result<Backends> + Send + Sync>,
    cancel: CancellationToken,
    operator_tx: Option<mpsc::Sender<OperatorDecision>>,
    task: Option<JoinHandle<Result<RunOutcome>>>,
    forward_task: Option<JoinHandle<()>>,
    engine_tx: mpsc::Sender<EngineEvent>,
    engine_rx: mpsc::Receiver<EngineEvent>,
    stream_tx: mpsc::Sender<StreamEvent>,
    stream_rx: mpsc::Receiver<StreamEvent>,
}

impl EngineController {
    pub fn new(
        store: ConfigStore,
        make_backends: Box<dyn Fn() -> Result<Backends> + Send + Sync>,
    ) -> Self {
        let (engine_tx, engine_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (stream_tx, stream_rx) = mpsc::channel(CHANNEL_CAPACITY);
        Self {
            store,
            make_backends,
            cancel: CancellationToken::new(),
            operator_tx: None,
            task: None,
            forward_task: None,
            engine_tx,
            engine_rx,
            stream_tx,
            stream_rx,
        }
    }

    pub async fn next_event(&mut self) -> BridgeInput {
        tokio::select! {
            Some(eng_ev) = self.engine_rx.recv() => BridgeInput::Engine(eng_ev),
            Some(stream_ev) = self.stream_rx.recv() => BridgeInput::Stream(stream_ev),
        }
    }

    pub fn drain_channels(&mut self, app: &mut App, regions: &UiLayoutRegions) {
        while let Ok(ev) = self.engine_rx.try_recv() {
            let _ = step(app, regions, BridgeInput::Engine(ev));
        }
        while let Ok(ev) = self.stream_rx.try_recv() {
            let _ = step(app, regions, BridgeInput::Stream(ev));
        }
    }

    pub async fn poll_finished(&mut self, app: &mut App) -> bool {
        let finished = self.task.as_ref().is_some_and(JoinHandle::is_finished);
        if !finished {
            return false;
        }
        let Some(handle) = self.task.take() else {
            return false;
        };
        if let Some(fwd) = self.forward_task.take() {
            let _ = fwd.await;
        }
        while let Ok(ev) = self.engine_rx.try_recv() {
            app.apply_engine_event(&ev);
        }
        while let Ok(ev) = self.stream_rx.try_recv() {
            app.push_stream_event(ev);
        }
        match handle.await {
            Ok(Ok(_)) => false,
            Ok(Err(err)) => {
                app.status = RunStatus::Error;
                app.awaiting_operator = false;
                app.status_message = Some(err.to_string());
                true
            }
            Err(join_err) => {
                app.status = RunStatus::Error;
                app.awaiting_operator = false;
                app.status_message = Some(join_err.to_string());
                true
            }
        }
    }

    pub async fn handle(&mut self, action: &AppAction, app: &mut App) -> Result<()> {
        match action {
            AppAction::SetMode(_)
            | AppAction::SetCycles(_)
            | AppAction::SetStageCount(_)
            | AppAction::SetStageRole { .. }
            | AppAction::AssignAgent { .. }
            | AppAction::UnassignAgent { .. } => {
                self.store.save_project(&app.config)?;
            }
            AppAction::IngestPlan(path) => {
                let resolved = self.store.resolve_safe_path(path)?;
                let plan = ImplementationPlan::from_file(&resolved, &app.config.step_marker)?;
                let step_count = plan.steps.len();
                app.config.plan_path = path.clone();
                if app.config.step_marker.trim().is_empty() {
                    app.config.step_marker = "## Step ".to_string();
                }
                self.store.save_project(&app.config)?;
                app.plan = Some(plan);
                app.step_statuses = vec![StepStatus::Pending; step_count];
                app.step_selected = 0;
                app.step_scroll = 0;
                app.status_message = Some(format!("Ingested {step_count} steps"));
            }
            AppAction::Run => {
                if let Some(prev) = self.task.take()
                    && !prev.is_finished()
                {
                    self.task = Some(prev);
                    return Ok(());
                }
                if let Some(fwd) = self.forward_task.take() {
                    fwd.abort();
                }

                let backends = (self.make_backends)()?;
                let engine = Arc::new(build_engine(&self.store, backends).await?);

                let mut eng_sub = engine.subscribe();
                let mut stream_sub = engine.subscribe_stream();
                self.operator_tx = Some(engine.operator_sender());
                self.cancel = CancellationToken::new();
                let child_token = self.cancel.child_token();
                let run_done = CancellationToken::new();
                let run_done_fwd = run_done.clone();

                let eng_tx_fwd = self.engine_tx.clone();
                let stream_tx_fwd = self.stream_tx.clone();

                self.forward_task = Some(tokio::spawn(async move {
                    let mut stream_open = true;
                    let mut events_open = true;
                    loop {
                        tokio::select! {
                            biased;
                            res = stream_sub.recv(), if stream_open => {
                                match res {
                                    Ok(sev) => {
                                        let _ = stream_tx_fwd.send(sev).await;
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                        let _ = stream_tx_fwd
                                            .send(StreamEvent::notice(format!("{n} stream events dropped")))
                                            .await;
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                        stream_open = false;
                                    }
                                }
                            }
                            res = eng_sub.recv(), if events_open => {
                                match res {
                                    Ok(ev) => {
                                        let _ = eng_tx_fwd.send(ev).await;
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                        events_open = false;
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                                }
                            }
                            () = run_done_fwd.cancelled() => {
                                drain_remaining(&mut stream_sub, &mut eng_sub, &stream_tx_fwd, &eng_tx_fwd).await;
                                break;
                            }
                        }
                    }
                }));

                app.status = RunStatus::Running;
                app.awaiting_operator = false;

                let stream_tx_task = self.stream_tx.clone();
                let engine_run = Arc::clone(&engine);
                self.task = Some(tokio::spawn(async move {
                    let opts = RunOptions {
                        wait_for_operator: true,
                        ..RunOptions::default()
                    };
                    let res = engine_run.run(opts, child_token).await;
                    if let Err(ref err) = res {
                        let _ = stream_tx_task
                            .send(StreamEvent {
                                kind: StreamEventKind::Error,
                                text: err.to_string(),
                                ..StreamEvent::notice("")
                            })
                            .await;
                    }
                    run_done.cancel();
                    res
                }));
            }
            AppAction::Interrupt => {
                self.cancel.cancel();
                if let Some(handle) = self.task.take() {
                    let _ = handle.await;
                }
                if let Some(fwd) = self.forward_task.take() {
                    let _ = fwd.await;
                }
                while let Ok(ev) = self.engine_rx.try_recv() {
                    app.apply_engine_event(&ev);
                }
                while let Ok(ev) = self.stream_rx.try_recv() {
                    app.push_stream_event(ev);
                }
                app.status = RunStatus::Idle;
                app.awaiting_operator = false;
            }
            AppAction::Approve => {
                if let Some(tx) = &self.operator_tx {
                    let _ = tx.send(OperatorDecision::Approve).await;
                }
            }
            AppAction::Reject(feedback) => {
                if let Some(tx) = &self.operator_tx {
                    let _ = tx
                        .send(OperatorDecision::Reject {
                            feedback: feedback.clone(),
                        })
                        .await;
                }
            }
            AppAction::SetMouseCapture(_) | AppAction::Quit => {}
        }
        Ok(())
    }
}

async fn drain_remaining(
    stream_sub: &mut tokio::sync::broadcast::Receiver<StreamEvent>,
    eng_sub: &mut tokio::sync::broadcast::Receiver<EngineEvent>,
    stream_tx: &mpsc::Sender<StreamEvent>,
    eng_tx: &mpsc::Sender<EngineEvent>,
) {
    loop {
        match stream_sub.try_recv() {
            Ok(sev) => {
                let _ = stream_tx.send(sev).await;
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                let _ = stream_tx
                    .send(StreamEvent::notice(format!("{n} stream events dropped")))
                    .await;
            }
            Err(
                tokio::sync::broadcast::error::TryRecvError::Empty
                | tokio::sync::broadcast::error::TryRecvError::Closed,
            ) => break,
        }
    }
    loop {
        match eng_sub.try_recv() {
            Ok(ev) => {
                let _ = eng_tx.send(ev).await;
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(
                tokio::sync::broadcast::error::TryRecvError::Empty
                | tokio::sync::broadcast::error::TryRecvError::Closed,
            ) => break,
        }
    }
}

struct TerminalGuard {
    mouse_capture: bool,
}

impl TerminalGuard {
    fn enter(mouse_capture: bool) -> Result<Self> {
        enable_raw_mode().map_err(|source| Error::Io {
            path: PathBuf::from("<terminal>"),
            source,
        })?;
        let mut stdout = std::io::stdout();
        let res = if mouse_capture {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        } else {
            execute!(stdout, EnterAlternateScreen)
        };
        if let Err(source) = res {
            let _ = disable_raw_mode();
            return Err(Error::Io {
                path: PathBuf::from("<terminal>"),
                source,
            });
        }
        Ok(Self { mouse_capture })
    }

    fn set_mouse_capture(&mut self, enable: bool) -> Result<()> {
        if self.mouse_capture == enable {
            return Ok(());
        }
        let mut stdout = std::io::stdout();
        let res = if enable {
            execute!(stdout, EnableMouseCapture)
        } else {
            execute!(stdout, DisableMouseCapture)
        };
        res.map_err(|source| Error::Io {
            path: PathBuf::from("<terminal>"),
            source,
        })?;
        self.mouse_capture = enable;
        Ok(())
    }

    fn restore_terminal() {
        let mut stdout = std::io::stdout();
        let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        Self::restore_terminal();
    }
}

fn compute_regions_with_selection(area: Rect, app: &mut App) -> UiLayoutRegions {
    let mut regions = UiLayoutRegions::compute(
        area,
        app.config.stages.len(),
        app.stage_scroll,
        app.slot_scroll,
    );
    let prev_stage_scroll = app.stage_scroll;
    let prev_slot_scroll = app.slot_scroll;
    app.keep_selection_visible(&regions);
    if app.stage_scroll != prev_stage_scroll || app.slot_scroll != prev_slot_scroll {
        regions = UiLayoutRegions::compute(
            area,
            app.config.stages.len(),
            app.stage_scroll,
            app.slot_scroll,
        );
    }
    regions
}

fn load_initial_app(store: &ConfigStore, project_root: &Path) -> Result<App> {
    let config = store.load_project()?;
    let roles = store.load_roles()?;
    let agents = store.load_agents()?;
    let mcp_servers = store.load_mcp_servers()?;
    let mut app = App::new(project_root.to_path_buf(), config, roles, agents);
    app.mcp_servers = mcp_servers;

    if let Ok(plan_path) = store.resolve_safe_path(&app.config.plan_path)
        && let Ok(plan) = ImplementationPlan::from_file(&plan_path, &app.config.step_marker)
    {
        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let statuses = match checkpoints.load_state() {
            Ok(Some(cp)) if cp.step_statuses.len() == plan.steps.len() => cp.step_statuses,
            _ => vec![StepStatus::Pending; plan.steps.len()],
        };
        app.step_statuses = statuses;
        app.plan = Some(plan);
    }

    Ok(app)
}

pub async fn run_tui(project_root: PathBuf) -> Result<()> {
    let store = ConfigStore::new(&project_root, None)?;
    let mut app = load_initial_app(&store, store.project_root())?;
    let root_for_backends = store.project_root().to_path_buf();
    let mut controller =
        EngineController::new(store, Box::new(move || Backends::real(&root_for_backends)));

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        TerminalGuard::restore_terminal();
        prev_hook(info);
    }));

    let mut guard = TerminalGuard::enter(app.mouse_capture)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|source| Error::Io {
        path: PathBuf::from("<terminal>"),
        source,
    })?;

    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(TICK_INTERVAL_MS));
    let mut needs_redraw = true;

    let size = terminal.size().map_err(|source| Error::Io {
        path: PathBuf::from("<terminal>"),
        source,
    })?;
    let mut area = Rect::new(0, 0, size.width, size.height);
    let mut regions = compute_regions_with_selection(area, &mut app);

    loop {
        if needs_redraw {
            regions = compute_regions_with_selection(area, &mut app);
            terminal
                .draw(|frame| render(frame, &app, &regions))
                .map_err(|source| Error::Io {
                    path: PathBuf::from("<terminal>"),
                    source,
                })?;
            needs_redraw = false;
        }

        if app.should_quit {
            break;
        }

        let maybe_input = tokio::select! {
            maybe_evt = event_stream.next() => {
                match maybe_evt {
                    Some(Ok(Event::Key(key))) => Some(BridgeInput::Key(key)),
                    Some(Ok(Event::Mouse(mouse))) => Some(BridgeInput::Mouse(mouse)),
                    Some(Ok(Event::Resize(w, h))) => {
                        area = Rect::new(0, 0, w, h);
                        Some(BridgeInput::Resize(w, h))
                    }
                    Some(Ok(_)) => None,
                    Some(Err(source)) => {
                        return Err(Error::Io {
                            path: PathBuf::from("<terminal>"),
                            source,
                        });
                    }
                    None => break,
                }
            }
            ctrl_input = controller.next_event() => {
                Some(ctrl_input)
            }
            _ = tick_interval.tick() => {
                Some(BridgeInput::Tick)
            }
        };

        let Some(input) = maybe_input else {
            continue;
        };

        let effects = step(&mut app, &regions, input);
        if effects.redraw {
            needs_redraw = true;
        }

        for action in &effects.actions {
            if let AppAction::SetMouseCapture(enable) = action {
                guard.set_mouse_capture(*enable)?;
            }
            if let Err(err) = controller.handle(action, &mut app).await {
                app.status_message = Some(err.to_string());
                needs_redraw = true;
            }
        }

        if controller.poll_finished(&mut app).await {
            needs_redraw = true;
        }

        if effects.quit || app.should_quit {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use orrery_core::checkpoint::{
        AgentReport, CheckpointManager, EnginePhase, ReportStatus, StepStatus, SubtaskAllocation,
    };
    use orrery_core::config::{
        AgentBackend, AgentConfig, ConfigStore, ExecutionMode, InferenceConfig, InferenceLifecycle,
        InferenceMode, ProjectConfig, RoleConfig, RoleKind, StageConfig, ValidationMode,
    };
    use orrery_core::docker::{MockContainerBackend, MockInferenceControl};
    use orrery_core::engine::{Backends, DriverSet, EngineEvent};
    use orrery_core::llm::{AgentDriver, MockAgentDriver};
    use orrery_core::mcp::MockMcpConnector;
    use orrery_core::security::InMemorySecretStore;
    use orrery_core::stream_log::{StreamEvent, StreamEventKind};
    use ratatui::layout::Rect;
    use tempfile::TempDir;

    use super::*;

    fn sample_app(root: &Path) -> App {
        let mut config = ProjectConfig::default_template("demo");
        config.stages = vec![
            StageConfig {
                role_id: "orchestrator".into(),
                agent_ids: vec!["orch".into()],
            },
            StageConfig {
                role_id: "coder".into(),
                agent_ids: vec!["coder_1".into()],
            },
            StageConfig {
                role_id: "validator".into(),
                agent_ids: vec!["val_1".into()],
            },
        ];
        let roles = vec![
            RoleConfig {
                role_id: "orchestrator".into(),
                display_name: "Orchestrator".into(),
                kind: RoleKind::Orchestrator,
                rules_file: "roles/orchestrator.md".into(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "coder".into(),
                display_name: "Coder".into(),
                kind: RoleKind::Implementer,
                rules_file: "roles/coder.md".into(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "validator".into(),
                display_name: "Validator".into(),
                kind: RoleKind::Validator,
                rules_file: "roles/validator.md".into(),
                allowed_mcp_tools: Vec::new(),
            },
        ];
        let agents = vec![
            AgentConfig {
                agent_id: "orch".into(),
                display_name: "Orch".into(),
                backend: AgentBackend::LocalModel {
                    inference_id: "inf".into(),
                    model: "m".into(),
                    temperature: 0.0,
                    max_context_tokens: 4096,
                },
            },
            AgentConfig {
                agent_id: "coder_1".into(),
                display_name: "Coder 1".into(),
                backend: AgentBackend::LocalModel {
                    inference_id: "inf".into(),
                    model: "m".into(),
                    temperature: 0.0,
                    max_context_tokens: 4096,
                },
            },
            AgentConfig {
                agent_id: "val_1".into(),
                display_name: "Validator 1".into(),
                backend: AgentBackend::LocalModel {
                    inference_id: "inf".into(),
                    model: "m".into(),
                    temperature: 0.0,
                    max_context_tokens: 4096,
                },
            },
        ];
        App::new(root.to_path_buf(), config, roles, agents)
    }

    fn setup_project_dir(validation_mode: ValidationMode) -> (TempDir, ConfigStore, App) {
        let tmp = TempDir::new().expect("tempdir");
        let global_dir = tmp.path().join("global");
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        std::fs::create_dir_all(&global_dir).expect("create global dir");

        let store = ConfigStore::new(&project_dir, Some(global_dir)).expect("config store");
        store.init_project("demo").expect("init project");

        let plan_md = "## Step 1: First\nbody 1\n";
        std::fs::write(project_dir.join("PLAN.md"), plan_md).expect("write plan");

        let mut app = sample_app(&project_dir);
        app.config.plan_path = PathBuf::from("PLAN.md");
        app.config.step_marker = "## Step ".into();
        app.config.validation_mode = validation_mode;
        if validation_mode == ValidationMode::GateAndOperator {
            app.config.stages.truncate(2);
        }
        app.config.inference = vec![InferenceConfig {
            inference_id: "inf".into(),
            endpoint_url: "http://192.0.2.1:8080".into(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: false,
            startup_timeout_secs: 10,
        }];
        store.save_project(&app.config).expect("save project");
        for role in &app.roles {
            store.save_role(role, "# rules\n").expect("save role");
        }
        for agent in &app.agents {
            store.save_agent(agent).expect("save agent");
        }
        store
            .save_inference(&app.config.inference[0])
            .expect("save inference");

        (tmp, store, app)
    }

    fn make_mock_backends(chat_driver: Arc<dyn AgentDriver>) -> Backends {
        let cli_driver: Arc<dyn AgentDriver> = Arc::new(MockAgentDriver::new());
        Backends {
            secrets: Arc::new(InMemorySecretStore::new()),
            containers: Arc::new(MockContainerBackend::new()),
            inference_control: Arc::new(MockInferenceControl::new()),
            connector: Arc::new(MockMcpConnector::default()),
            drivers: DriverSet {
                chat: chat_driver,
                cli: cli_driver,
            },
        }
    }

    #[tokio::test]
    async fn step_key_returns_actions() {
        let tmp = TempDir::new().expect("tempdir");
        let mut app = sample_app(tmp.path());
        let regions = UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 3, 0, 0);

        let eff_right = step(
            &mut app,
            &regions,
            BridgeInput::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        );
        assert!(eff_right.actions.is_empty());
        assert!(eff_right.redraw);
        assert!(!eff_right.quit);

        let eff_enter = step(
            &mut app,
            &regions,
            BridgeInput::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        );
        assert_eq!(
            eff_enter.actions,
            vec![AppAction::SetMode(ExecutionMode::Loop)]
        );
        assert!(eff_enter.redraw);
        assert!(!eff_enter.quit);

        let eff_tick = step(&mut app, &regions, BridgeInput::Tick);
        assert!(eff_tick.actions.is_empty());
        assert!(!eff_tick.redraw);
        assert!(!eff_tick.quit);

        let eff_resize = step(&mut app, &regions, BridgeInput::Resize(120, 40));
        assert!(eff_resize.redraw);

        let eff_quit = step(
            &mut app,
            &regions,
            BridgeInput::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
        );
        assert_eq!(eff_quit.actions, vec![AppAction::Quit]);
        assert!(eff_quit.redraw);
        assert!(eff_quit.quit);
    }

    #[tokio::test]
    async fn step_engine_event_updates_app() {
        let tmp = TempDir::new().expect("tempdir");
        let mut app = sample_app(tmp.path());
        app.step_statuses = vec![StepStatus::Pending];
        let regions = UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 3, 0, 0);

        let eff = step(
            &mut app,
            &regions,
            BridgeInput::Engine(EngineEvent::PhaseChanged(EnginePhase::OrchestratorTurn)),
        );
        assert!(eff.redraw);
        assert_eq!(app.status, RunStatus::Running);

        let eff2 = step(
            &mut app,
            &regions,
            BridgeInput::Engine(EngineEvent::StepStatusChanged {
                step_number: 1,
                status: StepStatus::Passed,
            }),
        );
        assert!(eff2.redraw);
        assert_eq!(app.step_statuses[0], StepStatus::Passed);
    }

    #[tokio::test]
    async fn step_stream_event_appends() {
        let tmp = TempDir::new().expect("tempdir");
        let mut app = sample_app(tmp.path());
        let regions = UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 3, 0, 0);

        let event = StreamEvent::notice("hello from stream");
        let eff = step(&mut app, &regions, BridgeInput::Stream(event.clone()));
        assert!(eff.redraw);
        assert_eq!(app.stream.len(), 1);
        assert_eq!(app.stream.back(), Some(&event));
    }

    #[tokio::test]
    async fn run_action_starts_engine_status_running() {
        let (_tmp, store, mut app) = setup_project_dir(ValidationMode::GateAndOperator);
        let driver = Arc::new(MockAgentDriver::new());
        driver.set_delay(Duration::from_secs(60));
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::StepPassed,
                summary: "passed".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        let driver_for_factory = Arc::clone(&driver);

        let mut controller = EngineController::new(
            store,
            Box::new(move || Ok(make_mock_backends(Arc::clone(&driver_for_factory) as _))),
        );

        controller
            .handle(&AppAction::Run, &mut app)
            .await
            .expect("handle Run");
        assert_eq!(app.status, RunStatus::Running);

        controller
            .handle(&AppAction::Interrupt, &mut app)
            .await
            .expect("clean up running task");
    }

    #[tokio::test]
    async fn interrupt_cancels_and_saves_checkpoint() {
        let (_tmp, store, mut app) = setup_project_dir(ValidationMode::GateAndOperator);
        let regions = UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 2, 0, 0);
        let driver = Arc::new(MockAgentDriver::new());
        driver.set_delay(Duration::from_secs(60));
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::StepPassed,
                summary: "passed".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        let driver_for_factory = Arc::clone(&driver);

        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let mut controller = EngineController::new(
            store,
            Box::new(move || Ok(make_mock_backends(Arc::clone(&driver_for_factory) as _))),
        );

        controller
            .handle(&AppAction::Run, &mut app)
            .await
            .expect("handle Run");

        loop {
            let input = tokio::time::timeout(Duration::from_secs(5), controller.next_event())
                .await
                .expect("timed out");
            let is_round = matches!(input, BridgeInput::Engine(EngineEvent::RoundStarted { .. }));
            let _ = step(&mut app, &regions, input);
            if is_round {
                break;
            }
        }

        controller
            .handle(&AppAction::Interrupt, &mut app)
            .await
            .expect("handle Interrupt");

        assert_eq!(app.status, RunStatus::Idle);
        let cp = checkpoints
            .load_state()
            .expect("load checkpoint")
            .expect("checkpoint exists");
        assert_eq!(cp.step_statuses[0], StepStatus::InProgress);
    }

    #[tokio::test]
    async fn approve_reaches_operator_channel() {
        let (_tmp, store, mut app) = setup_project_dir(ValidationMode::GateAndOperator);
        app.step_statuses = vec![StepStatus::Pending];
        let regions = UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 2, 0, 0);

        let driver = Arc::new(MockAgentDriver::new());
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::Dispatch,
                summary: "dispatch to coder".into(),
                target_stage: Some(1),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "coder_1".into(),
                    instructions: "do step 1".into(),
                    write_scope: Vec::new(),
                }],
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        driver.push(
            "coder_1",
            Ok(AgentReport {
                status: ReportStatus::Complete,
                summary: "implemented step 1".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::StepPassed,
                summary: "step 1 ready for operator approval".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        let driver_for_factory = Arc::clone(&driver);

        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let mut controller = EngineController::new(
            store,
            Box::new(move || Ok(make_mock_backends(Arc::clone(&driver_for_factory) as _))),
        );

        controller
            .handle(&AppAction::Run, &mut app)
            .await
            .expect("handle Run");

        loop {
            let input = tokio::time::timeout(Duration::from_secs(5), controller.next_event())
                .await
                .expect("timed out");
            let is_awaiting = matches!(
                input,
                BridgeInput::Engine(EngineEvent::AwaitingOperator { .. })
            );
            let _ = step(&mut app, &regions, input);
            if is_awaiting {
                break;
            }
        }
        assert_eq!(app.status, RunStatus::AwaitingOperator);

        controller
            .handle(&AppAction::Approve, &mut app)
            .await
            .expect("handle Approve");

        loop {
            let input = tokio::time::timeout(Duration::from_secs(5), controller.next_event())
                .await
                .expect("timed out");
            let is_finished = matches!(input, BridgeInput::Engine(EngineEvent::RunFinished(_)));
            let _ = step(&mut app, &regions, input);
            if is_finished {
                break;
            }
        }
        controller.drain_channels(&mut app, &regions);
        assert!(
            !tokio::time::timeout(Duration::from_secs(5), controller.poll_finished(&mut app))
                .await
                .expect("timed out")
        );

        assert_eq!(app.step_statuses[0], StepStatus::Passed);
        let cp = checkpoints
            .load_state()
            .expect("load state")
            .expect("state exists");
        assert_eq!(cp.step_statuses[0], StepStatus::Passed);
    }

    #[tokio::test]
    async fn config_edit_persists() {
        let (_tmp, store, mut app) = setup_project_dir(ValidationMode::Auto);
        let verify_store = store.clone();
        let driver = Arc::new(MockAgentDriver::new());
        let mut controller = EngineController::new(
            store,
            Box::new(move || Ok(make_mock_backends(Arc::clone(&driver) as _))),
        );

        app.config.set_stage_count(4).expect("set 4 stages");
        controller
            .handle(&AppAction::SetStageCount(4), &mut app)
            .await
            .expect("persist SetStageCount(4)");

        let reloaded = verify_store.load_project().expect("reload project");
        assert_eq!(reloaded.stages.len(), 4);

        controller
            .handle(&AppAction::IngestPlan(PathBuf::from("PLAN.md")), &mut app)
            .await
            .expect("ingest PLAN.md");
        assert_eq!(app.step_statuses.len(), 1);
        assert!(app.plan.is_some());

        // Also test load_initial_app, compute_regions_with_selection, Reject, SetMouseCapture, Quit.
        let loaded_app =
            load_initial_app(&verify_store, verify_store.project_root()).expect("load_initial_app");
        assert!(loaded_app.plan.is_some());
        assert_eq!(loaded_app.step_statuses.len(), 1);

        app.stage_selected = 3;
        app.slot_selected = 1;
        let _ = compute_regions_with_selection(Rect::new(0, 0, 80, 24), &mut app);

        let mouse_eff = step(
            &mut app,
            &UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 4, 0, 0),
            BridgeInput::Mouse(crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Moved,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }),
        );
        assert!(mouse_eff.redraw);

        controller
            .handle(&AppAction::Reject("nope".into()), &mut app)
            .await
            .expect("reject with no active channel");
        controller
            .handle(&AppAction::SetMouseCapture(false), &mut app)
            .await
            .expect("set mouse capture");
        controller
            .handle(&AppAction::Quit, &mut app)
            .await
            .expect("quit");
    }

    #[tokio::test]
    async fn run_error_sets_error_status() {
        let (_tmp, store, mut app) = setup_project_dir(ValidationMode::GateAndOperator);
        app.step_statuses = vec![StepStatus::Pending];
        let driver = Arc::new(MockAgentDriver::new());
        driver.push(
            "orch",
            Err(Error::Agent {
                agent_id: "orch".into(),
                message: "backend boom".into(),
            }),
        );
        let driver_for_factory = Arc::clone(&driver);

        let mut controller = EngineController::new(
            store,
            Box::new(move || Ok(make_mock_backends(Arc::clone(&driver_for_factory) as _))),
        );

        controller
            .handle(&AppAction::Run, &mut app)
            .await
            .expect("handle Run");

        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if controller.poll_finished(&mut app).await {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("timed out");

        assert_eq!(app.status, RunStatus::Error);
        assert!(!app.awaiting_operator);
        assert!(
            app.status_message
                .as_deref()
                .is_some_and(|m| m.contains("backend boom")),
            "expected error status_message, got {:?}",
            app.status_message
        );
        assert_eq!(app.step_statuses[0], StepStatus::InProgress);
        assert!(!app.step_statuses.contains(&StepStatus::Failed));
        assert!(
            app.stream
                .iter()
                .any(|e| e.kind == StreamEventKind::Error && e.text.contains("backend boom"))
        );
    }

    #[tokio::test]
    async fn run_forwards_hub_stream_events() {
        let (_tmp, store, mut app) = setup_project_dir(ValidationMode::Auto);
        app.step_statuses = vec![StepStatus::Pending];
        let regions = UiLayoutRegions::compute(Rect::new(0, 0, 100, 30), 3, 0, 0);

        let driver = Arc::new(MockAgentDriver::new());
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::Dispatch,
                summary: "dispatch to coder".into(),
                target_stage: Some(1),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "coder_1".into(),
                    instructions: "do step 1".into(),
                    write_scope: Vec::new(),
                }],
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        driver.push(
            "coder_1",
            Ok(AgentReport {
                status: ReportStatus::Complete,
                summary: "implemented step 1".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::Dispatch,
                summary: "dispatch to validator".into(),
                target_stage: Some(2),
                allocations: vec![SubtaskAllocation {
                    target_agent_id: "val_1".into(),
                    instructions: "verify step 1".into(),
                    write_scope: Vec::new(),
                }],
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        driver.push(
            "val_1",
            Ok(AgentReport {
                status: ReportStatus::Confirmed,
                summary: "verified step 1".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        driver.push(
            "orch",
            Ok(AgentReport {
                status: ReportStatus::StepPassed,
                summary: "step 1 passed".into(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: None,
                artifacts: Vec::new(),
            }),
        );
        let driver_for_factory = Arc::clone(&driver);

        let mut controller = EngineController::new(
            store,
            Box::new(move || Ok(make_mock_backends(Arc::clone(&driver_for_factory) as _))),
        );

        controller
            .handle(&AppAction::Run, &mut app)
            .await
            .expect("handle Run");

        loop {
            let input = tokio::time::timeout(Duration::from_secs(5), controller.next_event())
                .await
                .expect("timed out");
            let is_finished = matches!(input, BridgeInput::Engine(EngineEvent::RunFinished(_)));
            let _ = step(&mut app, &regions, input);
            if is_finished {
                break;
            }
        }
        controller.drain_channels(&mut app, &regions);

        assert!(
            app.stream
                .iter()
                .any(|e| e.agent_id.is_some() || e.kind != StreamEventKind::SystemNotice),
            "expected forwarded hub stream events with agent_id or non-SystemNotice kind, got {:?}",
            app.stream
        );
    }
}
