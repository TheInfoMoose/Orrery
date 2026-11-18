use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use orrery_core::checkpoint::{
    CheckpointManager, EnginePhase, OperatorDecision, StepStatus, WorkflowCheckpoint,
};
use orrery_core::config::{
    AgentBackend, AgentConfig, ConfigScope, ConfigStore, ExecutionMode, HookConfig, HookEvent,
    HookFailure, InferenceConfig, InferenceLifecycle, InferenceMode, McpServerConfig,
    McpTransportConfig, ProjectConfig, RoleConfig, RoleKind, WorkspaceMount, WriteToolSpec,
    resolve_compose_file,
};
use orrery_core::engine::{Backends, RunOptions, RunOutcome, build_engine};
use orrery_core::plan::ImplementationPlan;
use orrery_core::security::{KeyringBackend, OsKeyringStore, SecretStore, SecretValue};
use orrery_core::stream_log::{LogFollower, StreamEvent, tail_log};
use orrery_core::{Error, Result};
use tokio_util::sync::CancellationToken;

use crate::args::{
    AgentCmd, Command, GateCmd, HookCmd, InferenceCmd, McpCmd, RoleCmd, SecretCmd, WorkflowCmd,
};

pub type ReadSecretFn = Box<dyn FnMut() -> Result<String>>;
pub type MakeBackendsFn = Box<dyn Fn(&Path) -> Result<Backends> + Send + Sync>;

pub struct CliContext {
    pub out: Box<dyn Write>,
    pub err: Box<dyn Write>,
    pub secrets: Arc<dyn SecretStore>,
    pub read_secret: ReadSecretFn,
    pub make_backends: MakeBackendsFn,
}

pub fn render_error(err: &Error) -> String {
    let mut out = err.to_string();
    let mut current = std::error::Error::source(err);
    while let Some(source) = current {
        out.push_str(&format!(": {source}"));
        current = source.source();
    }
    out
}

pub fn exit_code_for(err: &Error) -> u8 {
    match err {
        Error::Interrupted => 130,
        _ => 1,
    }
}

fn write_stdout_line(ctx: &mut CliContext, line: &str) -> Result<()> {
    writeln!(ctx.out, "{line}").map_err(|source| Error::Io {
        path: PathBuf::from("<stdout>"),
        source,
    })
}

pub async fn dispatch(command: Command, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    match command {
        Command::Tui { path } => {
            let root = if path == Path::new(".") {
                project.to_path_buf()
            } else {
                path
            };
            orrery_tui::run_tui(root).await?;
            Ok(0)
        }
        Command::Init { path, name } => {
            let target = path.as_deref().unwrap_or(project);
            let store = ConfigStore::new(target, None)?;
            let project_name = match name {
                Some(n) => n,
                None => store
                    .project_root()
                    .file_name()
                    .and_then(|s| s.to_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("orrery-project")
                    .to_string(),
            };
            let cfg = store.init_project(project_name)?;
            write_stdout_line(
                ctx,
                &format!(
                    "Initialized project '{}' in {}",
                    cfg.project_name,
                    store.orrery_dir().display()
                ),
            )?;
            Ok(0)
        }
        Command::Ingest { md_file, marker } => {
            let store = ConfigStore::new(project, None)?;
            let mut cfg = store.load_project()?;
            let step_marker = marker.unwrap_or_else(|| cfg.step_marker.clone());
            let resolved = store.resolve_safe_path(&md_file)?;
            let plan = ImplementationPlan::from_file(&resolved, &step_marker)?;
            cfg.plan_path = md_file;
            cfg.step_marker = step_marker;
            store.save_project(&cfg)?;
            for step in &plan.steps {
                write_stdout_line(ctx, &step.title)?;
            }
            Ok(0)
        }
        Command::Workflow { command } => dispatch_workflow(command, project, ctx),
        Command::Agent { command } => dispatch_agent(command, project, ctx),
        Command::Inference { command } => dispatch_inference(command, project, ctx),
        Command::Secret { command } => dispatch_secret(command, ctx),
        Command::Role { command } => dispatch_role(command, project, ctx),
        Command::Mcp { command } => dispatch_mcp(command, project, ctx).await,
        Command::Gate { command } => dispatch_gate(command, project, ctx),
        Command::Hook { command } => dispatch_hook(command, project, ctx),
        Command::Run {
            r#loop,
            cycles,
            step,
        } => {
            let opts = RunOptions {
                mode: if r#loop {
                    Some(ExecutionMode::Loop)
                } else {
                    None
                },
                cycles,
                start_step: step,
                wait_for_operator: false,
            };
            dispatch_run(opts, project, ctx).await
        }
        Command::Approve => dispatch_operator_decision(OperatorDecision::Approve, project, ctx),
        Command::Reject { feedback } => {
            dispatch_operator_decision(OperatorDecision::Reject { feedback }, project, ctx)
        }
        Command::Resume => {
            let opts = RunOptions {
                wait_for_operator: false,
                ..RunOptions::default()
            };
            dispatch_run(opts, project, ctx).await
        }
        Command::Reset { step } => dispatch_reset(step, project, ctx),
        Command::Status { json } => dispatch_status(json, project, ctx),
        Command::Logs { tail, follow } => dispatch_logs(tail, follow, project, ctx).await,
    }
}

fn dispatch_workflow(command: WorkflowCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        WorkflowCmd::Show { json } => {
            let cfg = store.load_project()?;
            let roles = store.load_roles()?;
            let agents = store.load_agents()?;
            let warnings = cfg.validate(&roles, &agents)?;
            let effective_mode = cfg.effective_validation_mode(&roles);

            if json {
                let stages_json: Vec<serde_json::Value> = cfg
                    .stages
                    .iter()
                    .enumerate()
                    .map(|(index, stage)| {
                        serde_json::json!({
                            "index": index,
                            "role_id": stage.role_id,
                            "agent_ids": stage.agent_ids,
                        })
                    })
                    .collect();
                let warnings_json: Vec<String> = warnings.iter().map(ToString::to_string).collect();
                let payload = serde_json::json!({
                    "validation_mode": effective_mode,
                    "stages": stages_json,
                    "warnings": warnings_json,
                });
                let rendered = serde_json::to_string_pretty(&payload)?;
                write_stdout_line(ctx, &rendered)?;
            } else {
                write_stdout_line(ctx, &format!("Validation mode: {effective_mode:?}"))?;
                for (index, stage) in cfg.stages.iter().enumerate() {
                    let agents_str = if stage.agent_ids.is_empty() {
                        "(none)".to_string()
                    } else {
                        stage.agent_ids.join(", ")
                    };
                    write_stdout_line(
                        ctx,
                        &format!(
                            "Stage {index}: role={} agents=[{agents_str}]",
                            stage.role_id
                        ),
                    )?;
                }
                for warning in &warnings {
                    write_stdout_line(ctx, &format!("Warning: {warning}"))?;
                }
            }
            Ok(0)
        }
        WorkflowCmd::Stages { count } => {
            edit_project(&store, |cfg| cfg.set_stage_count(usize::from(count)))?;
            write_stdout_line(ctx, &format!("Set stage count to {count}"))?;
            Ok(0)
        }
        WorkflowCmd::SetRole { stage, role_id } => {
            edit_project(&store, |cfg| cfg.set_stage_role(stage, &role_id))?;
            write_stdout_line(ctx, &format!("Set stage {stage} role to '{role_id}'"))?;
            Ok(0)
        }
        WorkflowCmd::Assign { stage, agent_id } => {
            edit_project(&store, |cfg| cfg.assign_agent(stage, &agent_id))?;
            write_stdout_line(
                ctx,
                &format!("Assigned agent '{agent_id}' to stage {stage}"),
            )?;
            Ok(0)
        }
        WorkflowCmd::Unassign { stage, agent_id } => {
            edit_project(&store, |cfg| cfg.remove_agent(stage, &agent_id))?;
            write_stdout_line(
                ctx,
                &format!("Unassigned agent '{agent_id}' from stage {stage}"),
            )?;
            Ok(0)
        }
        WorkflowCmd::Clear { stage } => {
            edit_project(&store, |cfg| cfg.clear_stage(stage))?;
            write_stdout_line(ctx, &format!("Cleared agents from stage {stage}"))?;
            Ok(0)
        }
        WorkflowCmd::Mode { mode, cycles } => {
            let exec_mode = ExecutionMode::from(mode);
            edit_project(&store, |cfg| {
                cfg.execution_mode = exec_mode;
                if let Some(c) = cycles {
                    cfg.cycle_count = c;
                }
                Ok(())
            })?;
            write_stdout_line(ctx, &format!("Set execution mode to {exec_mode:?}"))?;
            Ok(0)
        }
    }
}

fn dispatch_gate(command: GateCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        GateCmd::Set { tools } => {
            edit_project(&store, |cfg| {
                cfg.gate_tools = tools;
                Ok(())
            })?;
            write_stdout_line(ctx, "Updated gate tools")?;
            Ok(0)
        }
        GateCmd::Clear => {
            edit_project(&store, |cfg| {
                cfg.gate_tools.clear();
                Ok(())
            })?;
            write_stdout_line(ctx, "Cleared gate tools")?;
            Ok(0)
        }
    }
}

fn dispatch_hook(command: HookCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        HookCmd::Set {
            event,
            tool,
            on_failure,
        } => {
            let hook_event = HookEvent::from(event);
            let hook_cfg = HookConfig {
                tool,
                on_failure: HookFailure::from(on_failure),
            };
            edit_project(&store, |cfg| {
                cfg.hooks.set(hook_event, hook_cfg);
                Ok(())
            })?;
            write_stdout_line(ctx, &format!("Set hook for {hook_event:?}"))?;
            Ok(0)
        }
        HookCmd::Clear { event } => {
            let hook_event = HookEvent::from(event);
            edit_project(&store, |cfg| {
                cfg.hooks.clear(hook_event);
                Ok(())
            })?;
            write_stdout_line(ctx, &format!("Cleared hook for {hook_event:?}"))?;
            Ok(0)
        }
    }
}

