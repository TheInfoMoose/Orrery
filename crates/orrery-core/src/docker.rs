use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use crate::checkpoint::SubtaskAllocation;
use crate::config::{AgentBackend, AgentConfig, InferenceConfig, InferenceMode, WorkspaceMount};
use crate::mcp::scopes_overlap;
use crate::security::SecretValue;
use crate::{Error, Result};

#[derive(Debug)]
pub enum EnvValue {
    Plain(String),
    Secret(SecretValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountSpec {
    Bind {
        host: PathBuf,
        container: String,
        read_only: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerState {
    Missing,
    Created,
    Running,
    Exited { code: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: i64,
}

#[derive(Debug)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub cmd: Vec<String>,
    pub env: Vec<(String, EnvValue)>,
    pub mounts: Vec<MountSpec>,
    pub memory_limit_mb: u64,
    pub auto_remove: bool,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn remove(&self, name: &str) -> Result<()>;
    async fn inspect(&self, name: &str) -> Result<ContainerState>;
    async fn run_to_completion(
        &self,
        spec: &ContainerSpec,
        stdin: Vec<u8>,
        on_output: &(dyn Fn(String) + Send + Sync),
    ) -> Result<ExitStatus>;
}

#[async_trait]
pub trait ComposeRunner: Send + Sync {
    async fn up(&self, file: &Path, service: Option<&str>) -> Result<()>;
    async fn down(&self, file: &Path, service: Option<&str>) -> Result<()>;
    async fn is_running(&self, file: &Path, service: Option<&str>) -> Result<bool>;
}

#[async_trait]
pub trait InferenceControl: Send + Sync {
    async fn is_running(&self, inference: &InferenceConfig) -> Result<bool>;
    async fn start(&self, inference: &InferenceConfig) -> Result<()>;
    async fn stop(&self, inference: &InferenceConfig) -> Result<()>;
    async fn wait_ready(&self, inference: &InferenceConfig) -> Result<()>;
    async fn unload(&self, inference: &InferenceConfig, model: &str) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceCall {
    IsRunning(String),
    Start(String),
    Stop(String),
    WaitReady(String),
    Unload(String, String),
}

#[derive(Default)]
pub struct MockInferenceControl {
    running: Mutex<HashSet<String>>,
    calls: Mutex<Vec<InferenceCall>>,
}

impl MockInferenceControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_running(&self, inference_id: &str, running: bool) {
        let mut guard = match self.running.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if running {
            guard.insert(inference_id.to_string());
        } else {
            guard.remove(inference_id);
        }
    }

    pub fn calls(&self) -> Vec<InferenceCall> {
        match self.calls.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[async_trait]
impl InferenceControl for MockInferenceControl {
    async fn is_running(&self, inference: &InferenceConfig) -> Result<bool> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .push(InferenceCall::IsRunning(inference.inference_id.clone()));
        let guard = self.running.lock().map_err(|_| Error::Security {
            message: "MockInferenceControl lock poisoned".into(),
        })?;
        Ok(guard.contains(&inference.inference_id))
    }

    async fn start(&self, inference: &InferenceConfig) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .push(InferenceCall::Start(inference.inference_id.clone()));
        self.running
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .insert(inference.inference_id.clone());
        Ok(())
    }

    async fn stop(&self, inference: &InferenceConfig) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .push(InferenceCall::Stop(inference.inference_id.clone()));
        self.running
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .remove(&inference.inference_id);
        Ok(())
    }

    async fn wait_ready(&self, inference: &InferenceConfig) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .push(InferenceCall::WaitReady(inference.inference_id.clone()));
        Ok(())
    }

    async fn unload(&self, inference: &InferenceConfig, model: &str) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockInferenceControl lock poisoned".into(),
            })?
            .push(InferenceCall::Unload(
                inference.inference_id.clone(),
                model.to_string(),
            ));
        Ok(())
    }
}

