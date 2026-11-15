use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use orrery_core::config::{
    ConfigStore, ExecutionMode, HookConfig, HookEvent, HookFailure, ProjectConfig,
};
use orrery_core::engine::Backends;
use orrery_core::plan::ImplementationPlan;
use orrery_core::security::SecretStore;
use orrery_core::{Error, Result};

use crate::args::{Command, GateCmd, HookCmd, WorkflowCmd};

pub type ReadSecretFn = Box<dyn FnMut() -> Result<String>>;
pub type MakeBackendsFn = Box<dyn Fn(&Path) -> Result<Backends> + Send + Sync>;

pub struct CliContext {
    pub out: Box<dyn Write>,
    pub err: Box<dyn Write>,
    pub secrets: Arc<dyn SecretStore>,
    pub read_secret: ReadSecretFn,
    pub make_backends: MakeBackendsFn,
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
                write_stdout_line(ctx, &format!("Step {}: {}", step.number, step.title))?;
            }
            Ok(0)
        }
        Command::Workflow { command } => dispatch_workflow(command, project, ctx),
        Command::Gate { command } => dispatch_gate(command, project, ctx),
        Command::Hook { command } => dispatch_hook(command, project, ctx),
        Command::Tui { .. }
        | Command::Agent { .. }
        | Command::Inference { .. }
        | Command::Secret { .. }
        | Command::Role { .. }
        | Command::Mcp { .. }
        | Command::Run { .. }
        | Command::Approve
        | Command::Reject { .. }
        | Command::Resume
        | Command::Reset { .. }
        | Command::Status { .. }
        | Command::Logs { .. } => {
            let _ = (&ctx.secrets, &ctx.read_secret, &ctx.make_backends);
            Err(Error::Validation {
                field: "command".into(),
                message: "not implemented until Step 35/36".into(),
            })
        }
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

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use orrery_core::config::{
        ConfigStore, ExecutionMode, HookConfig, HookFailure, ValidationMode,
    };
    use orrery_core::security::InMemorySecretStore;
    use tempfile::TempDir;

    use super::*;
    use crate::args::{HookEventArg, HookFailureArg, ModeArg};

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
        assert!(printed.contains("Step 1: 1: Setup"), "got: {printed}");
        assert!(printed.contains("Step 2: 2: Build"), "got: {printed}");

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

        // Verify exit_code_for Interrupted and unimplemented stub.
        assert_eq!(exit_code_for(&Error::Interrupted), 130);
        let stub_err = dispatch(Command::Approve, dir.path(), &mut ctx)
            .await
            .expect_err("unimplemented command must error");
        assert_eq!(
            stub_err.to_string(),
            "invalid command: not implemented until Step 35/36"
        );
    }
}