fn edit_project<F>(store: &ConfigStore, mutate: F) -> Result<ProjectConfig>
where
    F: FnOnce(&mut ProjectConfig) -> Result<()>,
{
    let mut cfg = store.load_project()?;
    mutate(&mut cfg)?;
    store.save_project(&cfg)?;
    Ok(cfg)
}

fn dispatch_agent(command: AgentCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        AgentCmd::AddLocalModel {
            id,
            name,
            inference,
            model,
            temperature,
            context,
        } => {
            let agent = AgentConfig {
                agent_id: id,
                display_name: name,
                backend: AgentBackend::LocalModel {
                    inference_id: inference,
                    model,
                    temperature,
                    max_context_tokens: context,
                },
            };
            store.save_agent(&agent)?;
            write_stdout_line(ctx, &format!("Saved agent '{}'", agent.agent_id))?;
            Ok(0)
        }
        AgentCmd::AddCli {
            id,
            name,
            image,
            command,
            mount,
            memory_mb,
            timeout,
            secret_env,
            keychain_service,
            keychain_account,
        } => {
            let agent = AgentConfig {
                agent_id: id,
                display_name: name,
                backend: AgentBackend::AgentCli {
                    image,
                    command,
                    workspace_mount: WorkspaceMount::from(mount),
                    memory_limit_mb: memory_mb,
                    timeout_secs: timeout,
                    keychain_service,
                    keychain_account,
                    secret_env,
                },
            };
            store.save_agent(&agent)?;
            write_stdout_line(ctx, &format!("Saved agent '{}'", agent.agent_id))?;
            write_stdout_line(
                ctx,
                "Residual risk: anyone with Docker socket access can read container env while it runs (that access is already root-equivalent).",
            )?;
            write_stdout_line(
                ctx,
                "Residual risk: a ReadWrite mount can write .git/hooks, which the host runs at the next git command.",
            )?;
            Ok(0)
        }
        AgentCmd::AddCloud {
            id,
            name,
            endpoint,
            model,
            keychain_service,
            keychain_account,
        } => {
            let agent = AgentConfig {
                agent_id: id,
                display_name: name,
                backend: AgentBackend::CloudApi {
                    endpoint_url: endpoint,
                    model,
                    temperature: 0.0,
                    max_context_tokens: 65536,
                    keychain_service,
                    keychain_account,
                },
            };
            store.save_agent(&agent)?;
            write_stdout_line(ctx, &format!("Saved agent '{}'", agent.agent_id))?;
            Ok(0)
        }
        AgentCmd::List { json } => {
            let agents = store.load_agents()?;
            if json {
                let rendered = serde_json::to_string_pretty(&agents)?;
                write_stdout_line(ctx, &rendered)?;
            } else {
                for agent in &agents {
                    write_stdout_line(
                        ctx,
                        &format!(
                            "{}: {} ({:?})",
                            agent.agent_id, agent.display_name, agent.backend
                        ),
                    )?;
                }
            }
            Ok(0)
        }
        AgentCmd::Show { id } => {
            let agents = store.load_agents()?;
            let agent = agents
                .into_iter()
                .find(|a| a.agent_id == id)
                .ok_or_else(|| Error::Validation {
                    field: "agent_id".into(),
                    message: format!("unknown agent_id '{id}'"),
                })?;
            write_stdout_line(ctx, &format!("agent_id: {}", agent.agent_id))?;
            write_stdout_line(ctx, &format!("display_name: {}", agent.display_name))?;
            write_stdout_line(ctx, &format!("backend: {:?}", agent.backend))?;
            if matches!(agent.backend, AgentBackend::AgentCli { .. }) {
                write_stdout_line(ctx, orrery_core::config::AGENT_CLI_GUARDRAIL_NOTE)?;
            }
            Ok(0)
        }
        AgentCmd::Remove { id } => {
            store.remove_agent(&id)?;
            write_stdout_line(ctx, &format!("Removed agent '{id}'"))?;
            Ok(0)
        }
    }
}

fn dispatch_inference(command: InferenceCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        InferenceCmd::Add {
            id,
            endpoint,
            mode,
            container,
            compose,
            service,
            no_unload,
            startup_timeout,
        } => {
            let lifecycle = if let Some(name) = container {
                InferenceLifecycle::Container { name }
            } else if let Some(file) = compose {
                let _ =
                    resolve_compose_file(&file, ConfigScope::Project, store.project_root(), None)?;
                InferenceLifecycle::Compose { file, service }
            } else {
                InferenceLifecycle::External
            };
            let inf = InferenceConfig {
                inference_id: id,
                endpoint_url: endpoint,
                mode: InferenceMode::from(mode),
                lifecycle,
                unload_after_stage: !no_unload,
                startup_timeout_secs: startup_timeout,
            };
            store.save_inference(&inf)?;
            write_stdout_line(ctx, &format!("Saved inference '{}'", inf.inference_id))?;
            Ok(0)
        }
        InferenceCmd::List { json } => {
            let list = store.load_inference()?;
            if json {
                let rendered = serde_json::to_string_pretty(&list)?;
                write_stdout_line(ctx, &rendered)?;
            } else {
                for inf in &list {
                    write_stdout_line(
                        ctx,
                        &format!(
                            "{}: {} ({:?}, {:?})",
                            inf.inference_id, inf.endpoint_url, inf.mode, inf.lifecycle
                        ),
                    )?;
                }
            }
            Ok(0)
        }
        InferenceCmd::Remove { id } => {
            store.remove_inference(&id)?;
            write_stdout_line(ctx, &format!("Removed inference '{id}'"))?;
            Ok(0)
        }
    }
}

fn dispatch_role(command: RoleCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        RoleCmd::Add {
            id,
            name,
            kind,
            rules_file,
            allow_tools,
        } => {
            let source_path = store.resolve_safe_path(&rules_file)?;
            let rules_markdown =
                std::fs::read_to_string(&source_path).map_err(|source| Error::Io {
                    path: source_path,
                    source,
                })?;
            let role = RoleConfig {
                role_id: id.clone(),
                display_name: name,
                kind: RoleKind::from(kind),
                rules_file: format!("{id}.md"),
                allowed_mcp_tools: allow_tools,
            };
            store.save_role(&role, &rules_markdown)?;
            write_stdout_line(ctx, &format!("Saved role '{}'", role.role_id))?;
            Ok(0)
        }
        RoleCmd::List { json } => {
            let roles = store.load_roles()?;
            if json {
                let rendered = serde_json::to_string_pretty(&roles)?;
                write_stdout_line(ctx, &rendered)?;
            } else {
                for role in &roles {
                    write_stdout_line(
                        ctx,
                        &format!("{}: {} ({:?})", role.role_id, role.display_name, role.kind),
                    )?;
                }
            }
            Ok(0)
        }
        RoleCmd::Show { id } => {
            let roles = store.load_roles()?;
            let role =
                roles
                    .into_iter()
                    .find(|r| r.role_id == id)
                    .ok_or_else(|| Error::Validation {
                        field: "role_id".into(),
                        message: format!("unknown role_id '{id}'"),
                    })?;
            let rules = store.load_role_rules(&id)?;
            write_stdout_line(ctx, &format!("role_id: {}", role.role_id))?;
            write_stdout_line(ctx, &format!("display_name: {}", role.display_name))?;
            write_stdout_line(ctx, &format!("kind: {:?}", role.kind))?;
            write_stdout_line(
                ctx,
                &format!("allowed_mcp_tools: {:?}", role.allowed_mcp_tools),
            )?;
            write_stdout_line(ctx, &rules)?;
            Ok(0)
        }
        RoleCmd::Remove { id } => {
            store.remove_role(&id)?;
            write_stdout_line(ctx, &format!("Removed role '{id}'"))?;
            Ok(0)
        }
    }
}

fn parse_write_tool_specs(specs: &[String]) -> Result<Vec<WriteToolSpec>> {
    let mut out = Vec::with_capacity(specs.len());
    for raw in specs {
        let (tool, args_part) = raw.split_once(':').ok_or_else(|| Error::Validation {
            field: "write_tools".into(),
            message: format!("expected '<tool>:<arg>[,<arg>]' in '{raw}'"),
        })?;
        let tool = tool.trim().to_string();
        let path_args: Vec<String> = args_part.split(',').map(|s| s.trim().to_string()).collect();
        if tool.is_empty() || path_args.is_empty() || path_args.iter().any(String::is_empty) {
            return Err(Error::Validation {
                field: "write_tools".into(),
                message: format!("expected '<tool>:<arg>[,<arg>]' in '{raw}'"),
            });
        }
        out.push(WriteToolSpec { tool, path_args });
    }
    Ok(out)
}

async fn validate_connect_and_save_mcp(
    store: &ConfigStore,
    project: &Path,
    ctx: &mut CliContext,
    server: McpServerConfig,
) -> Result<u8> {
    server.validate()?;
    server.validate_project_scope()?;
    let backends = (ctx.make_backends)(project)?;
    let transport = backends.connector.connect(&server).await?;
    let tools = transport.list_tools().await?;
    store.save_mcp_server(&server)?;
    write_stdout_line(ctx, &format!("{}: {} tools", server.server_id, tools.len()))?;
    Ok(0)
}