pub fn unload_request(
    inference: &InferenceConfig,
    model: &str,
) -> Option<(String, serde_json::Value)> {
    match inference.mode {
        InferenceMode::Router => Some((
            format!(
                "{}/models/unload",
                inference.endpoint_url.trim_end_matches('/')
            ),
            serde_json::json!({ "model": model }),
        )),
        InferenceMode::Single => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockCall {
    Start(String),
    Stop(String),
    Remove(String),
    Inspect(String),
    RunToCompletion {
        name: String,
        stdin_len: usize,
        secret_env: Vec<String>,
        plain_env: Vec<String>,
    },
}

pub type MockRunScript = Box<dyn Fn(&ContainerSpec, &[u8]) -> (Vec<String>, i64) + Send + Sync>;

#[derive(Default)]
pub struct MockContainerBackend {
    calls: Mutex<Vec<MockCall>>,
    states: Mutex<HashMap<String, ContainerState>>,
    script: Mutex<Option<MockRunScript>>,
    delay: Mutex<Option<Duration>>,
}

impl MockContainerBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_container(&self, name: &str, state: ContainerState) {
        let mut guard = match self.states.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert(name.to_string(), state);
    }

    pub fn calls(&self) -> Vec<MockCall> {
        match self.calls.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn set_run_script(&self, script: MockRunScript) {
        let mut guard = match self.script.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(script);
    }

    pub fn set_run_delay(&self, delay: Duration) {
        let mut guard = match self.delay.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(delay);
    }
}

#[async_trait]
impl ContainerBackend for MockContainerBackend {
    async fn start(&self, name: &str) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .push(MockCall::Start(name.to_string()));

        let mut states = self.states.lock().map_err(|_| Error::Security {
            message: "MockContainerBackend lock poisoned".into(),
        })?;
        match states.get(name) {
            None | Some(ContainerState::Missing) => Err(Error::Validation {
                field: "container".to_string(),
                message: format!(
                    "container '{name}' not found; Orrery never creates inference containers"
                ),
            }),
            Some(_) => {
                states.insert(name.to_string(), ContainerState::Running);
                Ok(())
            }
        }
    }

    async fn stop(&self, name: &str) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .push(MockCall::Stop(name.to_string()));
        self.states
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .insert(name.to_string(), ContainerState::Exited { code: 0 });
        Ok(())
    }

    async fn remove(&self, name: &str) -> Result<()> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .push(MockCall::Remove(name.to_string()));
        self.states
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .insert(name.to_string(), ContainerState::Missing);
        Ok(())
    }

    async fn inspect(&self, name: &str) -> Result<ContainerState> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .push(MockCall::Inspect(name.to_string()));
        let states = self.states.lock().map_err(|_| Error::Security {
            message: "MockContainerBackend lock poisoned".into(),
        })?;
        Ok(states.get(name).copied().unwrap_or(ContainerState::Missing))
    }

    async fn run_to_completion(
        &self,
        spec: &ContainerSpec,
        stdin: Vec<u8>,
        on_output: &(dyn Fn(String) + Send + Sync),
    ) -> Result<ExitStatus> {
        let delay = *self.delay.lock().map_err(|_| Error::Security {
            message: "MockContainerBackend lock poisoned".into(),
        })?;
        if let Some(d) = delay
            && !d.is_zero()
        {
            tokio::time::sleep(d).await;
        }

        let (lines, code) = {
            let script_guard = self.script.lock().map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?;
            match script_guard.as_ref() {
                Some(script) => script(spec, &stdin),
                None => (Vec::new(), 0),
            }
        };

        for line in lines {
            on_output(line);
        }

        let mut secret_env = Vec::new();
        let mut plain_env = Vec::new();
        for (k, v) in &spec.env {
            match v {
                EnvValue::Secret(_) => secret_env.push(k.clone()),
                EnvValue::Plain(val) => plain_env.push(format!("{k}={val}")),
            }
        }

        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .push(MockCall::RunToCompletion {
                name: spec.name.clone(),
                stdin_len: stdin.len(),
                secret_env,
                plain_env,
            });

        self.states
            .lock()
            .map_err(|_| Error::Security {
                message: "MockContainerBackend lock poisoned".into(),
            })?
            .insert(
                spec.name.clone(),
                if spec.auto_remove {
                    ContainerState::Missing
                } else {
                    ContainerState::Exited { code }
                },
            );

        Ok(ExitStatus { code })
    }
}

