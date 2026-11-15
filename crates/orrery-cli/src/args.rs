use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use orrery_core::config::{
    ExecutionMode, HookEvent, HookFailure, InferenceMode, RoleKind, WorkspaceMount,
};

#[derive(Debug, Clone, PartialEq, Parser)]
#[command(name = "orrery")]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    pub project: PathBuf,
    #[command(subcommand)]
    pub command: Option<Command>,
    pub path: Option<PathBuf>,
}

impl Cli {
    pub fn validate(&self) -> Result<(), clap::Error> {
        if self.path.is_some() && self.command.is_some() {
            return Err(<Cli as clap::CommandFactory>::command().error(
                clap::error::ErrorKind::ArgumentConflict,
                "a PATH argument cannot be combined with a subcommand; use --project <PATH>",
            ));
        }
        Ok(())
    }

    pub fn into_command(self) -> Command {
        self.command.unwrap_or_else(|| Command::Tui {
            path: self.path.unwrap_or_else(|| PathBuf::from(".")),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum Command {
    #[command(skip)]
    Tui {
        path: PathBuf,
    },
    Init {
        path: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
    },
    Ingest {
        md_file: PathBuf,
        #[arg(long)]
        marker: Option<String>,
    },
    Workflow {
        #[command(subcommand)]
        command: WorkflowCmd,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCmd,
    },
    Inference {
        #[command(subcommand)]
        command: InferenceCmd,
    },
    Secret {
        #[command(subcommand)]
        command: SecretCmd,
    },
    Role {
        #[command(subcommand)]
        command: RoleCmd,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCmd,
    },
    Gate {
        #[command(subcommand)]
        command: GateCmd,
    },
    Hook {
        #[command(subcommand)]
        command: HookCmd,
    },
    Run {
        #[arg(long = "loop")]
        r#loop: bool,
        #[arg(long)]
        cycles: Option<usize>,
        #[arg(long)]
        step: Option<usize>,
    },
    Approve,
    Reject {
        #[arg(long)]
        feedback: String,
    },
    Resume,
    Reset {
        #[arg(long)]
        step: usize,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
    Logs {
        #[arg(long)]
        tail: Option<usize>,
        #[arg(long)]
        follow: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ModeArg {
    Run,
    Loop,
}

impl From<ModeArg> for ExecutionMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Run => Self::Run,
            ModeArg::Loop => Self::Loop,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum WorkflowCmd {
    Show {
        #[arg(long)]
        json: bool,
    },
    Stages {
        #[arg(value_parser = clap::value_parser!(u8).range(2..=15))]
        count: u8,
    },
    SetRole {
        stage: usize,
        role_id: String,
    },
    Assign {
        stage: usize,
        agent_id: String,
    },
    Unassign {
        stage: usize,
        agent_id: String,
    },
    Clear {
        stage: usize,
    },
    Mode {
        #[arg(value_enum)]
        mode: ModeArg,
        #[arg(long)]
        cycles: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum MountArg {
    #[default]
    None,
    Ro,
    Rw,
}

impl From<MountArg> for WorkspaceMount {
    fn from(value: MountArg) -> Self {
        match value {
            MountArg::None => Self::None,
            MountArg::Ro => Self::ReadOnly,
            MountArg::Rw => Self::ReadWrite,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum AgentCmd {
    AddLocalModel {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        inference: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value_t = 0.1)]
        temperature: f64,
        #[arg(long, default_value_t = 32768)]
        context: usize,
    },
    AddCli {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        image: String,
        #[arg(long, required = true, num_args = 1..)]
        command: Vec<String>,
        #[arg(long, value_enum, default_value_t = MountArg::None)]
        mount: MountArg,
        #[arg(long, default_value_t = 4096)]
        memory_mb: u64,
        #[arg(long, default_value_t = 1800)]
        timeout: u64,
        #[arg(long)]
        secret_env: Option<String>,
        #[arg(long)]
        keychain_service: Option<String>,
        #[arg(long)]
        keychain_account: Option<String>,
    },
    AddCloud {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        keychain_service: String,
        #[arg(long)]
        keychain_account: String,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Show {
        id: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum InferenceModeArg {
    #[default]
    Single,
    Router,
}

impl From<InferenceModeArg> for InferenceMode {
    fn from(value: InferenceModeArg) -> Self {
        match value {
            InferenceModeArg::Single => Self::Single,
            InferenceModeArg::Router => Self::Router,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum InferenceCmd {
    Add {
        #[arg(long)]
        id: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long, value_enum, default_value_t = InferenceModeArg::Single)]
        mode: InferenceModeArg,
        #[arg(long, conflicts_with = "compose")]
        container: Option<String>,
        #[arg(long, conflicts_with = "container")]
        compose: Option<PathBuf>,
        #[arg(long, requires = "compose")]
        service: Option<String>,
        #[arg(long)]
        no_unload: bool,
        #[arg(long, default_value_t = 300)]
        startup_timeout: u64,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum SecretCmd {
    Set {
        #[arg(long)]
        service: String,
        #[arg(long)]
        account: String,
    },
    Delete {
        #[arg(long)]
        service: String,
        #[arg(long)]
        account: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RoleKindArg {
    Orchestrator,
    Implementer,
    Validator,
}

impl From<RoleKindArg> for RoleKind {
    fn from(value: RoleKindArg) -> Self {
        match value {
            RoleKindArg::Orchestrator => Self::Orchestrator,
            RoleKindArg::Implementer => Self::Implementer,
            RoleKindArg::Validator => Self::Validator,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum RoleCmd {
    Add {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        kind: RoleKindArg,
        #[arg(long)]
        rules_file: PathBuf,
        #[arg(long = "allow-tool")]
        allow_tools: Vec<String>,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Show {
        id: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum McpCmd {
    AddStdio {
        #[arg(long)]
        id: String,
        #[arg(long)]
        command: String,
        #[arg(long = "arg", allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long = "write-tool")]
        write_tools: Vec<String>,
    },
    AddHttp {
        #[arg(long)]
        id: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        keychain_service: Option<String>,
        #[arg(long)]
        keychain_account: Option<String>,
        #[arg(long = "write-tool")]
        write_tools: Vec<String>,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Tools {
        id: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum GateCmd {
    Set {
        #[arg(required = true, num_args = 1..)]
        tools: Vec<String>,
    },
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HookEventArg {
    Start,
    Pass,
    Retry,
    Halt,
}

impl From<HookEventArg> for HookEvent {
    fn from(value: HookEventArg) -> Self {
        match value {
            HookEventArg::Start => Self::StepStart,
            HookEventArg::Pass => Self::StepPass,
            HookEventArg::Retry => Self::StepRetry,
            HookEventArg::Halt => Self::Halt,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum HookFailureArg {
    #[default]
    Halt,
    Warn,
}

impl From<HookFailureArg> for HookFailure {
    fn from(value: HookFailureArg) -> Self {
        match value {
            HookFailureArg::Halt => Self::Halt,
            HookFailureArg::Warn => Self::Warn,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum HookCmd {
    Set {
        #[arg(value_enum)]
        event: HookEventArg,
        tool: String,
        #[arg(long, value_enum, default_value_t = HookFailureArg::Halt)]
        on_failure: HookFailureArg,
    },
    Clear {
        #[arg(value_enum)]
        event: HookEventArg,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_row_parses() {
        let rows: &[&[&str]] = &[
            &["orrery", "./workspace"],
            &["orrery", "init", "./workspace", "--name", "demo"],
            &["orrery", "ingest", "plan.md", "--marker", "## Step "],
            &["orrery", "workflow", "show", "--json"],
            &["orrery", "workflow", "stages", "3"],
            &["orrery", "workflow", "set-role", "1", "coder"],
            &["orrery", "workflow", "assign", "1", "local-coder"],
            &["orrery", "workflow", "unassign", "1", "local-coder"],
            &["orrery", "workflow", "clear", "1"],
            &["orrery", "workflow", "mode", "loop", "--cycles", "2"],
            &[
                "orrery",
                "agent",
                "add-local-model",
                "--id",
                "local-coder",
                "--name",
                "Local Coder",
                "--inference",
                "coder-14b",
                "--model",
                "qwen-coder",
                "--temperature",
                "0.2",
                "--context",
                "32768",
            ],
            &[
                "orrery",
                "agent",
                "add-cli",
                "--id",
                "cli-val",
                "--name",
                "CLI Validator",
                "--image",
                "registry.example.com/agents/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "--command",
                "agent-cli",
                "run",
                "--mount",
                "ro",
                "--memory-mb",
                "4096",
                "--timeout",
                "1800",
                "--secret-env",
                "AGENT_TOKEN",
                "--keychain-service",
                "orrery.agent",
                "--keychain-account",
                "testuser",
            ],
            &[
                "orrery",
                "agent",
                "add-cloud",
                "--id",
                "cloud-val",
                "--name",
                "Cloud Validator",
                "--endpoint",
                "https://api.example.com/v1/chat/completions",
                "--model",
                "example-model",
                "--keychain-service",
                "orrery.cloud",
                "--keychain-account",
                "testuser",
            ],
            &["orrery", "agent", "list", "--json"],
            &["orrery", "agent", "show", "local-coder"],
            &["orrery", "agent", "remove", "local-coder"],
            &[
                "orrery",
                "inference",
                "add",
                "--id",
                "coder-14b",
                "--endpoint",
                "http://192.0.2.10:8080",
                "--mode",
                "single",
                "--compose",
                "docker-models/compose.yaml",
                "--service",
                "llama",
                "--no-unload",
                "--startup-timeout",
                "120",
            ],
            &["orrery", "inference", "list", "--json"],
            &["orrery", "inference", "remove", "coder-14b"],
            &[
                "orrery",
                "secret",
                "set",
                "--service",
                "orrery.cloud",
                "--account",
                "testuser",
            ],
            &[
                "orrery",
                "secret",
                "delete",
                "--service",
                "orrery.cloud",
                "--account",
                "testuser",
            ],
            &[
                "orrery",
                "role",
                "add",
                "--id",
                "reviewer",
                "--name",
                "Reviewer",
                "--kind",
                "validator",
                "--rules-file",
                "reviewer.md",
                "--allow-tool",
                "fs:read_file",
                "--allow-tool",
                "git:diff",
            ],
            &["orrery", "role", "list", "--json"],
            &["orrery", "role", "show", "reviewer"],
            &["orrery", "role", "remove", "reviewer"],
            &[
                "orrery",
                "mcp",
                "add-stdio",
                "--id",
                "fs",
                "--command",
                "mcp-fs",
                "--arg",
                "--stdio",
                "--write-tool",
                "write_file:path,dest",
            ],
            &[
                "orrery",
                "mcp",
                "add-http",
                "--id",
                "remote",
                "--endpoint",
                "https://mcp.example.com/v1",
                "--keychain-service",
                "orrery.mcp",
                "--keychain-account",
                "testuser",
                "--write-tool",
                "put_file:path",
            ],
            &["orrery", "mcp", "list", "--json"],
            &["orrery", "mcp", "tools", "fs"],
            &["orrery", "mcp", "remove", "fs"],
            &["orrery", "gate", "set", "fs:test", "fs:lint"],
            &["orrery", "gate", "clear"],
            &[
                "orrery",
                "hook",
                "set",
                "start",
                "fs:notify",
                "--on-failure",
                "warn",
            ],
            &["orrery", "hook", "clear", "start"],
            &["orrery", "run", "--loop", "--cycles", "3", "--step", "1"],
            &["orrery", "approve"],
            &["orrery", "reject", "--feedback", "needs test coverage"],
            &["orrery", "resume"],
            &["orrery", "reset", "--step", "2"],
            &["orrery", "status", "--json"],
            &["orrery", "logs", "--tail", "100", "--follow"],
        ];

        for argv in rows {
            let parsed = Cli::try_parse_from(*argv);
            assert!(parsed.is_ok(), "failed to parse {argv:?}: {parsed:?}");
        }
    }

    #[test]
    fn bare_orrery_is_tui_dot() {
        let res = Cli::try_parse_from(["orrery"]);
        assert!(res.is_ok());
        if let Ok(cli) = res {
            assert_eq!(cli.project, PathBuf::from("."));
            assert_eq!(
                cli.into_command(),
                Command::Tui {
                    path: PathBuf::from(".")
                }
            );
        }
    }

    #[test]
    fn orrery_path_is_tui_path() {
        let res = Cli::try_parse_from(["orrery", "./some/dir"]);
        assert!(res.is_ok());
        if let Ok(cli) = res {
            assert_eq!(
                cli.into_command(),
                Command::Tui {
                    path: PathBuf::from("./some/dir")
                }
            );
        }
    }

    #[test]
    fn stage_count_out_of_range_rejected() {
        assert!(Cli::try_parse_from(["orrery", "workflow", "stages", "1"]).is_err());
        assert!(Cli::try_parse_from(["orrery", "workflow", "stages", "16"]).is_err());

        let min_ok = Cli::try_parse_from(["orrery", "workflow", "stages", "2"]);
        assert!(min_ok.is_ok());
        if let Ok(cli) = min_ok {
            assert_eq!(
                cli.into_command(),
                Command::Workflow {
                    command: WorkflowCmd::Stages { count: 2 }
                }
            );
        }

        let max_ok = Cli::try_parse_from(["orrery", "workflow", "stages", "15"]);
        assert!(max_ok.is_ok());
        if let Ok(cli) = max_ok {
            assert_eq!(
                cli.into_command(),
                Command::Workflow {
                    command: WorkflowCmd::Stages { count: 15 }
                }
            );
        }
    }

    #[test]
    fn project_flag_is_global() {
        let after = Cli::try_parse_from(["orrery", "status", "--project", "./x"]);
        assert!(after.is_ok(), "after failed: {after:?}");
        if let Ok(cli) = after {
            assert_eq!(cli.project, PathBuf::from("./x"));
            assert_eq!(cli.into_command(), Command::Status { json: false });
        }

        let before = Cli::try_parse_from(["orrery", "--project", "./x", "status"]);
        assert!(before.is_ok(), "before failed: {before:?}");
        if let Ok(cli) = before {
            assert_eq!(cli.project, PathBuf::from("./x"));
            assert_eq!(cli.into_command(), Command::Status { json: false });
        }
    }

    #[test]
    fn path_with_subcommand_is_usage_error() {
        let both = Cli::try_parse_from(["orrery", "./x", "status"]);
        assert!(both.is_ok(), "expected parse ok: {both:?}");
        if let Ok(cli) = both {
            let err = cli.validate();
            assert!(err.is_err());
            if let Err(e) = err {
                assert_eq!(e.kind(), clap::error::ErrorKind::ArgumentConflict);
            }
        }

        let path_only = Cli::try_parse_from(["orrery", "./x"]);
        assert!(path_only.is_ok());
        if let Ok(cli) = path_only {
            assert!(cli.validate().is_ok());
        }

        let sub_only = Cli::try_parse_from(["orrery", "status"]);
        assert!(sub_only.is_ok());
        if let Ok(cli) = sub_only {
            assert!(cli.validate().is_ok());
        }

        let flag_and_sub = Cli::try_parse_from(["orrery", "--project", "./x", "status"]);
        assert!(flag_and_sub.is_ok());
        if let Ok(cli) = flag_and_sub {
            assert!(cli.validate().is_ok());
            assert_eq!(cli.project, PathBuf::from("./x"));
            assert_eq!(cli.into_command(), Command::Status { json: false });
        }
    }

    #[test]
    fn mount_and_inference_flags() {
        let mount_rw = Cli::try_parse_from([
            "orrery",
            "agent",
            "add-cli",
            "--id",
            "a1",
            "--name",
            "Agent",
            "--image",
            "registry.example.com/a@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "--command",
            "cmd",
            "--mount",
            "rw",
        ]);
        assert!(mount_rw.is_ok(), "mount rw failed: {mount_rw:?}");
        if let Ok(cli) = mount_rw {
            assert_eq!(
                cli.into_command(),
                Command::Agent {
                    command: AgentCmd::AddCli {
                        id: "a1".to_string(),
                        name: "Agent".to_string(),
                        image: "registry.example.com/a@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                        command: vec!["cmd".to_string()],
                        mount: MountArg::Rw,
                        memory_mb: 4096,
                        timeout: 1800,
                        secret_env: None,
                        keychain_service: None,
                        keychain_account: None,
                    }
                }
            );
        }

        let mode_router = Cli::try_parse_from([
            "orrery",
            "inference",
            "add",
            "--id",
            "inf1",
            "--endpoint",
            "http://192.0.2.10:8080",
            "--mode",
            "router",
            "--container",
            "llama-router",
        ]);
        assert!(mode_router.is_ok(), "mode router failed: {mode_router:?}");
        if let Ok(cli) = mode_router {
            assert_eq!(
                cli.into_command(),
                Command::Inference {
                    command: InferenceCmd::Add {
                        id: "inf1".to_string(),
                        endpoint: "http://192.0.2.10:8080".to_string(),
                        mode: InferenceModeArg::Router,
                        container: Some("llama-router".to_string()),
                        compose: None,
                        service: None,
                        no_unload: false,
                        startup_timeout: 300,
                    }
                }
            );
        }

        assert!(
            Cli::try_parse_from([
                "orrery",
                "agent",
                "add-cli",
                "--id",
                "a1",
                "--name",
                "Agent",
                "--image",
                "registry.example.com/a@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "--command",
                "cmd",
                "--mount",
                "bad",
            ])
            .is_err()
        );

        assert!(
            Cli::try_parse_from([
                "orrery",
                "inference",
                "add",
                "--id",
                "inf1",
                "--endpoint",
                "http://192.0.2.10:8080",
                "--mode",
                "other",
            ])
            .is_err()
        );

        assert!(
            Cli::try_parse_from([
                "orrery",
                "inference",
                "add",
                "--id",
                "inf1",
                "--endpoint",
                "http://192.0.2.10:8080",
                "--container",
                "llama-1",
                "--compose",
                "docker-models/compose.yaml",
            ])
            .is_err()
        );

        assert_eq!(ExecutionMode::from(ModeArg::Run), ExecutionMode::Run);
        assert_eq!(ExecutionMode::from(ModeArg::Loop), ExecutionMode::Loop);
        assert_eq!(WorkspaceMount::from(MountArg::None), WorkspaceMount::None);
        assert_eq!(WorkspaceMount::from(MountArg::Ro), WorkspaceMount::ReadOnly);
        assert_eq!(
            WorkspaceMount::from(MountArg::Rw),
            WorkspaceMount::ReadWrite
        );
        assert_eq!(
            InferenceMode::from(InferenceModeArg::Single),
            InferenceMode::Single
        );
        assert_eq!(
            InferenceMode::from(InferenceModeArg::Router),
            InferenceMode::Router
        );
        assert_eq!(
            RoleKind::from(RoleKindArg::Orchestrator),
            RoleKind::Orchestrator
        );
        assert_eq!(
            RoleKind::from(RoleKindArg::Implementer),
            RoleKind::Implementer
        );
        assert_eq!(RoleKind::from(RoleKindArg::Validator), RoleKind::Validator);
        assert_eq!(HookEvent::from(HookEventArg::Start), HookEvent::StepStart);
        assert_eq!(HookEvent::from(HookEventArg::Pass), HookEvent::StepPass);
        assert_eq!(HookEvent::from(HookEventArg::Retry), HookEvent::StepRetry);
        assert_eq!(HookEvent::from(HookEventArg::Halt), HookEvent::Halt);
        assert_eq!(HookFailure::from(HookFailureArg::Halt), HookFailure::Halt);
        assert_eq!(HookFailure::from(HookFailureArg::Warn), HookFailure::Warn);
    }
}