async fn dispatch_mcp(command: McpCmd, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    match command {
        McpCmd::AddStdio {
            id,
            command,
            args,
            write_tools,
        } => {
            let server = McpServerConfig {
                server_id: id,
                transport: McpTransportConfig::Stdio { command, args },
                write_tools: parse_write_tool_specs(&write_tools)?,
            };
            validate_connect_and_save_mcp(&store, project, ctx, server).await
        }
        McpCmd::AddHttp {
            id,
            endpoint,
            keychain_service,
            keychain_account,
            write_tools,
        } => {
            let server = McpServerConfig {
                server_id: id,
                transport: McpTransportConfig::StreamableHttp {
                    endpoint_url: endpoint,
                    keychain_service,
                    keychain_account,
                },
                write_tools: parse_write_tool_specs(&write_tools)?,
            };
            validate_connect_and_save_mcp(&store, project, ctx, server).await
        }
        McpCmd::List { json } => {
            let servers = store.load_mcp_servers()?;
            if json {
                let rendered = serde_json::to_string_pretty(&servers)?;
                write_stdout_line(ctx, &rendered)?;
            } else {
                for server in &servers {
                    write_stdout_line(
                        ctx,
                        &format!("{}: {:?}", server.server_id, server.transport),
                    )?;
                }
            }
            Ok(0)
        }
        McpCmd::Tools { id } => {
            let servers = store.load_mcp_servers()?;
            let server = servers
                .into_iter()
                .find(|s| s.server_id == id)
                .ok_or_else(|| Error::Validation {
                    field: "server_id".into(),
                    message: format!("unknown MCP server '{id}'"),
                })?;
            let backends = (ctx.make_backends)(project)?;
            let transport = backends.connector.connect(&server).await?;
            let tools = transport.list_tools().await?;
            for tool in &tools {
                write_stdout_line(
                    ctx,
                    &format!("{}:{} - {}", server.server_id, tool.name, tool.description),
                )?;
            }
            Ok(0)
        }
        McpCmd::Remove { id } => {
            store.remove_mcp_server(&id)?;
            write_stdout_line(ctx, &format!("Removed MCP server '{id}'"))?;
            Ok(0)
        }
    }
}

fn dispatch_secret(command: SecretCmd, ctx: &mut CliContext) -> Result<u8> {
    match command {
        SecretCmd::Set { service, account } => {
            let raw = (ctx.read_secret)()?;
            if raw.is_empty() {
                return Err(Error::Validation {
                    field: "secret".into(),
                    message: "secret value is empty".into(),
                });
            }
            let secret = SecretValue::new(raw);
            ctx.secrets.set(&service, &account, &secret)?;
            write_stdout_line(
                ctx,
                &format!("Stored secret for service '{service}' account '{account}'"),
            )?;
            if let Ok(os_store) = OsKeyringStore::new()
                && os_store.backend() == KeyringBackend::Keyutils
            {
                write_stdout_line(
                    ctx,
                    "Note: kernel keyutils keyring does not survive reboot; re-run `orrery secret set` after reboot.",
                )?;
            }
            Ok(0)
        }
        SecretCmd::Delete { service, account } => {
            ctx.secrets.delete(&service, &account)?;
            write_stdout_line(
                ctx,
                &format!("Deleted secret for service '{service}' account '{account}'"),
            )?;
            Ok(0)
        }
    }
}

fn format_stream_event(ev: &StreamEvent) -> String {
    let agent = ev.agent_id.as_deref().unwrap_or("-");
    format!("[{:?}] {agent}: {}", ev.kind, ev.text)
}

async fn dispatch_run(opts: RunOptions, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    let backends = (ctx.make_backends)(project)?;
    let engine = build_engine(&store, backends).await?;
    let mut stream_rx = engine.subscribe_stream();
    for ev in engine.recent_stream(usize::MAX)? {
        write_stdout_line(ctx, &format_stream_event(&ev))?;
    }
    let cancel = CancellationToken::new();

    let signal_cancel = cancel.clone();
    let signal_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancel.cancel();
        }
    });

    let run_fut = engine.run(opts, cancel);
    tokio::pin!(run_fut);

    let outcome_res = loop {
        tokio::select! {
            biased;
            recv = stream_rx.recv() => {
                match recv {
                    Ok(ev) => {
                        write_stdout_line(ctx, &format_stream_event(&ev))?;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        write_stdout_line(
                            ctx,
                            &format!("[SystemNotice] -: {n} stream events dropped"),
                        )?;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break run_fut.as_mut().await;
                    }
                }
            }
            res = &mut run_fut => {
                break res;
            }
        }
    };

    signal_task.abort();
    while let Ok(ev) = stream_rx.try_recv() {
        write_stdout_line(ctx, &format_stream_event(&ev))?;
    }

    let outcome = match outcome_res {
        Ok(outcome) => outcome,
        Err(err) => {
            let text = engine.redact(&render_error(&err));
            writeln!(ctx.err, "{text}").map_err(|source| Error::Io {
                path: PathBuf::from("<stderr>"),
                source,
            })?;
            return Ok(exit_code_for(&err));
        }
    };

    match outcome {
        RunOutcome::Completed { .. } | RunOutcome::NoPendingSteps => Ok(0),
        RunOutcome::AwaitingOperator { .. } => Ok(3),
        RunOutcome::Interrupted => Ok(130),
    }
}

fn dispatch_operator_decision(
    decision: OperatorDecision,
    project: &Path,
    ctx: &mut CliContext,
) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
    let mut state = checkpoints.load_state()?.ok_or_else(|| Error::Validation {
        field: "checkpoint".into(),
        message: "no step awaiting operator".into(),
    })?;
    if state.step_statuses.get(state.current_step_index) != Some(&StepStatus::AwaitingOperator) {
        return Err(Error::Validation {
            field: "checkpoint".into(),
            message: "no step awaiting operator".into(),
        });
    }
    state.operator_decision = Some(decision);
    checkpoints.save_state(&state)?;
    write_stdout_line(
        ctx,
        &format!(
            "Recorded operator decision for step {}",
            state.current_step_index + 1
        ),
    )?;
    Ok(0)
}

fn dispatch_reset(step: usize, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
    let mut state = match checkpoints.load_state()? {
        Some(cp) => cp,
        None => {
            let cfg = store.load_project()?;
            let resolved = store.resolve_safe_path(&cfg.plan_path)?;
            let plan = ImplementationPlan::from_file(&resolved, &cfg.step_marker)?;
            WorkflowCheckpoint::new(&plan)
        }
    };
    state.reset_from(step)?;
    checkpoints.save_state(&state)?;
    write_stdout_line(ctx, &format!("Reset steps from step {step} to Pending"))?;
    Ok(0)
}