pub fn cli_container_name(run_id: &uuid::Uuid, agent_id: &str) -> String {
    let simple = run_id.simple().to_string();
    let prefix: String = simple.chars().take(8).collect();
    format!("orrery-cli-{prefix}-{agent_id}")
}

pub fn agent_io_dir(project_root: &Path, run_id: &uuid::Uuid, agent_id: &str) -> PathBuf {
    project_root
        .join(".orrery")
        .join("agent_io")
        .join(run_id.to_string())
        .join(agent_id)
}

pub fn effective_concurrency(
    max_concurrent_agents: usize,
    writer: bool,
    agents: &[AgentConfig],
    allocations: &[SubtaskAllocation],
) -> usize {
    let cap = 1usize.max(max_concurrent_agents.min(agents.len()));
    if !writer {
        return cap;
    }

    if allocations.is_empty() || allocations.iter().any(|alloc| alloc.write_scope.is_empty()) {
        return 1;
    }

    for (i, alloc_a) in allocations.iter().enumerate() {
        for alloc_b in &allocations[i + 1..] {
            for scope_a in &alloc_a.write_scope {
                for scope_b in &alloc_b.write_scope {
                    if scopes_overlap(scope_a, scope_b) {
                        return 1;
                    }
                }
            }
        }
    }

    if agents.iter().any(|agent| {
        matches!(
            agent.backend,
            AgentBackend::AgentCli {
                workspace_mount: WorkspaceMount::ReadWrite,
                ..
            }
        )
    }) {
        return 1;
    }

    cap
}