fn dispatch_status(json: bool, project: &Path, ctx: &mut CliContext) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    let cfg = store.load_project()?;
    let resolved = store.resolve_safe_path(&cfg.plan_path)?;
    if !resolved.is_file() {
        return Err(Error::Validation {
            field: "plan_path".into(),
            message: format!(
                "plan file '{}' not found; run `orrery ingest <MD_FILE>` first",
                cfg.plan_path.display()
            ),
        });
    }
    let plan = ImplementationPlan::from_file(&resolved, &cfg.step_marker)?;
    let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
    let state = checkpoints.load_state()?;

    let (run_id, phase, current_step, round, retry_count, awaiting_reason, statuses) = match state {
        Some(mut cp) => {
            cp.reconcile(&plan)?;
            let current = plan
                .steps
                .get(cp.current_step_index)
                .map_or(cp.current_step_index + 1, |s| s.number);
            let step_statuses: Vec<StepStatus> = (0..plan.steps.len())
                .map(|i| {
                    cp.step_statuses
                        .get(i)
                        .copied()
                        .unwrap_or(StepStatus::Pending)
                })
                .collect();
            (
                Some(cp.run_id.to_string()),
                cp.phase,
                current,
                cp.round,
                cp.retry_count,
                cp.awaiting_reason,
                step_statuses,
            )
        }
        None => (
            None,
            EnginePhase::Idle,
            plan.steps.first().map_or(0, |s| s.number),
            0,
            0,
            None,
            vec![StepStatus::Pending; plan.steps.len()],
        ),
    };

    if json {
        let steps_json: Vec<serde_json::Value> = plan
            .steps
            .iter()
            .zip(statuses.iter())
            .map(|(step, status)| {
                serde_json::json!({
                    "number": step.number,
                    "title": step.title,
                    "status": status,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "run_id": run_id,
            "phase": phase,
            "current_step": current_step,
            "round": round,
            "retry_count": retry_count,
            "awaiting_reason": awaiting_reason,
            "steps": steps_json,
        });
        let rendered = serde_json::to_string_pretty(&payload)?;
        write_stdout_line(ctx, &rendered)?;
    } else {
        write_stdout_line(
            ctx,
            &format!(
                "Phase: {phase:?} | current_step: {current_step} | round: {round} | retries: {retry_count}"
            ),
        )?;
        if let Some(reason) = &awaiting_reason {
            write_stdout_line(ctx, &format!("Awaiting operator: {reason}"))?;
        }
        for (step, status) in plan.steps.iter().zip(statuses.iter()) {
            write_stdout_line(ctx, &format!("[{status:?}] {}", step.title))?;
        }
    }
    Ok(0)
}

async fn dispatch_logs(
    tail: Option<usize>,
    follow: bool,
    project: &Path,
    ctx: &mut CliContext,
) -> Result<u8> {
    let store = ConfigStore::new(project, None)?;
    let log_dir = store.orrery_dir().join("logs");
    let limit = tail.unwrap_or(usize::MAX);
    let lines = tail_log(&log_dir, limit)?;
    for line in &lines {
        write_stdout_line(ctx, line)?;
    }
    if follow {
        let mut follower = LogFollower::new(&log_dir)?;
        let ctrl_c = tokio::signal::ctrl_c();
        tokio::pin!(ctrl_c);
        loop {
            tokio::select! {
                _ = &mut ctrl_c => {
                    return Ok(130);
                }
                () = tokio::time::sleep(Duration::from_millis(500)) => {
                    for line in follower.poll()? {
                        write_stdout_line(ctx, &line)?;
                    }
                }
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use orrery_core::checkpoint::AgentReport;
    use orrery_core::config::{
        ConfigStore, ExecutionMode, HookConfig, HookFailure, ValidationMode,
    };
    use orrery_core::docker::{MockContainerBackend, MockInferenceControl};
    use orrery_core::engine::DriverSet;
    use orrery_core::llm::{AgentDriver, MockAgentDriver};
    use orrery_core::mcp::{McpTransport, MockMcpConnector, MockMcpTransport};
    use orrery_core::security::InMemorySecretStore;
    use tempfile::TempDir;

    use super::*;
    use crate::args::{
        HookEventArg, HookFailureArg, InferenceModeArg, ModeArg, MountArg, RoleKindArg,
    };

    #[derive(Clone, Default)]
    struct SharedVec(Arc<Mutex<Vec<u8>>>);

    impl SharedVec {
        fn as_string(&self) -> String {
            let guard = self.0.lock().expect("buffer lock");
            String::from_utf8_lossy(&guard).into_owned()
        }

        fn clear(&self) {
            self.0.lock().expect("buffer lock").clear();
        }
    }

    impl Write for SharedVec {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("buffer lock").write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.lock().expect("buffer lock").flush()
        }
    }

    fn test_ctx() -> (CliContext, SharedVec, SharedVec) {
        let out = SharedVec::default();
        let err = SharedVec::default();
        let secrets = Arc::new(InMemorySecretStore::new());
        let ctx = CliContext {
            out: Box::new(out.clone()),
            err: Box::new(err.clone()),
            secrets,
            read_secret: Box::new(|| Ok(String::new())),
            make_backends: Box::new(|_root: &Path| {
                Err(Error::Validation {
                    field: "backends".into(),
                    message: "unexpected backends call in Step 34 test".into(),
                })
            }),
        };
        (ctx, out, err)
    }

    #[tokio::test]
    async fn init_creates_layout_and_template() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        let code = dispatch(
            Command::Init {
                path: None,
                name: Some("demo-app".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init should succeed");
        assert_eq!(code, 0);
        assert!(out.as_string().contains("demo-app"));

        let orrery_dir = dir.path().join(".orrery");
        for sub in ["roles", "agents", "mcp", "checkpoints", "agent_io", "logs"] {
            assert!(orrery_dir.join(sub).is_dir(), "missing subdir {sub}");
        }
        for role_id in ["orchestrator", "coder", "validator"] {
            assert!(
                orrery_dir
                    .join("roles")
                    .join(format!("{role_id}.toml"))
                    .is_file(),
                "missing role toml {role_id}"
            );
            assert!(
                orrery_dir
                    .join("roles")
                    .join(format!("{role_id}.md"))
                    .is_file(),
                "missing role md {role_id}"
            );
        }

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(cfg.project_name, "demo-app");
        assert_eq!(cfg.stages.len(), 3);
        assert_eq!(cfg.stages[0].role_id, "orchestrator");
        assert_eq!(cfg.stages[1].role_id, "coder");
        assert_eq!(cfg.stages[2].role_id, "validator");
        assert_eq!(cfg.execution_mode, ExecutionMode::Run);
        assert_eq!(cfg.validation_mode, ValidationMode::Auto);

        // Also test init with explicit path and default name derived from directory.
        let sub_dir = dir.path().join("nested-proj");
        std::fs::create_dir_all(&sub_dir).expect("create nested dir");
        let code2 = dispatch(
            Command::Init {
                path: Some(sub_dir.clone()),
                name: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init nested should succeed");
        assert_eq!(code2, 0);
        let nested_store = ConfigStore::new(&sub_dir, None).expect("nested store");
        let nested_cfg = nested_store.load_project().expect("load nested project");
        assert_eq!(nested_cfg.project_name, "nested-proj");
    }

    #[tokio::test]
    async fn ingest_sets_plan_and_prints_steps() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("ingest-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");
        out.clear();

        let plan_path = dir.path().join("roadmap.md");
        std::fs::write(
            &plan_path,
            "# Roadmap\n### Phase 1: Setup\ndo setup\n### Phase 2: Build\ndo build\n",
        )
        .expect("write plan");

        let code = dispatch(
            Command::Ingest {
                md_file: PathBuf::from("roadmap.md"),
                marker: Some("### Phase ".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("ingest should succeed");
        assert_eq!(code, 0);

        let printed = out.as_string();
        assert!(printed.contains("1: Setup"), "got: {printed}");
        assert!(printed.contains("2: Build"), "got: {printed}");
        assert!(!printed.contains("Step 1: 1:"), "got: {printed}");

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(cfg.plan_path, PathBuf::from("roadmap.md"));
        assert_eq!(cfg.step_marker, "### Phase ");
    }

    #[tokio::test]
    async fn ingest_bad_plan_exits_1() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("bad-plan-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        std::fs::write(dir.path().join("bad.md"), "just preamble without steps\n")
            .expect("write bad plan");

        let err = dispatch(
            Command::Ingest {
                md_file: PathBuf::from("bad.md"),
                marker: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("bad plan must error");
        assert_eq!(exit_code_for(&err), 1);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(
            cfg.plan_path,
            PathBuf::from("STEPPED_IMPLEMENTATION_PLAN.md")
        );
    }

    #[tokio::test]
    async fn workflow_show_json_shape() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("show-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");
        out.clear();

        let code = dispatch(
            Command::Workflow {
                command: WorkflowCmd::Show { json: true },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("workflow show --json");
        assert_eq!(code, 0);

        let val: serde_json::Value =
            serde_json::from_str(&out.as_string()).expect("valid json output");
        assert_eq!(val["validation_mode"], "Agents");
        let stages = val["stages"].as_array().expect("stages array");
        assert_eq!(stages.len(), 3);
        assert_eq!(
            stages[0],
            serde_json::json!({"index": 0, "role_id": "orchestrator", "agent_ids": []})
        );
        assert_eq!(
            stages[1],
            serde_json::json!({"index": 1, "role_id": "coder", "agent_ids": []})
        );
        assert_eq!(
            stages[2],
            serde_json::json!({"index": 2, "role_id": "validator", "agent_ids": []})
        );
        assert_eq!(val["warnings"], serde_json::json!([]));

        // Also check plain-text show and warning emission when no validator stage is present.
        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Stages { count: 2 },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("shrink stages to 2");
        out.clear();

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Show { json: true },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("workflow show --json with warning");
        let val_warn: serde_json::Value =
            serde_json::from_str(&out.as_string()).expect("valid json");
        assert_eq!(val_warn["validation_mode"], "GateAndOperator");
        assert_eq!(
            val_warn["warnings"]
                .as_array()
                .expect("warnings array")
                .len(),
            1
        );

        out.clear();
        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Show { json: false },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("workflow show text");
        let text = out.as_string();
        assert!(text.contains("Validation mode: GateAndOperator"));
        assert!(text.contains("Stage 0: role=orchestrator"));
        assert!(text.contains("Warning:"));
    }

    #[tokio::test]
    async fn workflow_stages_grows_and_shrinks() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("stages-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let code_grow = dispatch(
            Command::Workflow {
                command: WorkflowCmd::Stages { count: 5 },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("grow stages");
        assert_eq!(code_grow, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(cfg.stages.len(), 5);
        assert_eq!(cfg.stages[3].role_id, "coder");
        assert_eq!(cfg.stages[4].role_id, "coder");

        let code_shrink = dispatch(
            Command::Workflow {
                command: WorkflowCmd::Stages { count: 2 },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("shrink stages");
        assert_eq!(code_shrink, 0);

        let cfg2 = store.load_project().expect("load_project");
        assert_eq!(cfg2.stages.len(), 2);
        assert_eq!(cfg2.stages[0].role_id, "orchestrator");
        assert_eq!(cfg2.stages[1].role_id, "coder");
    }

    #[tokio::test]
    async fn workflow_set_role_assign_unassign_clear() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("edit-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::SetRole {
                    stage: 2,
                    role_id: "coder".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("set-role");

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Assign {
                    stage: 1,
                    agent_id: "coder-a".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("assign a");

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Assign {
                    stage: 1,
                    agent_id: "coder-b".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("assign b");

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(cfg.stages[2].role_id, "coder");
        assert_eq!(
            cfg.stages[1].agent_ids,
            vec!["coder-a".to_string(), "coder-b".to_string()]
        );

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Unassign {
                    stage: 1,
                    agent_id: "coder-a".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("unassign a");

        let cfg2 = store.load_project().expect("load_project");
        assert_eq!(cfg2.stages[1].agent_ids, vec!["coder-b".to_string()]);

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Clear { stage: 1 },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("clear stage 1");

        let cfg3 = store.load_project().expect("load_project");
        assert!(cfg3.stages[1].agent_ids.is_empty());
    }

    #[tokio::test]
    async fn workflow_invalid_edit_exits_1() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("invalid-edit-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let err = dispatch(
            Command::Workflow {
                command: WorkflowCmd::SetRole {
                    stage: 2,
                    role_id: "orchestrator".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("orchestrator on stage 2 must fail");
        assert_eq!(exit_code_for(&err), 1);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(cfg.stages[2].role_id, "validator");
    }

    #[tokio::test]
    async fn workflow_mode_loop_with_cycles() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("mode-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let code = dispatch(
            Command::Workflow {
                command: WorkflowCmd::Mode {
                    mode: ModeArg::Loop,
                    cycles: Some(4),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("set loop mode");
        assert_eq!(code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(cfg.execution_mode, ExecutionMode::Loop);
        assert_eq!(cfg.cycle_count, 4);

        dispatch(
            Command::Workflow {
                command: WorkflowCmd::Mode {
                    mode: ModeArg::Run,
                    cycles: None,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("set run mode");

        let cfg2 = store.load_project().expect("load_project");
        assert_eq!(cfg2.execution_mode, ExecutionMode::Run);
        assert_eq!(cfg2.cycle_count, 4);
    }

    #[tokio::test]
    async fn gate_set_and_clear() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("gate-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let code_set = dispatch(
            Command::Gate {
                command: GateCmd::Set {
                    tools: vec!["fs:test".to_string(), "fs:lint".to_string()],
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("gate set");
        assert_eq!(code_set, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(
            cfg.gate_tools,
            vec!["fs:test".to_string(), "fs:lint".to_string()]
        );

        let code_clear = dispatch(
            Command::Gate {
                command: GateCmd::Clear,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("gate clear");
        assert_eq!(code_clear, 0);

        let cfg2 = store.load_project().expect("load_project");
        assert!(cfg2.gate_tools.is_empty());
    }

    #[tokio::test]
    async fn hook_set_and_clear() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("hook-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let code_set = dispatch(
            Command::Hook {
                command: HookCmd::Set {
                    event: HookEventArg::Start,
                    tool: "fs:notify".to_string(),
                    on_failure: HookFailureArg::Warn,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("hook set");
        assert_eq!(code_set, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let cfg = store.load_project().expect("load_project");
        assert_eq!(
            cfg.hooks.on_step_start,
            Some(HookConfig {
                tool: "fs:notify".to_string(),
                on_failure: HookFailure::Warn,
            })
        );

        let code_clear = dispatch(
            Command::Hook {
                command: HookCmd::Clear {
                    event: HookEventArg::Start,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("hook clear");
        assert_eq!(code_clear, 0);

        let cfg2 = store.load_project().expect("load_project");
        assert!(cfg2.hooks.on_step_start.is_none());

        assert_eq!(exit_code_for(&Error::Interrupted), 130);
    }

    fn mock_backends_with_transports(
        transports: HashMap<String, Arc<dyn McpTransport>>,
    ) -> MakeBackendsFn {
        Box::new(move |_root: &Path| {
            let chat: Arc<dyn AgentDriver> = Arc::new(MockAgentDriver::new());
            let cli: Arc<dyn AgentDriver> = Arc::new(MockAgentDriver::new());
            Ok(Backends {
                secrets: Arc::new(InMemorySecretStore::new()),
                containers: Arc::new(MockContainerBackend::new()),
                inference_control: Arc::new(MockInferenceControl::new()),
                connector: Arc::new(MockMcpConnector {
                    transports: transports.clone(),
                }),
                drivers: DriverSet { chat, cli },
            })
        })
    }

    #[tokio::test]
    async fn agent_add_each_backend_and_list_json() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("agents-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        dispatch(
            Command::Agent {
                command: AgentCmd::AddLocalModel {
                    id: "local-coder".to_string(),
                    name: "Local Coder".to_string(),
                    inference: "coder-14b".to_string(),
                    model: "qwen-coder".to_string(),
                    temperature: 0.2,
                    context: 32768,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("add-local-model");

        dispatch(
            Command::Agent {
                command: AgentCmd::AddCli {
                    id: "cli-val".to_string(),
                    name: "CLI Validator".to_string(),
                    image: "registry.example.com/agents/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                    command: vec!["agent-cli".to_string(), "run".to_string()],
                    mount: MountArg::Ro,
                    memory_mb: 4096,
                    timeout: 1800,
                    secret_env: Some("AGENT_TOKEN".to_string()),
                    keychain_service: Some("orrery.agent".to_string()),
                    keychain_account: Some("testuser".to_string()),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("add-cli");

        dispatch(
            Command::Agent {
                command: AgentCmd::AddCloud {
                    id: "cloud-val".to_string(),
                    name: "Cloud Validator".to_string(),
                    endpoint: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "example-model".to_string(),
                    keychain_service: "orrery.cloud".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("add-cloud");

        out.clear();
        let code = dispatch(
            Command::Agent {
                command: AgentCmd::List { json: true },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("agent list --json");
        assert_eq!(code, 0);

        let listed: Vec<AgentConfig> =
            serde_json::from_str(&out.as_string()).expect("parse AgentConfig array");
        assert_eq!(listed.len(), 3);
        assert!(
            listed.iter().any(|a| a.agent_id == "local-coder"
                && matches!(a.backend, AgentBackend::LocalModel { .. }))
        );
        assert!(
            listed
                .iter()
                .any(|a| a.agent_id == "cli-val"
                    && matches!(a.backend, AgentBackend::AgentCli { .. }))
        );
        assert!(listed.iter().any(
            |a| a.agent_id == "cloud-val" && matches!(a.backend, AgentBackend::CloudApi { .. })
        ));

        out.clear();
        dispatch(
            Command::Agent {
                command: AgentCmd::List { json: false },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("agent list plain");
        let text = out.as_string();
        assert!(text.contains("local-coder"));
        assert!(text.contains("cli-val"));
        assert!(text.contains("cloud-val"));
    }

    #[tokio::test]
    async fn agent_add_cli_prints_residual_risk() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("risk-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");
        out.clear();

        let code = dispatch(
            Command::Agent {
                command: AgentCmd::AddCli {
                    id: "cli-coder".to_string(),
                    name: "CLI Coder".to_string(),
                    image: "registry.example.com/agents/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                    command: vec!["agent-cli".to_string()],
                    mount: MountArg::Rw,
                    memory_mb: 4096,
                    timeout: 1800,
                    secret_env: None,
                    keychain_service: None,
                    keychain_account: None,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("add-cli");
        assert_eq!(code, 0);

        let printed = out.as_string();
        assert!(
            printed.contains("Docker socket"),
            "missing Docker socket note: {printed}"
        );
        assert!(
            printed.contains(".git/hooks"),
            "missing .git/hooks note: {printed}"
        );
    }

    #[tokio::test]
    async fn agent_add_with_embedded_secret_exits_1() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("secret-reject-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let err = dispatch(
            Command::Agent {
                command: AgentCmd::AddLocalModel {
                    id: "bad-agent".to_string(),
                    name: "Bad Agent".to_string(),
                    inference: "coder-14b".to_string(),
                    model: "sk-ABCDEFGH12345678".to_string(),
                    temperature: 0.1,
                    context: 32768,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("embedded secret must fail");
        assert_eq!(exit_code_for(&err), 1);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        assert!(store.load_agents().expect("load_agents").is_empty());
    }

    #[tokio::test]
    async fn agent_show_and_remove() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("show-remove-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        dispatch(
            Command::Agent {
                command: AgentCmd::AddCli {
                    id: "cli-agent".to_string(),
                    name: "CLI Agent".to_string(),
                    image: "registry.example.com/agents/cli:1.0.0".to_string(),
                    command: vec!["agent-cli".to_string()],
                    mount: MountArg::Ro,
                    memory_mb: 2048,
                    timeout: 600,
                    secret_env: None,
                    keychain_service: None,
                    keychain_account: None,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("add-cli");
        out.clear();

        let code = dispatch(
            Command::Agent {
                command: AgentCmd::Show {
                    id: "cli-agent".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("agent show");
        assert_eq!(code, 0);
        let shown = out.as_string();
        assert!(shown.contains("cli-agent"), "got: {shown}");
        assert!(
            shown.contains("MCP gatekeeper") && shown.contains("workspace_mount"),
            "missing guardrail note: {shown}"
        );

        let rm_code = dispatch(
            Command::Agent {
                command: AgentCmd::Remove {
                    id: "cli-agent".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("agent remove");
        assert_eq!(rm_code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        assert!(store.load_agents().expect("load_agents").is_empty());

        let missing_err = dispatch(
            Command::Agent {
                command: AgentCmd::Show {
                    id: "cli-agent".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("show removed agent must fail");
        assert_eq!(exit_code_for(&missing_err), 1);
    }

    #[tokio::test]
    async fn inference_add_defaults_external_single() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("inf-defaults-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let code = dispatch(
            Command::Inference {
                command: InferenceCmd::Add {
                    id: "ext-model".to_string(),
                    endpoint: "http://192.0.2.10:8080".to_string(),
                    mode: InferenceModeArg::Single,
                    container: None,
                    compose: None,
                    service: None,
                    no_unload: false,
                    startup_timeout: 300,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference add defaults");
        assert_eq!(code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let list = store.load_inference().expect("load_inference");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].inference_id, "ext-model");
        assert_eq!(list[0].mode, InferenceMode::Single);
        assert_eq!(list[0].lifecycle, InferenceLifecycle::External);
        assert!(list[0].unload_after_stage);
        assert_eq!(list[0].startup_timeout_secs, 300);
    }

    #[tokio::test]
    async fn inference_add_container_and_compose() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("inf-lifecycle-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        dispatch(
            Command::Inference {
                command: InferenceCmd::Add {
                    id: "coder-container".to_string(),
                    endpoint: "http://192.0.2.11:8080".to_string(),
                    mode: InferenceModeArg::Single,
                    container: Some("llama-coder-14b".to_string()),
                    compose: None,
                    service: None,
                    no_unload: false,
                    startup_timeout: 300,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference add container");

        let compose_rel = PathBuf::from("docker-models/router/compose.yaml");
        let compose_disk = dir.path().join(".orrery").join(&compose_rel);
        std::fs::create_dir_all(compose_disk.parent().expect("parent")).expect("mkdirs");
        std::fs::write(&compose_disk, "services:\n  llama:\n    image: app:1.0\n")
            .expect("write compose");

        dispatch(
            Command::Inference {
                command: InferenceCmd::Add {
                    id: "router-compose".to_string(),
                    endpoint: "http://192.0.2.12:8080".to_string(),
                    mode: InferenceModeArg::Router,
                    container: None,
                    compose: Some(compose_rel.clone()),
                    service: Some("llama".to_string()),
                    no_unload: true,
                    startup_timeout: 120,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference add compose");

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let raw_project = store.load_project().expect("load_project");
        assert_eq!(raw_project.inference.len(), 2);
        assert_eq!(
            raw_project.inference[0].lifecycle,
            InferenceLifecycle::Container {
                name: "llama-coder-14b".to_string()
            }
        );
        assert_eq!(raw_project.inference[1].mode, InferenceMode::Router);
        assert!(!raw_project.inference[1].unload_after_stage);
        assert_eq!(raw_project.inference[1].startup_timeout_secs, 120);
        assert_eq!(
            raw_project.inference[1].lifecycle,
            InferenceLifecycle::Compose {
                file: compose_rel,
                service: Some("llama".to_string()),
            }
        );

        let loaded = store.load_inference().expect("load_inference");
        assert_eq!(loaded.len(), 2);
    }

    #[tokio::test]
    async fn inference_add_compose_absolute_rejected() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("inf-abs-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let abs_compose = dir.path().join(".orrery").join("compose.yaml");
        std::fs::write(&abs_compose, "services: {}\n").expect("write compose");

        let err = dispatch(
            Command::Inference {
                command: InferenceCmd::Add {
                    id: "bad-compose".to_string(),
                    endpoint: "http://192.0.2.13:8080".to_string(),
                    mode: InferenceModeArg::Single,
                    container: None,
                    compose: Some(abs_compose),
                    service: None,
                    no_unload: false,
                    startup_timeout: 300,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("absolute compose path must fail");
        assert_eq!(exit_code_for(&err), 1);

        out.clear();
        let code = dispatch(
            Command::Inference {
                command: InferenceCmd::List { json: false },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference list");
        assert_eq!(code, 0);
        assert!(out.as_string().is_empty(), "got: {}", out.as_string());

        let store = ConfigStore::new(dir.path(), None).expect("store");
        assert!(store.load_inference().expect("load_inference").is_empty());
    }

    #[tokio::test]
    async fn inference_list_remove() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("inf-list-rm-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        dispatch(
            Command::Inference {
                command: InferenceCmd::Add {
                    id: "coder-14b".to_string(),
                    endpoint: "http://192.0.2.10:8080".to_string(),
                    mode: InferenceModeArg::Single,
                    container: None,
                    compose: None,
                    service: None,
                    no_unload: false,
                    startup_timeout: 300,
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference add");
        out.clear();

        dispatch(
            Command::Inference {
                command: InferenceCmd::List { json: true },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference list --json");
        let parsed: Vec<InferenceConfig> =
            serde_json::from_str(&out.as_string()).expect("parse InferenceConfig array");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].inference_id, "coder-14b");

        out.clear();
        dispatch(
            Command::Inference {
                command: InferenceCmd::List { json: false },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference list plain");
        assert!(out.as_string().contains("coder-14b"));

        let rm_code = dispatch(
            Command::Inference {
                command: InferenceCmd::Remove {
                    id: "coder-14b".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("inference remove");
        assert_eq!(rm_code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        assert!(store.load_inference().expect("load_inference").is_empty());
    }

    #[tokio::test]
    async fn role_add_reads_rules_file() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("role-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let rules_body = "# Reviewer Rules\n\nVerify all gates pass.\n";
        std::fs::write(dir.path().join("reviewer.md"), rules_body).expect("write reviewer.md");

        let code = dispatch(
            Command::Role {
                command: RoleCmd::Add {
                    id: "reviewer".to_string(),
                    name: "Reviewer".to_string(),
                    kind: RoleKindArg::Validator,
                    rules_file: PathBuf::from("reviewer.md"),
                    allow_tools: vec!["fs:read_file".to_string(), "git:diff".to_string()],
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("role add");
        assert_eq!(code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let loaded_rules = store.load_role_rules("reviewer").expect("load_role_rules");
        assert_eq!(loaded_rules, rules_body);

        out.clear();
        dispatch(
            Command::Role {
                command: RoleCmd::List { json: true },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("role list --json");
        let roles: Vec<RoleConfig> =
            serde_json::from_str(&out.as_string()).expect("parse RoleConfig array");
        assert!(roles.iter().any(|r| r.role_id == "reviewer"
            && r.kind == RoleKind::Validator
            && r.allowed_mcp_tools == vec!["fs:read_file".to_string(), "git:diff".to_string()]));

        out.clear();
        dispatch(
            Command::Role {
                command: RoleCmd::Show {
                    id: "reviewer".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("role show");
        assert!(out.as_string().contains("Verify all gates pass."));

        dispatch(
            Command::Role {
                command: RoleCmd::Remove {
                    id: "reviewer".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("role remove");
        assert!(
            !store
                .load_roles()
                .expect("load_roles")
                .iter()
                .any(|r| r.role_id == "reviewer")
        );
    }

    #[tokio::test]
    async fn mcp_add_stdio_and_http_list() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        let mut transports: HashMap<String, Arc<dyn McpTransport>> = HashMap::new();
        transports.insert(
            "fs".to_string(),
            Arc::new(
                MockMcpTransport::new("fs")
                    .with_tool("read_file", serde_json::json!({}))
                    .with_tool("write_file", serde_json::json!({})),
            ),
        );
        transports.insert(
            "remote".to_string(),
            Arc::new(MockMcpTransport::new("remote").with_tool("put_file", serde_json::json!({}))),
        );
        ctx.make_backends = mock_backends_with_transports(transports);

        dispatch(
            Command::Init {
                path: None,
                name: Some("mcp-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");
        out.clear();

        let code_stdio = dispatch(
            Command::Mcp {
                command: McpCmd::AddStdio {
                    id: "fs".to_string(),
                    command: "mcp-fs".to_string(),
                    args: vec!["--stdio".to_string()],
                    write_tools: vec!["write_file:path,dest".to_string()],
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp add-stdio");
        assert_eq!(code_stdio, 0);
        assert!(
            out.as_string().contains("fs: 2 tools"),
            "got: {}",
            out.as_string()
        );

        out.clear();
        let code_http = dispatch(
            Command::Mcp {
                command: McpCmd::AddHttp {
                    id: "remote".to_string(),
                    endpoint: "https://mcp.example.com/v1".to_string(),
                    keychain_service: Some("orrery.mcp".to_string()),
                    keychain_account: Some("testuser".to_string()),
                    write_tools: vec!["put_file:path".to_string()],
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp add-http");
        assert_eq!(code_http, 0);
        assert!(
            out.as_string().contains("remote: 1 tools"),
            "got: {}",
            out.as_string()
        );

        out.clear();
        dispatch(
            Command::Mcp {
                command: McpCmd::List { json: true },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp list --json");
        let servers: Vec<McpServerConfig> =
            serde_json::from_str(&out.as_string()).expect("parse McpServerConfig array");
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].server_id, "fs");
        assert_eq!(
            servers[0].write_tools,
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string(), "dest".to_string()],
            }]
        );
        assert_eq!(servers[1].server_id, "remote");

        out.clear();
        dispatch(
            Command::Mcp {
                command: McpCmd::List { json: false },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp list plain");
        assert!(out.as_string().contains("fs:"));
        assert!(out.as_string().contains("remote:"));

        dispatch(
            Command::Mcp {
                command: McpCmd::Remove {
                    id: "remote".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp remove");
        let store = ConfigStore::new(dir.path(), None).expect("store");
        assert_eq!(store.load_mcp_servers().expect("load_mcp_servers").len(), 1);
    }

    #[tokio::test]
    async fn mcp_add_unreachable_server_not_saved() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();
        ctx.make_backends = mock_backends_with_transports(HashMap::new());

        dispatch(
            Command::Init {
                path: None,
                name: Some("mcp-unreach-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let err = dispatch(
            Command::Mcp {
                command: McpCmd::AddStdio {
                    id: "unreachable".to_string(),
                    command: "mcp-unreachable".to_string(),
                    args: Vec::new(),
                    write_tools: Vec::new(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("unreachable server must fail");
        assert_eq!(exit_code_for(&err), 1);

        out.clear();
        let code = dispatch(
            Command::Mcp {
                command: McpCmd::List { json: false },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp list");
        assert_eq!(code, 0);
        assert!(out.as_string().is_empty(), "got: {}", out.as_string());

        let store = ConfigStore::new(dir.path(), None).expect("store");
        assert!(
            store
                .load_mcp_servers()
                .expect("load_mcp_servers")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn mcp_tools_lists_mock_tools() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, _err) = test_ctx();

        let mut transports: HashMap<String, Arc<dyn McpTransport>> = HashMap::new();
        transports.insert(
            "fs".to_string(),
            Arc::new(
                MockMcpTransport::new("fs")
                    .with_tool("read_file", serde_json::json!({}))
                    .with_tool("list_dir", serde_json::json!({})),
            ),
        );
        ctx.make_backends = mock_backends_with_transports(transports);

        dispatch(
            Command::Init {
                path: None,
                name: Some("mcp-tools-proj".to_string()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        dispatch(
            Command::Mcp {
                command: McpCmd::AddStdio {
                    id: "fs".to_string(),
                    command: "mcp-fs".to_string(),
                    args: Vec::new(),
                    write_tools: Vec::new(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp add-stdio");
        out.clear();

        let code = dispatch(
            Command::Mcp {
                command: McpCmd::Tools {
                    id: "fs".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("mcp tools");
        assert_eq!(code, 0);

        let listed = out.as_string();
        assert!(listed.contains("fs:read_file"), "got: {listed}");
        assert!(listed.contains("fs:list_dir"), "got: {listed}");
    }

    #[tokio::test]
    async fn secret_set_and_delete() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, out, err) = test_ctx();
        let raw_secret = "top-secret-token-value-98765";
        ctx.read_secret = Box::new(move || Ok(raw_secret.to_string()));

        let set_code = dispatch(
            Command::Secret {
                command: SecretCmd::Set {
                    service: "orrery.cloud".to_string(),
                    account: "testuser".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("secret set");
        assert_eq!(set_code, 0);

        assert!(!out.as_string().contains(raw_secret));
        assert!(!err.as_string().contains(raw_secret));

        let stored = ctx
            .secrets
            .get("orrery.cloud", "testuser")
            .expect("secret must be stored in InMemorySecretStore");
        assert_eq!(stored.len(), raw_secret.len());
        assert_eq!(format!("{stored}"), "[REDACTED]");

        let del_code = dispatch(
            Command::Secret {
                command: SecretCmd::Delete {
                    service: "orrery.cloud".to_string(),
                    account: "testuser".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("secret delete");
        assert_eq!(del_code, 0);
        assert!(ctx.secrets.get("orrery.cloud", "testuser").is_err());
    }

    #[tokio::test]
    async fn secret_set_empty_rejected() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();
        ctx.read_secret = Box::new(|| Ok(String::new()));

        let err = dispatch(
            Command::Secret {
                command: SecretCmd::Set {
                    service: "orrery.cloud".to_string(),
                    account: "testuser".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("empty secret must fail");
        assert_eq!(exit_code_for(&err), 1);
        assert_eq!(err.to_string(), "invalid secret: secret value is empty");
        assert!(ctx.secrets.get("orrery.cloud", "testuser").is_err());
    }

    fn dispatch_report(target_stage: usize, target_agent_id: &str) -> AgentReport {
        AgentReport {
            status: orrery_core::checkpoint::ReportStatus::Dispatch,
            summary: format!("dispatch to stage {target_stage}"),
            target_stage: Some(target_stage),
            allocations: vec![orrery_core::checkpoint::SubtaskAllocation {
                target_agent_id: target_agent_id.to_string(),
                instructions: "execute subtask".to_string(),
                write_scope: Vec::new(),
            }],
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn complete_report() -> AgentReport {
        AgentReport {
            status: orrery_core::checkpoint::ReportStatus::Complete,
            summary: "implementation complete".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn confirmed_report() -> AgentReport {
        AgentReport {
            status: orrery_core::checkpoint::ReportStatus::Confirmed,
            summary: "validation confirmed".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn step_passed_report(summary: &str) -> AgentReport {
        AgentReport {
            status: orrery_core::checkpoint::ReportStatus::StepPassed,
            summary: summary.to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        }
    }

    fn escalate_report(reason: &str) -> AgentReport {
        AgentReport {
            status: orrery_core::checkpoint::ReportStatus::Escalate,
            summary: "escalating to operator".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: Some(reason.to_string()),
            artifacts: Vec::new(),
        }
    }

    fn push_one_step_pass(driver: &MockAgentDriver, summary: &str) {
        driver.push("orch-1", Ok(dispatch_report(1, "coder-1")));
        driver.push("coder-1", Ok(complete_report()));
        driver.push("orch-1", Ok(dispatch_report(2, "val-1")));
        driver.push("val-1", Ok(confirmed_report()));
        driver.push("orch-1", Ok(step_passed_report(summary)));
    }

    async fn setup_runnable_project(
        dir: &Path,
        plan_markdown: &str,
        chat_driver: Arc<MockAgentDriver>,
        secrets: Arc<InMemorySecretStore>,
    ) -> (CliContext, SharedVec, SharedVec) {
        let (mut ctx, out, err) = test_ctx();
        ctx.secrets = secrets.clone();
        ctx.make_backends = Box::new(move |_root: &Path| {
            let chat: Arc<dyn AgentDriver> = chat_driver.clone();
            let cli: Arc<dyn AgentDriver> = Arc::new(MockAgentDriver::new());
            Ok(Backends {
                secrets: secrets.clone(),
                containers: Arc::new(MockContainerBackend::new()),
                inference_control: Arc::new(MockInferenceControl::new()),
                connector: Arc::new(MockMcpConnector {
                    transports: HashMap::new(),
                }),
                drivers: DriverSet { chat, cli },
            })
        });

        dispatch(
            Command::Init {
                path: None,
                name: Some("run-proj".to_string()),
            },
            dir,
            &mut ctx,
        )
        .await
        .expect("init");

        std::fs::write(dir.join("STEPPED_IMPLEMENTATION_PLAN.md"), plan_markdown)
            .expect("write plan");

        dispatch(
            Command::Inference {
                command: InferenceCmd::Add {
                    id: "ext-inf".to_string(),
                    endpoint: "http://192.0.2.10:8080".to_string(),
                    mode: InferenceModeArg::Single,
                    container: None,
                    compose: None,
                    service: None,
                    no_unload: false,
                    startup_timeout: 300,
                },
            },
            dir,
            &mut ctx,
        )
        .await
        .expect("add inference");

        for (id, name) in [
            ("orch-1", "Orchestrator"),
            ("coder-1", "Coder"),
            ("val-1", "Validator"),
        ] {
            dispatch(
                Command::Agent {
                    command: AgentCmd::AddLocalModel {
                        id: id.to_string(),
                        name: name.to_string(),
                        inference: "ext-inf".to_string(),
                        model: "test-model".to_string(),
                        temperature: 0.1,
                        context: 32768,
                    },
                },
                dir,
                &mut ctx,
            )
            .await
            .expect("add agent");
        }

        for (stage, agent_id) in [(0, "orch-1"), (1, "coder-1"), (2, "val-1")] {
            dispatch(
                Command::Workflow {
                    command: WorkflowCmd::Assign {
                        stage,
                        agent_id: agent_id.to_string(),
                    },
                },
                dir,
                &mut ctx,
            )
            .await
            .expect("assign agent");
        }

        out.clear();
        err.clear();
        (ctx, out, err)
    }

    #[tokio::test]
    async fn run_mode_passes_one_step_exit_0() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "step 1 done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, out, _err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nbody 1\n## Step 2: Second\nbody 2\n",
            driver,
            secrets,
        )
        .await;

        let code = dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run should succeed");
        assert_eq!(code, 0);

        let printed = out.as_string();
        assert!(
            printed.contains("[ReportSaved] orch-1: StepPassed: step 1 done"),
            "got: {printed}"
        );

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let state = checkpoints
            .load_state()
            .expect("load_state")
            .expect("checkpoint");
        assert_eq!(
            state.step_statuses,
            vec![StepStatus::Passed, StepStatus::Pending]
        );
    }

    #[tokio::test]
    async fn run_loop_cycles_two() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "step 1 done");
        push_one_step_pass(&driver, "step 2 done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, _out, _err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nb1\n## Step 2: Second\nb2\n## Step 3: Third\nb3\n",
            driver,
            secrets,
        )
        .await;

        let code = dispatch(
            Command::Run {
                r#loop: true,
                cycles: Some(2),
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run --loop --cycles 2");
        assert_eq!(code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let state = checkpoints
            .load_state()
            .expect("load_state")
            .expect("checkpoint");
        assert_eq!(
            state.step_statuses,
            vec![StepStatus::Passed, StepStatus::Passed, StepStatus::Pending]
        );
    }

    #[tokio::test]
    async fn escalation_exits_3_then_approve_resume_completes() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        driver.push("orch-1", Ok(escalate_report("ambiguous requirement")));
        push_one_step_pass(&driver, "step 1 resolved");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, _out, _err) =
            setup_runnable_project(dir.path(), "## Step 1: First\nb1\n", driver, secrets).await;

        let code_esc = dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run escalates");
        assert_eq!(code_esc, 3);

        let approve_code = dispatch(Command::Approve, dir.path(), &mut ctx)
            .await
            .expect("approve");
        assert_eq!(approve_code, 0);

        let resume_code = dispatch(Command::Resume, dir.path(), &mut ctx)
            .await
            .expect("resume");
        assert_eq!(resume_code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let state = checkpoints
            .load_state()
            .expect("load_state")
            .expect("checkpoint");
        assert_eq!(state.step_statuses, vec![StepStatus::Passed]);
    }

    #[tokio::test]
    async fn reject_records_feedback() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        driver.push("orch-1", Ok(escalate_report("need guidance")));
        push_one_step_pass(&driver, "completed after reject");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, _out, _err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nb1\n",
            driver.clone(),
            secrets,
        )
        .await;

        let code_esc = dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run escalates");
        assert_eq!(code_esc, 3);

        let feedback_msg = "use bounded retry backoff";
        let reject_code = dispatch(
            Command::Reject {
                feedback: feedback_msg.to_string(),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("reject");
        assert_eq!(reject_code, 0);

        let resume_code = dispatch(Command::Resume, dir.path(), &mut ctx)
            .await
            .expect("resume");
        assert_eq!(resume_code, 0);

        let invs = driver.invocations();
        let second_orch = invs
            .iter()
            .filter(|inv| inv.agent.agent_id == "orch-1")
            .nth(1)
            .expect("second orchestrator invocation");
        assert!(
            second_orch.user_prompt.contains(feedback_msg),
            "expected feedback in prompt, got: {}",
            second_orch.user_prompt
        );
    }

    #[tokio::test]
    async fn approve_without_awaiting_step_exits_1() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, _out, _err) =
            setup_runnable_project(dir.path(), "## Step 1: First\nb1\n", driver, secrets).await;

        // No checkpoint on disk yet.
        let err_no_cp = dispatch(Command::Approve, dir.path(), &mut ctx)
            .await
            .expect_err("approve without checkpoint must fail");
        assert_eq!(exit_code_for(&err_no_cp), 1);
        assert!(err_no_cp.to_string().contains("no step awaiting operator"));

        // Run step to Passed, then try approve/reject when no step is AwaitingOperator.
        dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run");

        let err_passed = dispatch(Command::Approve, dir.path(), &mut ctx)
            .await
            .expect_err("approve on passed step must fail");
        assert_eq!(exit_code_for(&err_passed), 1);

        let err_reject = dispatch(
            Command::Reject {
                feedback: "too late".to_string(),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect_err("reject on passed step must fail");
        assert_eq!(exit_code_for(&err_reject), 1);
    }

    #[tokio::test]
    async fn reset_step_one_marks_pending() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "step 1 done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, _out, _err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nb1\n## Step 2: Second\nb2\n",
            driver,
            secrets,
        )
        .await;

        dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run");

        let reset_code = dispatch(Command::Reset { step: 1 }, dir.path(), &mut ctx)
            .await
            .expect("reset --step 1");
        assert_eq!(reset_code, 0);

        let store = ConfigStore::new(dir.path(), None).expect("store");
        let checkpoints = CheckpointManager::new(store.orrery_dir().join("checkpoints"));
        let state = checkpoints
            .load_state()
            .expect("load_state")
            .expect("checkpoint");
        assert_eq!(
            state.step_statuses,
            vec![StepStatus::Pending, StepStatus::Pending]
        );
        assert_eq!(state.current_step_index, 0);
        assert_eq!(state.phase, EnginePhase::Idle);
    }

    #[tokio::test]
    async fn status_json_shape() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "step 1 done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, out, _err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nb1\n## Step 2: Second\nb2\n",
            driver,
            secrets,
        )
        .await;

        // Before any run (no checkpoint on disk yet).
        let pre_code = dispatch(Command::Status { json: true }, dir.path(), &mut ctx)
            .await
            .expect("status --json before run");
        assert_eq!(pre_code, 0);
        let pre_val: serde_json::Value =
            serde_json::from_str(&out.as_string()).expect("parse pre status json");
        assert!(pre_val["run_id"].is_null());
        assert_eq!(pre_val["phase"], "Idle");
        assert_eq!(pre_val["current_step"], 1);
        assert_eq!(pre_val["round"], 0);
        assert_eq!(pre_val["retry_count"], 0);
        assert!(pre_val["awaiting_reason"].is_null());

        dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run");
        out.clear();

        let code = dispatch(Command::Status { json: true }, dir.path(), &mut ctx)
            .await
            .expect("status --json");
        assert_eq!(code, 0);

        let val: serde_json::Value =
            serde_json::from_str(&out.as_string()).expect("parse status json");
        assert!(val["run_id"].is_string());
        assert_eq!(val["phase"], "Done");
        assert_eq!(val["current_step"], 1);
        assert_eq!(val["round"], 3);
        assert_eq!(val["retry_count"], 0);
        assert!(val["awaiting_reason"].is_null());
        let steps = val["steps"].as_array().expect("steps array");
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[0],
            serde_json::json!({"number": 1, "title": "1: First", "status": "Passed"})
        );
        assert_eq!(
            steps[1],
            serde_json::json!({"number": 2, "title": "2: Second", "status": "Pending"})
        );

        out.clear();
        let plain_code = dispatch(Command::Status { json: false }, dir.path(), &mut ctx)
            .await
            .expect("status plain");
        assert_eq!(plain_code, 0);
        let plain = out.as_string();
        assert!(plain.contains("[Passed] 1: First"), "got: {plain}");
        assert!(plain.contains("[Pending] 2: Second"), "got: {plain}");
    }

    #[tokio::test]
    async fn logs_tail_shows_redacted_lines() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        let raw_secret = "super-secret-api-token-778899";
        push_one_step_pass(&driver, &format!("done with token {raw_secret}"));

        let secrets = Arc::new(InMemorySecretStore::new());
        secrets
            .set("orrery.cloud", "testuser", &SecretValue::new(raw_secret))
            .expect("store secret");

        let (mut ctx, out, _err) =
            setup_runnable_project(dir.path(), "## Step 1: First\nb1\n", driver, secrets).await;

        // Register a CloudApi agent referencing that keychain entry so build_engine registers the secret with SecretRedactor.
        dispatch(
            Command::Agent {
                command: AgentCmd::AddCloud {
                    id: "cloud-helper".to_string(),
                    name: "Cloud Helper".to_string(),
                    endpoint: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "example-model".to_string(),
                    keychain_service: "orrery.cloud".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("add cloud agent");

        dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run");
        out.clear();

        let code = dispatch(
            Command::Logs {
                tail: Some(5),
                follow: false,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("logs --tail 5");
        assert_eq!(code, 0);

        let logged = out.as_string();
        assert!(!logged.is_empty());
        assert!(!logged.contains(raw_secret), "secret leaked: {logged}");
        assert!(
            logged.contains("[REDACTED]"),
            "expected redaction: {logged}"
        );
    }

    fn cli_equivalent(a: &orrery_tui::app::AppAction) -> Option<Vec<String>> {
        use orrery_tui::app::AppAction;
        match a {
            AppAction::Run => Some(vec!["orrery".into(), "run".into()]),
            AppAction::Approve => Some(vec!["orrery".into(), "approve".into()]),
            AppAction::Reject(f) => Some(vec![
                "orrery".into(),
                "reject".into(),
                "--feedback".into(),
                f.clone(),
            ]),
            AppAction::IngestPlan(p) => Some(vec![
                "orrery".into(),
                "ingest".into(),
                p.to_string_lossy().into_owned(),
            ]),
            AppAction::SetMode(mode) => {
                let mode_str = match mode {
                    ExecutionMode::Run => "run",
                    ExecutionMode::Loop => "loop",
                };
                Some(vec![
                    "orrery".into(),
                    "workflow".into(),
                    "mode".into(),
                    mode_str.into(),
                ])
            }
            AppAction::SetCycles(n) => Some(vec![
                "orrery".into(),
                "workflow".into(),
                "mode".into(),
                "loop".into(),
                "--cycles".into(),
                n.to_string(),
            ]),
            AppAction::SetStageCount(n) => Some(vec![
                "orrery".into(),
                "workflow".into(),
                "stages".into(),
                n.to_string(),
            ]),
            AppAction::SetStageRole { stage, role_id } => Some(vec![
                "orrery".into(),
                "workflow".into(),
                "set-role".into(),
                stage.to_string(),
                role_id.clone(),
            ]),
            AppAction::AssignAgent { stage, agent_id } => Some(vec![
                "orrery".into(),
                "workflow".into(),
                "assign".into(),
                stage.to_string(),
                agent_id.clone(),
            ]),
            AppAction::UnassignAgent { stage, agent_id } => Some(vec![
                "orrery".into(),
                "workflow".into(),
                "unassign".into(),
                stage.to_string(),
                agent_id.clone(),
            ]),
            AppAction::Interrupt | AppAction::SetMouseCapture(_) | AppAction::Quit => None,
        }
    }

    #[test]
    fn tui_action_parity() {
        use clap::Parser;
        use orrery_tui::app::AppAction;

        let actions = [
            AppAction::Run,
            AppAction::Interrupt,
            AppAction::Approve,
            AppAction::Reject("needs tests".to_string()),
            AppAction::IngestPlan(PathBuf::from("plan.md")),
            AppAction::SetMode(ExecutionMode::Run),
            AppAction::SetMode(ExecutionMode::Loop),
            AppAction::SetCycles(3),
            AppAction::SetStageCount(4),
            AppAction::SetStageRole {
                stage: 1,
                role_id: "coder".to_string(),
            },
            AppAction::AssignAgent {
                stage: 1,
                agent_id: "coder-1".to_string(),
            },
            AppAction::UnassignAgent {
                stage: 1,
                agent_id: "coder-1".to_string(),
            },
            AppAction::SetMouseCapture(false),
            AppAction::Quit,
        ];

        for action in &actions {
            match cli_equivalent(action) {
                Some(argv) => {
                    let parsed = crate::args::Cli::try_parse_from(&argv);
                    assert!(
                        parsed.is_ok(),
                        "expected {argv:?} from {action:?} to parse: {parsed:?}"
                    );
                }
                None => {
                    assert!(
                        matches!(
                            action,
                            AppAction::Interrupt | AppAction::SetMouseCapture(_) | AppAction::Quit
                        ),
                        "unexpected None for {action:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn render_error_includes_source() {
        let err = Error::Io {
            path: "plan.md".into(),
            source: io::Error::new(io::ErrorKind::NotFound, "missing"),
        };
        let rendered = render_error(&err);
        assert!(rendered.contains("plan.md"), "rendered: {rendered}");
        assert!(rendered.contains("missing"), "rendered: {rendered}");
    }

    #[tokio::test]
    async fn status_before_ingest_hints_ingest() {
        let dir = TempDir::new().expect("tempdir");
        let (mut ctx, _out, _err) = test_ctx();

        dispatch(
            Command::Init {
                path: None,
                name: Some("demo".into()),
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("init");

        let err = dispatch(Command::Status { json: false }, dir.path(), &mut ctx)
            .await
            .expect_err("status before ingest should fail");
        match err {
            Error::Validation { field, message } => {
                assert_eq!(field, "plan_path");
                assert!(
                    message.contains("orrery ingest"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn status_reports_edited_step() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "step 1 done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, _out, _err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nb1\n## Step 2: Second\nb2\n",
            driver,
            secrets,
        )
        .await;

        dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run");

        std::fs::write(
            dir.path().join("STEPPED_IMPLEMENTATION_PLAN.md"),
            "## Step 1: First\nb1 edited\n## Step 2: Second\nb2\n",
        )
        .expect("overwrite plan");

        let err = dispatch(Command::Status { json: false }, dir.path(), &mut ctx)
            .await
            .expect_err("status on edited passed step must fail");
        assert!(
            err.to_string().contains("step edited"),
            "expected 'step edited' in error, got: {err}"
        );
    }

    #[tokio::test]
    async fn run_prints_startup_notices() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        push_one_step_pass(&driver, "step 1 done");
        let secrets = Arc::new(InMemorySecretStore::new());
        let (mut ctx, out, _err) =
            setup_runnable_project(dir.path(), "## Step 1: First\nb1\n", driver, secrets).await;

        let store = ConfigStore::new(dir.path(), None).expect("store");
        store
            .save_mcp_server(&McpServerConfig {
                server_id: "unused".to_string(),
                transport: McpTransportConfig::Stdio {
                    command: "fake-mcp".to_string(),
                    args: Vec::new(),
                },
                write_tools: Vec::new(),
            })
            .expect("save unused mcp server");

        let code = dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await
        .expect("run");
        assert_eq!(code, 0);

        let stdout = out.as_string();
        assert!(
            stdout.contains("is not used by this workflow"),
            "expected unused server startup notice in stdout, got: {stdout}"
        );
    }

    #[tokio::test]
    async fn run_error_is_redacted() {
        let dir = TempDir::new().expect("tempdir");
        let driver = Arc::new(MockAgentDriver::new());
        let secrets = Arc::new(InMemorySecretStore::new());
        secrets
            .set(
                "orrery.test",
                "testuser",
                &SecretValue::new("sk-test-leak-123"),
            )
            .expect("store secret");

        let (mut ctx, _out, err) = setup_runnable_project(
            dir.path(),
            "## Step 1: First\nb1\n",
            driver.clone(),
            secrets,
        )
        .await;

        let store = ConfigStore::new(dir.path(), None).expect("store");
        store
            .save_agent(&AgentConfig {
                agent_id: "cloud-orch".to_string(),
                display_name: "Cloud Orchestrator".to_string(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "cloud-model".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 8192,
                    keychain_service: "orrery.test".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            })
            .expect("save cloud-orch");
        let mut project = store.load_project().expect("load project");
        project
            .replace_agent(0, 0, "cloud-orch")
            .expect("replace stage 0 agent");
        store.save_project(&project).expect("save project");

        driver.push(
            "cloud-orch",
            Err(Error::Agent {
                agent_id: "cloud-orch".into(),
                message: "upstream echoed sk-test-leak-123".into(),
            }),
        );

        let res = dispatch(
            Command::Run {
                r#loop: false,
                cycles: None,
                step: None,
            },
            dir.path(),
            &mut ctx,
        )
        .await;
        assert_eq!(res.expect("run"), 1);

        let stderr = err.as_string();
        assert!(stderr.contains("[REDACTED]"), "stderr: {stderr}");
        assert!(!stderr.contains("sk-test-leak-123"), "stderr: {stderr}");
    }
}