pub fn mounts_for(agent: &AgentConfig, project_root: &Path, agent_io_dir: &Path) -> Vec<MountSpec> {
    let AgentBackend::AgentCli {
        workspace_mount, ..
    } = &agent.backend
    else {
        return Vec::new();
    };

    let mut mounts = Vec::new();
    match workspace_mount {
        WorkspaceMount::ReadOnly => {
            mounts.push(MountSpec::Bind {
                host: project_root.to_path_buf(),
                container: "/workspace".to_string(),
                read_only: true,
            });
        }
        WorkspaceMount::ReadWrite => {
            mounts.push(MountSpec::Bind {
                host: project_root.to_path_buf(),
                container: "/workspace".to_string(),
                read_only: false,
            });
        }
        WorkspaceMount::None => {}
    }

    mounts.push(MountSpec::Bind {
        host: agent_io_dir.join("in"),
        container: "/orrery/in".to_string(),
        read_only: true,
    });
    mounts.push(MountSpec::Bind {
        host: agent_io_dir.join("out"),
        container: "/orrery/out".to_string(),
        read_only: false,
    });

    mounts
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::config::InferenceLifecycle;

    fn local_agent(id: &str) -> AgentConfig {
        AgentConfig {
            agent_id: id.to_string(),
            display_name: id.to_string(),
            backend: AgentBackend::LocalModel {
                inference_id: "coder-14b".to_string(),
                model: "qwen2.5-coder-14b-q4_k_m.gguf".to_string(),
                temperature: 0.1,
                max_context_tokens: 16384,
            },
        }
    }

    fn cli_agent(id: &str, mount: WorkspaceMount) -> AgentConfig {
        AgentConfig {
            agent_id: id.to_string(),
            display_name: id.to_string(),
            backend: AgentBackend::AgentCli {
                image: "ghcr.io/example/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                command: vec!["cli-agent".to_string()],
                workspace_mount: mount,
                memory_limit_mb: 1024,
                timeout_secs: 60,
                keychain_service: None,
                keychain_account: None,
                secret_env: None,
            },
        }
    }

    fn cloud_agent(id: &str) -> AgentConfig {
        AgentConfig {
            agent_id: id.to_string(),
            display_name: id.to_string(),
            backend: AgentBackend::CloudApi {
                endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                model: "test-model".to_string(),
                temperature: 0.2,
                max_context_tokens: 32768,
                keychain_service: "orrery.test".to_string(),
                keychain_account: "testuser".to_string(),
            },
        }
    }

    fn alloc(agent_id: &str, scopes: &[&str]) -> SubtaskAllocation {
        SubtaskAllocation {
            target_agent_id: agent_id.to_string(),
            instructions: "do work".to_string(),
            write_scope: scopes.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn concurrency_non_writer_uses_cap() {
        let two_agents = vec![local_agent("a1"), local_agent("a2")];
        assert_eq!(effective_concurrency(4, false, &two_agents, &[]), 2);

        let five_agents = vec![
            local_agent("a1"),
            local_agent("a2"),
            local_agent("a3"),
            local_agent("a4"),
            local_agent("a5"),
        ];
        assert_eq!(effective_concurrency(2, false, &five_agents, &[]), 2);

        assert_eq!(effective_concurrency(4, false, &[], &[]), 1);
    }

    #[test]
    fn concurrency_writer_without_scopes_is_one() {
        let agents = vec![local_agent("a1"), local_agent("a2")];
        assert_eq!(effective_concurrency(4, true, &agents, &[]), 1);

        let partial_allocs = vec![alloc("a1", &["src/a"]), alloc("a2", &[])];
        assert_eq!(effective_concurrency(4, true, &agents, &partial_allocs), 1);
    }

    #[test]
    fn concurrency_writer_disjoint_scopes() {
        let agents = vec![local_agent("a1"), local_agent("a2")];
        let allocs = vec![alloc("a1", &["src/a"]), alloc("a2", &["src/b"])];
        assert_eq!(effective_concurrency(4, true, &agents, &allocs), 2);
    }

    #[test]
    fn concurrency_writer_overlapping_scopes_is_one() {
        let agents = vec![local_agent("a1"), local_agent("a2")];
        let allocs = vec![alloc("a1", &["src"]), alloc("a2", &["src/lib.rs"])];
        assert_eq!(effective_concurrency(4, true, &agents, &allocs), 1);
    }

    #[test]
    fn concurrency_writer_with_rw_cli_is_one() {
        let agents = vec![
            local_agent("a1"),
            cli_agent("a2", WorkspaceMount::ReadWrite),
        ];
        let allocs = vec![alloc("a1", &["src/a"]), alloc("a2", &["src/b"])];
        assert_eq!(effective_concurrency(4, true, &agents, &allocs), 1);
    }

    #[test]
    fn mounts_local_and_cloud_empty() {
        let root = Path::new("project");
        let io_dir = Path::new("project/.orrery/agent_io/run-1/a1");
        assert!(mounts_for(&local_agent("a1"), root, io_dir).is_empty());
        assert!(mounts_for(&cloud_agent("a2"), root, io_dir).is_empty());
    }

    #[test]
    fn mounts_cli_readonly() {
        let root = Path::new("project");
        let io_dir = Path::new("project/.orrery/agent_io/run-1/cli1");
        let mounts = mounts_for(&cli_agent("cli1", WorkspaceMount::ReadOnly), root, io_dir);
        assert_eq!(
            mounts,
            vec![
                MountSpec::Bind {
                    host: PathBuf::from("project"),
                    container: "/workspace".to_string(),
                    read_only: true,
                },
                MountSpec::Bind {
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/in"),
                    container: "/orrery/in".to_string(),
                    read_only: true,
                },
                MountSpec::Bind {
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/out"),
                    container: "/orrery/out".to_string(),
                    read_only: false,
                },
            ]
        );
    }

    #[test]
    fn mounts_cli_readwrite() {
        let root = Path::new("project");
        let io_dir = Path::new("project/.orrery/agent_io/run-1/cli1");
        let mounts = mounts_for(&cli_agent("cli1", WorkspaceMount::ReadWrite), root, io_dir);
        assert_eq!(
            mounts,
            vec![
                MountSpec::Bind {
                    host: PathBuf::from("project"),
                    container: "/workspace".to_string(),
                    read_only: false,
                },
                MountSpec::Bind {
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/in"),
                    container: "/orrery/in".to_string(),
                    read_only: true,
                },
                MountSpec::Bind {
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/out"),
                    container: "/orrery/out".to_string(),
                    read_only: false,
                },
            ]
        );
    }

    #[test]
    fn mounts_cli_none_has_no_project_mount() {
        let root = Path::new("project");
        let io_dir = Path::new("project/.orrery/agent_io/run-1/cli1");
        let mounts = mounts_for(&cli_agent("cli1", WorkspaceMount::None), root, io_dir);
        assert_eq!(
            mounts,
            vec![
                MountSpec::Bind {
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/in"),
                    container: "/orrery/in".to_string(),
                    read_only: true,
                },
                MountSpec::Bind {
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/out"),
                    container: "/orrery/out".to_string(),
                    read_only: false,
                },
            ]
        );
    }

    #[test]
    fn mounts_never_bind_orrery_dir() {
        let root = Path::new("project");
        let orrery_dir = root.join(".orrery");
        let run_id = uuid::Uuid::nil();
        let io_dir = agent_io_dir(root, &run_id, "cli1");
        assert_eq!(
            io_dir,
            PathBuf::from("project/.orrery/agent_io/00000000-0000-0000-0000-000000000000/cli1")
        );

        for mode in [
            WorkspaceMount::None,
            WorkspaceMount::ReadOnly,
            WorkspaceMount::ReadWrite,
        ] {
            let mounts = mounts_for(&cli_agent("cli1", mode), root, &io_dir);
            for mount in mounts {
                let MountSpec::Bind { host, .. } = mount;
                assert_ne!(host, orrery_dir);
            }
        }
    }

    #[test]
    fn unload_request_router_only() {
        let router_inf = InferenceConfig {
            inference_id: "router".to_string(),
            endpoint_url: "http://127.0.0.1:8081".to_string(),
            mode: InferenceMode::Router,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        let m = "qwen2.5-coder-14b";
        assert_eq!(
            unload_request(&router_inf, m),
            Some((
                "http://127.0.0.1:8081/models/unload".to_string(),
                serde_json::json!({ "model": m }),
            ))
        );

        let single_inf = InferenceConfig {
            inference_id: "single".to_string(),
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        assert_eq!(unload_request(&single_inf, m), None);
    }

    #[test]
    fn cli_container_name_format() {
        let run_id = uuid::Uuid::parse_str("a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d").unwrap();
        assert_eq!(
            cli_container_name(&run_id, "coder-cli"),
            "orrery-cli-a1b2c3d4-coder-cli"
        );
    }

    #[tokio::test]
    async fn mock_start_unknown_container_fails() {
        let backend = MockContainerBackend::new();
        assert_eq!(
            backend.inspect("missing-box").await.unwrap(),
            ContainerState::Missing
        );
        let err = backend.start("missing-box").await.unwrap_err();
        match err {
            Error::Validation { field, message } => {
                assert_eq!(field, "container");
                assert_eq!(
                    message,
                    "container 'missing-box' not found; Orrery never creates inference containers"
                );
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn mock_records_calls_and_runs_script() {
        let backend = MockContainerBackend::new();
        backend.add_container("llama-box", ContainerState::Exited { code: 0 });
        backend.start("llama-box").await.unwrap();
        assert_eq!(
            backend.inspect("llama-box").await.unwrap(),
            ContainerState::Running
        );
        backend.stop("llama-box").await.unwrap();
        assert_eq!(
            backend.inspect("llama-box").await.unwrap(),
            ContainerState::Exited { code: 0 }
        );
        backend.remove("llama-box").await.unwrap();
        assert_eq!(
            backend.inspect("llama-box").await.unwrap(),
            ContainerState::Missing
        );

        backend.set_run_delay(Duration::from_millis(5));
        backend.set_run_script(Box::new(|spec, stdin| {
            let text = String::from_utf8_lossy(stdin);
            (
                vec![format!("ran {}", spec.name), format!("stdin={text}")],
                0,
            )
        }));

        let spec = ContainerSpec {
            name: "orrery-cli-12345678-agent".to_string(),
            image: "ghcr.io/example/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            cmd: vec!["run".to_string()],
            env: vec![
                ("MODE".to_string(), EnvValue::Plain("test".to_string())),
                (
                    "API_KEY".to_string(),
                    EnvValue::Secret(SecretValue::new("super-secret-token")),
                ),
            ],
            mounts: Vec::new(),
            memory_limit_mb: 512,
            auto_remove: false,
        };

        let collected = Arc::new(Mutex::new(Vec::new()));
        let collected_cb = Arc::clone(&collected);
        let status = backend
            .run_to_completion(&spec, b"prompt-input".to_vec(), &move |line| {
                collected_cb.lock().unwrap().push(line);
            })
            .await
            .unwrap();
        assert_eq!(status, ExitStatus { code: 0 });
        assert_eq!(
            *collected.lock().unwrap(),
            vec![
                "ran orrery-cli-12345678-agent".to_string(),
                "stdin=prompt-input".to_string(),
            ]
        );
        assert_eq!(
            backend.calls(),
            vec![
                MockCall::Start("llama-box".to_string()),
                MockCall::Inspect("llama-box".to_string()),
                MockCall::Stop("llama-box".to_string()),
                MockCall::Inspect("llama-box".to_string()),
                MockCall::Remove("llama-box".to_string()),
                MockCall::Inspect("llama-box".to_string()),
                MockCall::RunToCompletion {
                    name: "orrery-cli-12345678-agent".to_string(),
                    stdin_len: 12,
                    secret_env: vec!["API_KEY".to_string()],
                    plain_env: vec!["MODE=test".to_string()],
                },
            ]
        );

        let inf_ctrl = MockInferenceControl::new();
        let inf = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            mode: InferenceMode::Router,
            lifecycle: InferenceLifecycle::Container {
                name: "llama-box".to_string(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        assert!(!inf_ctrl.is_running(&inf).await.unwrap());
        inf_ctrl.start(&inf).await.unwrap();
        assert!(inf_ctrl.is_running(&inf).await.unwrap());
        inf_ctrl.wait_ready(&inf).await.unwrap();
        inf_ctrl.unload(&inf, "m1").await.unwrap();
        inf_ctrl.stop(&inf).await.unwrap();
        assert!(!inf_ctrl.is_running(&inf).await.unwrap());
        inf_ctrl.set_running("coder-14b", true);
        assert!(inf_ctrl.is_running(&inf).await.unwrap());
        inf_ctrl.set_running("coder-14b", false);
        assert_eq!(
            inf_ctrl.calls(),
            vec![
                InferenceCall::IsRunning("coder-14b".to_string()),
                InferenceCall::Start("coder-14b".to_string()),
                InferenceCall::IsRunning("coder-14b".to_string()),
                InferenceCall::WaitReady("coder-14b".to_string()),
                InferenceCall::Unload("coder-14b".to_string(), "m1".to_string()),
                InferenceCall::Stop("coder-14b".to_string()),
                InferenceCall::IsRunning("coder-14b".to_string()),
                InferenceCall::IsRunning("coder-14b".to_string()),
            ]
        );
    }
}
