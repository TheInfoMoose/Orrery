use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bollard::models::{ContainerCreateBody, ContainerStateStatusEnum, HostConfig};
use bollard::query_parameters::{
    AttachContainerOptionsBuilder, CreateContainerOptionsBuilder, InspectContainerOptionsBuilder,
    RemoveContainerOptionsBuilder, StartContainerOptionsBuilder, StopContainerOptionsBuilder,
    WaitContainerOptionsBuilder,
};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::checkpoint::SubtaskAllocation;
use crate::config::{
    AgentBackend, AgentConfig, InferenceConfig, InferenceLifecycle, InferenceMode, WorkspaceMount,
};
use crate::mcp::scopes_overlap;
use crate::security::{SecretRedactor, SecretValue};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposeOp {
    Up,
    Down,
    Ps,
}

fn compose_args(file: &Path, service: Option<&str>, op: ComposeOp) -> Vec<String> {
    let mut args = vec![
        "compose".to_string(),
        "-f".to_string(),
        file.to_string_lossy().into_owned(),
    ];
    match op {
        ComposeOp::Up => {
            args.push("up".to_string());
            args.push("-d".to_string());
            if let Some(svc) = service {
                args.push(svc.to_string());
            }
        }
        ComposeOp::Down => match service {
            Some(svc) => {
                args.push("stop".to_string());
                args.push(svc.to_string());
            }
            None => {
                args.push("down".to_string());
            }
        },
        ComposeOp::Ps => {
            args.push("ps".to_string());
            args.push("--status".to_string());
            args.push("running".to_string());
            args.push("-q".to_string());
            if let Some(svc) = service {
                args.push(svc.to_string());
            }
        }
    }
    args
}

fn ensure_absolute_compose_file(file: &Path) -> Result<()> {
    if !file.is_absolute() {
        return Err(Error::Validation {
            field: "compose".to_string(),
            message: "compose file must be resolved by ConfigStore::load_inference".to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DockerComposeCli;

impl DockerComposeCli {
    async fn run_compose(file: &Path, args: &[String]) -> Result<std::process::Output> {
        let output = tokio::process::Command::new("docker")
            .args(args)
            .output()
            .await
            .map_err(|source| Error::Io {
                path: file.to_path_buf(),
                source,
            })?;
        if !output.status.success() {
            let raw_stderr = String::from_utf8_lossy(&output.stderr);
            let redacted = SecretRedactor::new().redact(raw_stderr.trim())?;
            return Err(Error::Validation {
                field: "compose".to_string(),
                message: redacted,
            });
        }
        Ok(output)
    }
}

#[async_trait]
impl ComposeRunner for DockerComposeCli {
    async fn up(&self, file: &Path, service: Option<&str>) -> Result<()> {
        ensure_absolute_compose_file(file)?;
        let args = compose_args(file, service, ComposeOp::Up);
        Self::run_compose(file, &args).await?;
        Ok(())
    }

    async fn down(&self, file: &Path, service: Option<&str>) -> Result<()> {
        ensure_absolute_compose_file(file)?;
        let args = compose_args(file, service, ComposeOp::Down);
        Self::run_compose(file, &args).await?;
        Ok(())
    }

    async fn is_running(&self, file: &Path, service: Option<&str>) -> Result<bool> {
        ensure_absolute_compose_file(file)?;
        let args = compose_args(file, service, ComposeOp::Ps);
        let output = Self::run_compose(file, &args).await?;
        Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
    }
}

#[async_trait]
pub trait InferenceControl: Send + Sync {
    async fn is_running(&self, inference: &InferenceConfig) -> Result<bool>;
    async fn start(&self, inference: &InferenceConfig) -> Result<()>;
    async fn stop(&self, inference: &InferenceConfig) -> Result<()>;
    async fn wait_ready(&self, inference: &InferenceConfig) -> Result<()>;
    async fn unload(&self, inference: &InferenceConfig, model: &str) -> Result<()>;
}

pub struct DockerInferenceControl {
    pub containers: Arc<dyn ContainerBackend>,
    pub compose: Arc<dyn ComposeRunner>,
    client: reqwest::Client,
}

impl DockerInferenceControl {
    pub fn new(containers: Arc<dyn ContainerBackend>, compose: Arc<dyn ComposeRunner>) -> Self {
        Self {
            containers,
            compose,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl InferenceControl for DockerInferenceControl {
    async fn is_running(&self, inference: &InferenceConfig) -> Result<bool> {
        match &inference.lifecycle {
            InferenceLifecycle::External => Ok(true),
            InferenceLifecycle::Container { name } => {
                Ok(self.containers.inspect(name).await? == ContainerState::Running)
            }
            InferenceLifecycle::Compose { file, service } => {
                self.compose.is_running(file, service.as_deref()).await
            }
        }
    }

    async fn start(&self, inference: &InferenceConfig) -> Result<()> {
        match &inference.lifecycle {
            InferenceLifecycle::External => Ok(()),
            InferenceLifecycle::Container { name } => self.containers.start(name).await,
            InferenceLifecycle::Compose { file, service } => {
                self.compose.up(file, service.as_deref()).await
            }
        }
    }

    async fn stop(&self, inference: &InferenceConfig) -> Result<()> {
        match &inference.lifecycle {
            InferenceLifecycle::External => Ok(()),
            InferenceLifecycle::Container { name } => self.containers.stop(name).await,
            InferenceLifecycle::Compose { file, service } => {
                self.compose.down(file, service.as_deref()).await
            }
        }
    }

    async fn wait_ready(&self, inference: &InferenceConfig) -> Result<()> {
        let health_url = format!("{}/health", inference.endpoint_url.trim_end_matches('/'));
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(inference.startup_timeout_secs);
        loop {
            if let Ok(resp) = self
                .client
                .get(&health_url)
                .timeout(Duration::from_secs(2))
                .send()
                .await
                && resp.status() == reqwest::StatusCode::OK
            {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Agent {
                    agent_id: inference.inference_id.clone(),
                    message: "not ready".to_string(),
                });
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn unload(&self, inference: &InferenceConfig, model: &str) -> Result<()> {
        if let Some((url, body)) = unload_request(inference, model) {
            self.client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(Error::Http)?
                .error_for_status()
                .map_err(Error::Http)?;
        }
        Ok(())
    }
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
            mounts.push(MountSpec::Bind {
                host: agent_io_dir.join("mask"),
                container: "/workspace/.orrery".to_string(),
                read_only: true,
            });
        }
        WorkspaceMount::ReadWrite => {
            mounts.push(MountSpec::Bind {
                host: project_root.to_path_buf(),
                container: "/workspace".to_string(),
                read_only: false,
            });
            mounts.push(MountSpec::Bind {
                host: agent_io_dir.join("mask"),
                container: "/workspace/.orrery".to_string(),
                read_only: true,
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

struct LeaseOpenGuard {
    open: Arc<AtomicBool>,
    armed: bool,
}

impl LeaseOpenGuard {
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for LeaseOpenGuard {
    fn drop(&mut self) {
        if self.armed {
            self.open.store(false, Ordering::SeqCst);
        }
    }
}

pub struct ResourceManager {
    backend: Arc<dyn ContainerBackend>,
    inference_control: Arc<dyn InferenceControl>,
    inference: Vec<InferenceConfig>,
    max_concurrent_agents: usize,
    project_root: PathBuf,
    lease_open: Arc<AtomicBool>,
    started: Mutex<HashSet<String>>,
    held: Mutex<Vec<(String, String)>>,
}

impl ResourceManager {
    pub fn new(
        backend: Arc<dyn ContainerBackend>,
        inference_control: Arc<dyn InferenceControl>,
        inference: Vec<InferenceConfig>,
        max_concurrent_agents: usize,
        project_root: PathBuf,
    ) -> Self {
        Self {
            backend,
            inference_control,
            inference,
            max_concurrent_agents,
            project_root,
            lease_open: Arc::new(AtomicBool::new(false)),
            started: Mutex::new(HashSet::new()),
            held: Mutex::new(Vec::new()),
        }
    }

    pub async fn begin_stage(
        &self,
        run_id: uuid::Uuid,
        agents: &[AgentConfig],
        writer: bool,
        allocations: &[SubtaskAllocation],
    ) -> Result<StageLease> {
        if self.lease_open.swap(true, Ordering::SeqCst) {
            return Err(Error::Engine {
                step_number: 0,
                message: "stage lease already open".to_string(),
            });
        }
        let open_guard = LeaseOpenGuard {
            open: Arc::clone(&self.lease_open),
            armed: true,
        };

        let mut needed: Vec<(String, String)> = Vec::new();
        let mut needed_servers: Vec<String> = Vec::new();
        let mut cli_agents: Vec<String> = Vec::new();

        for agent in agents {
            match &agent.backend {
                AgentBackend::LocalModel {
                    inference_id,
                    model,
                    ..
                } => {
                    if !self
                        .inference
                        .iter()
                        .any(|inf| &inf.inference_id == inference_id)
                    {
                        return Err(Error::Validation {
                            field: "inference_id".to_string(),
                            message: format!("unknown inference_id '{inference_id}'"),
                        });
                    }
                    let pair = (inference_id.clone(), model.clone());
                    if !needed.contains(&pair) {
                        needed.push(pair);
                    }
                    if !needed_servers.contains(inference_id) {
                        needed_servers.push(inference_id.clone());
                    }
                }
                AgentBackend::AgentCli { .. } => {
                    if !cli_agents.contains(&agent.agent_id) {
                        cli_agents.push(agent.agent_id.clone());
                    }
                }
                AgentBackend::CloudApi { .. } => {}
            }
        }

        let prev_held = self
            .held
            .lock()
            .map_err(|_| Error::Security {
                message: "ResourceManager held lock poisoned".to_string(),
            })?
            .clone();

        for (held_inf_id, held_model) in &prev_held {
            let Some(inf) = self
                .inference
                .iter()
                .find(|i| &i.inference_id == held_inf_id)
            else {
                continue;
            };
            if !inf.unload_after_stage {
                continue;
            }
            match inf.mode {
                InferenceMode::Router => {
                    let still_needed = needed
                        .iter()
                        .any(|(id, m)| id == held_inf_id && m == held_model);
                    if !still_needed {
                        self.inference_control.unload(inf, held_model).await?;
                        self.held
                            .lock()
                            .map_err(|_| Error::Security {
                                message: "ResourceManager held lock poisoned".to_string(),
                            })?
                            .retain(|(id, m)| !(id == held_inf_id && m == held_model));
                    }
                }
                InferenceMode::Single => {
                    let server_needed = needed_servers.iter().any(|id| id == held_inf_id);
                    if !server_needed {
                        let was_started = self
                            .started
                            .lock()
                            .map_err(|_| Error::Security {
                                message: "ResourceManager started lock poisoned".to_string(),
                            })?
                            .remove(held_inf_id);
                        if was_started {
                            self.inference_control.stop(inf).await?;
                        }
                        self.held
                            .lock()
                            .map_err(|_| Error::Security {
                                message: "ResourceManager held lock poisoned".to_string(),
                            })?
                            .retain(|(id, _)| id != held_inf_id);
                    }
                }
            }
        }

        for server_id in &needed_servers {
            let Some(inf) = self.inference.iter().find(|i| &i.inference_id == server_id) else {
                continue;
            };
            if !self.inference_control.is_running(inf).await? {
                self.inference_control.start(inf).await?;
                self.started
                    .lock()
                    .map_err(|_| Error::Security {
                        message: "ResourceManager started lock poisoned".to_string(),
                    })?
                    .insert(inf.inference_id.clone());
            }
            self.inference_control.wait_ready(inf).await?;
        }

        *self.held.lock().map_err(|_| Error::Security {
            message: "ResourceManager held lock poisoned".to_string(),
        })? = needed;

        let concurrency =
            effective_concurrency(self.max_concurrent_agents, writer, agents, allocations);
        let lease = StageLease {
            run_id,
            semaphore: Arc::new(Semaphore::new(concurrency)),
            concurrency,
            cli_agents,
            open: Arc::clone(&self.lease_open),
        };
        open_guard.disarm();
        Ok(lease)
    }

    pub async fn end_stage(&self, lease: StageLease) -> Result<()> {
        for agent_id in &lease.cli_agents {
            let name = cli_container_name(&lease.run_id, agent_id);
            // Backends return Ok for a missing container; any other failure
            // may leave a container holding secret env, so it propagates.
            self.backend.remove(&name).await?;
        }

        let run_io_dir = self
            .project_root
            .join(".orrery")
            .join("agent_io")
            .join(lease.run_id.to_string());
        if run_io_dir.exists() {
            fs::remove_dir_all(&run_io_dir).map_err(|source| Error::Io {
                path: run_io_dir,
                source,
            })?;
        }

        lease.open.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        let held = {
            let mut guard = self.held.lock().map_err(|_| Error::Security {
                message: "ResourceManager held lock poisoned".to_string(),
            })?;
            std::mem::take(&mut *guard)
        };

        for (inf_id, model) in &held {
            if let Some(inf) = self.inference.iter().find(|i| &i.inference_id == inf_id)
                && inf.mode == InferenceMode::Router
                && inf.unload_after_stage
            {
                self.inference_control.unload(inf, model).await?;
            }
        }

        let started = {
            let mut guard = self.started.lock().map_err(|_| Error::Security {
                message: "ResourceManager started lock poisoned".to_string(),
            })?;
            std::mem::take(&mut *guard)
        };

        for inf in &self.inference {
            if started.contains(&inf.inference_id) {
                self.inference_control.stop(inf).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct StageLease {
    run_id: uuid::Uuid,
    semaphore: Arc<Semaphore>,
    concurrency: usize,
    cli_agents: Vec<String>,
    open: Arc<AtomicBool>,
}

impl StageLease {
    pub async fn acquire_slot(&self) -> Result<SlotPermit> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| Error::Engine {
                step_number: 0,
                message: "stage semaphore closed".to_string(),
            })?;
        Ok(SlotPermit { _permit: permit })
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency
    }
}

impl Drop for StageLease {
    fn drop(&mut self) {
        self.open.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub struct SlotPermit {
    _permit: OwnedSemaphorePermit,
}

fn is_not_found(err: &bollard::errors::Error) -> bool {
    matches!(
        err,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

fn map_inspect_state(state: Option<&bollard::models::ContainerState>) -> ContainerState {
    if state.and_then(|s| s.running) == Some(true) {
        ContainerState::Running
    } else if state.and_then(|s| s.status) == Some(ContainerStateStatusEnum::CREATED) {
        ContainerState::Created
    } else {
        ContainerState::Exited {
            code: state.and_then(|s| s.exit_code).unwrap_or(0),
        }
    }
}

pub struct BollardContainerBackend {
    docker: bollard::Docker,
}

impl BollardContainerBackend {
    pub fn connect() -> Result<Self> {
        let docker = bollard::Docker::connect_with_local_defaults().map_err(Error::Docker)?;
        Ok(Self { docker })
    }
}

pub fn create_body(spec: &ContainerSpec) -> ContainerCreateBody {
    let env: Vec<String> = spec
        .env
        .iter()
        .map(|(k, v)| match v {
            EnvValue::Plain(val) => format!("{k}={val}"),
            EnvValue::Secret(sec) => format!("{k}={}", sec.expose()),
        })
        .collect();

    let binds: Vec<String> = spec
        .mounts
        .iter()
        .map(
            |MountSpec::Bind {
                 host,
                 container,
                 read_only,
             }| {
                let host_str = host.to_string_lossy();
                if *read_only {
                    format!("{host_str}:{container}:ro")
                } else {
                    format!("{host_str}:{container}")
                }
            },
        )
        .collect();

    let memory_bytes = i64::try_from(spec.memory_limit_mb)
        .unwrap_or(i64::MAX)
        .saturating_mul(1024 * 1024);

    ContainerCreateBody {
        image: Some(spec.image.clone()),
        cmd: Some(spec.cmd.clone()),
        env: Some(env),
        open_stdin: Some(true),
        attach_stdin: Some(true),
        stdin_once: Some(true),
        host_config: Some(HostConfig {
            binds: Some(binds),
            memory: Some(memory_bytes),
            auto_remove: Some(spec.auto_remove),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[async_trait]
impl ContainerBackend for BollardContainerBackend {
    async fn start(&self, name: &str) -> Result<()> {
        match self.inspect(name).await? {
            ContainerState::Missing => Err(Error::Validation {
                field: "container".to_string(),
                message: format!(
                    "container '{name}' not found; Orrery never creates inference containers"
                ),
            }),
            ContainerState::Running => Ok(()),
            ContainerState::Created | ContainerState::Exited { .. } => {
                let options = StartContainerOptionsBuilder::default().build();
                self.docker
                    .start_container(name, Some(options))
                    .await
                    .map_err(Error::Docker)
            }
        }
    }

    async fn stop(&self, name: &str) -> Result<()> {
        let options = StopContainerOptionsBuilder::default().build();
        match self.docker.stop_container(name, Some(options)).await {
            Ok(()) => Ok(()),
            Err(err) if is_not_found(&err) => Ok(()),
            Err(err) => Err(Error::Docker(err)),
        }
    }

    async fn remove(&self, name: &str) -> Result<()> {
        let options = RemoveContainerOptionsBuilder::default().force(true).build();
        match self.docker.remove_container(name, Some(options)).await {
            Ok(()) => Ok(()),
            Err(err) if is_not_found(&err) => Ok(()),
            Err(err) => Err(Error::Docker(err)),
        }
    }

    async fn inspect(&self, name: &str) -> Result<ContainerState> {
        let options = InspectContainerOptionsBuilder::default().build();
        match self.docker.inspect_container(name, Some(options)).await {
            Ok(info) => Ok(map_inspect_state(info.state.as_ref())),
            Err(err) if is_not_found(&err) => Ok(ContainerState::Missing),
            Err(err) => Err(Error::Docker(err)),
        }
    }

    async fn run_to_completion(
        &self,
        spec: &ContainerSpec,
        stdin: Vec<u8>,
        on_output: &(dyn Fn(String) + Send + Sync),
    ) -> Result<ExitStatus> {
        let create_opts = CreateContainerOptionsBuilder::default()
            .name(&spec.name)
            .build();
        self.docker
            .create_container(Some(create_opts), create_body(spec))
            .await
            .map_err(Error::Docker)?;

        let attach_opts = AttachContainerOptionsBuilder::default()
            .stdin(true)
            .stdout(true)
            .stderr(true)
            .stream(true)
            .build();
        let attached = self
            .docker
            .attach_container(&spec.name, Some(attach_opts))
            .await
            .map_err(Error::Docker)?;

        let start_opts = StartContainerOptionsBuilder::default().build();
        self.docker
            .start_container(&spec.name, Some(start_opts))
            .await
            .map_err(Error::Docker)?;

        // Write stdin while draining output: writing everything first can
        // deadlock once the container's output fills the attach stream.
        let mut input = attached.input;
        let mut output = attached.output;
        let writer = async move {
            input.write_all(&stdin).await?;
            input.shutdown().await
        };
        let reader = async {
            while let Some(item) = output.next().await {
                let chunk = item.map_err(Error::Docker)?;
                let text = String::from_utf8_lossy(chunk.as_ref());
                for line in text.lines() {
                    on_output(line.to_string());
                }
            }
            Ok::<(), Error>(())
        };
        let (write_res, read_res) = tokio::join!(writer, reader);
        read_res?;
        match write_res {
            Ok(()) => {}
            // The container closed stdin early (e.g. it reads /orrery/in/prompt.md
            // instead); its exit code below decides success.
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) => {}
            Err(err) => return Err(Error::Docker(bollard::errors::Error::from(err))),
        }

        let wait_opts = WaitContainerOptionsBuilder::default().build();
        let mut wait_stream = self.docker.wait_container(&spec.name, Some(wait_opts));
        let code = match wait_stream.next().await {
            Some(Ok(resp)) => resp.status_code,
            Some(Err(bollard::errors::Error::DockerContainerWaitError { code, error }))
                if error.is_empty() =>
            {
                code
            }
            Some(Err(err)) => return Err(Error::Docker(err)),
            None => {
                return Err(Error::Validation {
                    field: "container".to_string(),
                    message: format!("container '{}' ended without an exit status", spec.name),
                });
            }
        };
        Ok(ExitStatus { code })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::config::InferenceLifecycle;

    fn local_agent(id: &str) -> AgentConfig {
        local_agent_on(id, "coder-14b", "Qwen2.5-Coder-14B-Instruct-Q4_K_M")
    }

    fn local_agent_on(id: &str, inference_id: &str, model: &str) -> AgentConfig {
        AgentConfig {
            agent_id: id.to_string(),
            display_name: id.to_string(),
            backend: AgentBackend::LocalModel {
                inference_id: inference_id.to_string(),
                model: model.to_string(),
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

    fn spec_coder_14b() -> InferenceConfig {
        InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: "llama-coder-14b".to_string(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        }
    }

    fn spec_router(compose_file: PathBuf) -> InferenceConfig {
        InferenceConfig {
            inference_id: "router".to_string(),
            endpoint_url: "http://127.0.0.1:8081".to_string(),
            mode: InferenceMode::Router,
            lifecycle: InferenceLifecycle::Compose {
                file: compose_file,
                service: Some("llama".to_string()),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        }
    }

    fn spec_always_on(endpoint_url: &str) -> InferenceConfig {
        InferenceConfig {
            inference_id: "always-on".to_string(),
            endpoint_url: endpoint_url.to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 300,
        }
    }

    #[derive(Default)]
    struct RecordingComposeRunner {
        calls: Mutex<Vec<(String, PathBuf, Option<String>)>>,
        running: Mutex<bool>,
    }

    impl RecordingComposeRunner {
        fn calls(&self) -> Vec<(String, PathBuf, Option<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ComposeRunner for RecordingComposeRunner {
        async fn up(&self, file: &Path, service: Option<&str>) -> Result<()> {
            self.calls.lock().unwrap().push((
                "up".to_string(),
                file.to_path_buf(),
                service.map(str::to_string),
            ));
            *self.running.lock().unwrap() = true;
            Ok(())
        }

        async fn down(&self, file: &Path, service: Option<&str>) -> Result<()> {
            self.calls.lock().unwrap().push((
                "down".to_string(),
                file.to_path_buf(),
                service.map(str::to_string),
            ));
            *self.running.lock().unwrap() = false;
            Ok(())
        }

        async fn is_running(&self, file: &Path, service: Option<&str>) -> Result<bool> {
            self.calls.lock().unwrap().push((
                "is_running".to_string(),
                file.to_path_buf(),
                service.map(str::to_string),
            ));
            Ok(*self.running.lock().unwrap())
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
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/mask"),
                    container: "/workspace/.orrery".to_string(),
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
                    host: PathBuf::from("project/.orrery/agent_io/run-1/cli1/mask"),
                    container: "/workspace/.orrery".to_string(),
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
    fn mounts_mask_orrery_dir() {
        let tmp_root = tempfile::TempDir::new().unwrap();
        let run_id = uuid::Uuid::new_v4();
        let io_dir = agent_io_dir(tmp_root.path(), &run_id, "cli1");

        for mode in [WorkspaceMount::ReadOnly, WorkspaceMount::ReadWrite] {
            let mounts = mounts_for(&cli_agent("cli1", mode), tmp_root.path(), &io_dir);
            let mask_mounts: Vec<&MountSpec> = mounts
                .iter()
                .filter(|m| {
                    let MountSpec::Bind { container, .. } = m;
                    container == "/workspace/.orrery"
                })
                .collect();
            assert_eq!(mask_mounts.len(), 1);
            assert_eq!(
                mask_mounts[0],
                &MountSpec::Bind {
                    host: io_dir.join("mask"),
                    container: "/workspace/.orrery".to_string(),
                    read_only: true,
                }
            );
        }

        let none_mounts = mounts_for(
            &cli_agent("cli1", WorkspaceMount::None),
            tmp_root.path(),
            &io_dir,
        );
        assert!(none_mounts.iter().all(|m| {
            let MountSpec::Bind { container, .. } = m;
            !container.starts_with("/workspace")
        }));
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

    #[tokio::test]
    async fn second_begin_stage_fails_while_open() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            inf_ctrl,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let agents = vec![local_agent("a1")];
        let _lease = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();

        let err = rm
            .begin_stage(run_id, &agents, false, &[])
            .await
            .unwrap_err();
        match err {
            Error::Engine {
                step_number,
                message,
            } => {
                assert_eq!(step_number, 0);
                assert_eq!(message, "stage lease already open");
            }
            other => panic!("expected Error::Engine, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn begin_after_end_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            inf_ctrl,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let agents = vec![local_agent("a1")];
        let lease1 = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        rm.end_stage(lease1).await.unwrap();

        let lease2 = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        assert_eq!(lease2.concurrency(), 1);
        rm.end_stage(lease2).await.unwrap();
    }

    #[tokio::test]
    async fn dropped_lease_does_not_block_next_stage() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            inf_ctrl,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let agents = vec![local_agent("a1")];
        let lease1 = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        drop(lease1);

        let lease2 = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        rm.end_stage(lease2).await.unwrap();
    }

    #[tokio::test]
    async fn stopped_server_started_and_waited() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let lease = rm
            .begin_stage(uuid::Uuid::new_v4(), &[local_agent("a1")], false, &[])
            .await
            .unwrap();
        rm.end_stage(lease).await.unwrap();

        assert_eq!(
            inf_ctrl.calls(),
            vec![
                InferenceCall::IsRunning("coder-14b".to_string()),
                InferenceCall::Start("coder-14b".to_string()),
                InferenceCall::WaitReady("coder-14b".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn running_server_not_started_or_stopped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        inf_ctrl.set_running("coder-14b", true);

        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let lease = rm
            .begin_stage(uuid::Uuid::new_v4(), &[local_agent("a1")], false, &[])
            .await
            .unwrap();
        rm.end_stage(lease).await.unwrap();
        rm.shutdown().await.unwrap();

        let calls = inf_ctrl.calls();
        assert!(!calls.iter().any(|c| matches!(c, InferenceCall::Start(_))));
        assert!(!calls.iter().any(|c| matches!(c, InferenceCall::Stop(_))));
    }

    #[tokio::test]
    async fn external_server_never_started() {
        let tmp = tempfile::TempDir::new().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf).await;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .await;
            }
        });

        let backend = Arc::new(MockContainerBackend::new());
        let compose = Arc::new(RecordingComposeRunner::default());
        let ctrl = Arc::new(DockerInferenceControl::new(
            Arc::clone(&backend) as Arc<dyn ContainerBackend>,
            Arc::clone(&compose) as Arc<dyn ComposeRunner>,
        ));

        let ext = spec_always_on(&format!("http://{addr}"));
        assert!(ctrl.is_running(&ext).await.unwrap());
        ctrl.start(&ext).await.unwrap();
        ctrl.stop(&ext).await.unwrap();
        ctrl.wait_ready(&ext).await.unwrap();
        ctrl.unload(&ext, "model").await.unwrap();

        let rm = ResourceManager::new(
            Arc::clone(&backend) as Arc<dyn ContainerBackend>,
            Arc::clone(&ctrl) as Arc<dyn InferenceControl>,
            vec![ext],
            2,
            tmp.path().to_path_buf(),
        );
        let agent = local_agent_on("a1", "always-on", "m1");
        let lease = rm
            .begin_stage(uuid::Uuid::new_v4(), &[agent], false, &[])
            .await
            .unwrap();
        rm.end_stage(lease).await.unwrap();
        rm.shutdown().await.unwrap();

        assert!(backend.calls().is_empty());
        assert!(compose.calls().is_empty());

        // Also verify Container and Compose delegation + Router HTTP unload on DockerInferenceControl
        backend.add_container("llama-coder-14b", ContainerState::Exited { code: 0 });
        let mut container_inf = spec_coder_14b();
        container_inf.endpoint_url = format!("http://{addr}");
        assert!(!ctrl.is_running(&container_inf).await.unwrap());
        ctrl.start(&container_inf).await.unwrap();
        assert!(ctrl.is_running(&container_inf).await.unwrap());
        ctrl.stop(&container_inf).await.unwrap();

        let compose_file = tmp.path().join(".orrery").join("compose.yaml");
        let mut router_inf = spec_router(compose_file);
        router_inf.endpoint_url = format!("http://{addr}");
        assert!(!ctrl.is_running(&router_inf).await.unwrap());
        ctrl.start(&router_inf).await.unwrap();
        assert!(ctrl.is_running(&router_inf).await.unwrap());
        ctrl.unload(&router_inf, "m1").await.unwrap();
        ctrl.stop(&router_inf).await.unwrap();

        server_task.abort();

        // Verify wait_ready timeout returns Error::Agent { agent_id: inference_id, message: "not ready" }
        let mut unready_ext = spec_always_on(&format!("http://{addr}"));
        unready_ext.startup_timeout_secs = 0;
        let err = ctrl.wait_ready(&unready_ext).await.unwrap_err();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "always-on");
                assert_eq!(message, "not ready");
            }
            other => panic!("expected Error::Agent, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn same_server_next_stage_not_restarted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let agents = vec![local_agent("a1")];
        let lease1 = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        rm.end_stage(lease1).await.unwrap();

        let lease2 = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        rm.end_stage(lease2).await.unwrap();

        let calls = inf_ctrl.calls();
        let starts = calls
            .iter()
            .filter(|c| matches!(c, InferenceCall::Start(id) if id == "coder-14b"))
            .count();
        let stops = calls
            .iter()
            .filter(|c| matches!(c, InferenceCall::Stop(_)))
            .count();
        assert_eq!(starts, 1);
        assert_eq!(stops, 0);
    }

    #[tokio::test]
    async fn single_mode_swapped_between_stages() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let other_inf = InferenceConfig {
            inference_id: "other".to_string(),
            endpoint_url: "http://127.0.0.1:8083".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: "llama-other".to_string(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };

        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![spec_coder_14b(), other_inf],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let stage_a = vec![local_agent("a1")];
        let lease_a = rm.begin_stage(run_id, &stage_a, false, &[]).await.unwrap();
        rm.end_stage(lease_a).await.unwrap();

        let stage_b = vec![local_agent_on("a2", "other", "other-model")];
        let lease_b = rm.begin_stage(run_id, &stage_b, false, &[]).await.unwrap();
        rm.end_stage(lease_b).await.unwrap();

        let calls = inf_ctrl.calls();
        let stop_pos = calls
            .iter()
            .position(|c| c == &InferenceCall::Stop("coder-14b".to_string()))
            .expect("expected Stop(coder-14b)");
        let start_other_pos = calls
            .iter()
            .position(|c| c == &InferenceCall::Start("other".to_string()))
            .expect("expected Start(other)");
        assert!(stop_pos < start_other_pos);
    }

    #[tokio::test]
    async fn router_unloads_models_next_stage_drops() {
        let tmp = tempfile::TempDir::new().unwrap();
        let compose_file = tmp.path().join(".orrery").join("compose.yaml");
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![spec_router(compose_file)],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let stage_a = vec![local_agent_on("a1", "router", "m1")];
        let lease_a = rm.begin_stage(run_id, &stage_a, false, &[]).await.unwrap();
        rm.end_stage(lease_a).await.unwrap();

        let stage_b = vec![local_agent_on("a2", "router", "m2")];
        let lease_b = rm.begin_stage(run_id, &stage_b, false, &[]).await.unwrap();
        rm.end_stage(lease_b).await.unwrap();

        let calls = inf_ctrl.calls();
        assert!(calls.contains(&InferenceCall::Unload(
            "router".to_string(),
            "m1".to_string()
        )));
        assert!(!calls.contains(&InferenceCall::Unload(
            "router".to_string(),
            "m2".to_string()
        )));
    }

    #[tokio::test]
    async fn unload_after_stage_false_keeps_server() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());

        let mut coder_inf = spec_coder_14b();
        coder_inf.unload_after_stage = false;

        let mut router_inf = spec_router(tmp.path().join(".orrery").join("compose.yaml"));
        router_inf.unload_after_stage = false;

        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![coder_inf, router_inf],
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let stage_a = vec![local_agent("a1"), local_agent_on("a2", "router", "m1")];
        let lease_a = rm.begin_stage(run_id, &stage_a, false, &[]).await.unwrap();
        rm.end_stage(lease_a).await.unwrap();

        let stage_b = vec![cloud_agent("c1")];
        let lease_b = rm.begin_stage(run_id, &stage_b, false, &[]).await.unwrap();
        rm.end_stage(lease_b).await.unwrap();

        let calls = inf_ctrl.calls();
        assert!(!calls.iter().any(|c| matches!(c, InferenceCall::Stop(_))));
        assert!(
            !calls
                .iter()
                .any(|c| matches!(c, InferenceCall::Unload(_, _)))
        );
    }

    #[tokio::test]
    async fn end_stage_removes_cli_containers_and_agent_io() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            Arc::clone(&backend) as Arc<dyn ContainerBackend>,
            inf_ctrl,
            Vec::new(),
            2,
            tmp.path().to_path_buf(),
        );

        let run_id = uuid::Uuid::new_v4();
        let io_dir = agent_io_dir(tmp.path(), &run_id, "cli-validator");
        fs::create_dir_all(io_dir.join("in")).unwrap();
        fs::create_dir_all(io_dir.join("out")).unwrap();
        let run_io_root = tmp
            .path()
            .join(".orrery")
            .join("agent_io")
            .join(run_id.to_string());
        assert!(run_io_root.exists());

        let agents = vec![cli_agent("cli-validator", WorkspaceMount::ReadOnly)];
        let lease = rm.begin_stage(run_id, &agents, false, &[]).await.unwrap();
        rm.end_stage(lease).await.unwrap();

        let expected_name = cli_container_name(&run_id, "cli-validator");
        assert_eq!(backend.calls(), vec![MockCall::Remove(expected_name)]);
        assert!(!run_io_root.exists());
    }

    #[tokio::test]
    async fn unknown_inference_id_is_validation_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            inf_ctrl,
            vec![spec_coder_14b()],
            2,
            tmp.path().to_path_buf(),
        );

        let bad_agent = local_agent_on("a1", "missing-server", "m1");
        let err = rm
            .begin_stage(uuid::Uuid::new_v4(), &[bad_agent], false, &[])
            .await
            .unwrap_err();
        match err {
            Error::Validation { field, .. } => assert_eq!(field, "inference_id"),
            other => panic!("expected Error::Validation, got {other:?}"),
        }

        // Failed begin_stage must not leave lease_open stuck true
        let lease = rm
            .begin_stage(uuid::Uuid::new_v4(), &[local_agent("a1")], false, &[])
            .await
            .unwrap();
        rm.end_stage(lease).await.unwrap();
    }

    #[tokio::test]
    async fn slots_limit_parallelism() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());
        let rm = ResourceManager::new(
            backend,
            inf_ctrl,
            vec![spec_coder_14b()],
            1,
            tmp.path().to_path_buf(),
        );

        let agents = vec![local_agent("a1"), local_agent("a2")];
        let lease = rm
            .begin_stage(uuid::Uuid::new_v4(), &agents, false, &[])
            .await
            .unwrap();
        assert_eq!(lease.concurrency(), 1);

        let permit1 = lease.acquire_slot().await.unwrap();
        let second_attempt =
            tokio::time::timeout(Duration::from_millis(50), lease.acquire_slot()).await;
        assert!(second_attempt.is_err());

        drop(permit1);
        let _permit2 = lease.acquire_slot().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_unloads_router_and_stops_only_started() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backend = Arc::new(MockContainerBackend::new());
        let inf_ctrl = Arc::new(MockInferenceControl::new());

        let pre_running = InferenceConfig {
            inference_id: "pre-running".to_string(),
            endpoint_url: "http://127.0.0.1:8084".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: "llama-pre".to_string(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        inf_ctrl.set_running("pre-running", true);

        let router_inf = spec_router(tmp.path().join(".orrery").join("compose.yaml"));
        let rm = ResourceManager::new(
            backend,
            Arc::clone(&inf_ctrl) as Arc<dyn InferenceControl>,
            vec![spec_coder_14b(), router_inf, pre_running],
            3,
            tmp.path().to_path_buf(),
        );

        let agents = vec![
            local_agent("a1"),
            local_agent_on("a2", "router", "router-m1"),
            local_agent_on("a3", "pre-running", "pre-m1"),
        ];
        let lease = rm
            .begin_stage(uuid::Uuid::new_v4(), &agents, false, &[])
            .await
            .unwrap();
        rm.end_stage(lease).await.unwrap();

        rm.shutdown().await.unwrap();

        let calls = inf_ctrl.calls();
        assert!(calls.contains(&InferenceCall::Unload(
            "router".to_string(),
            "router-m1".to_string()
        )));
        assert!(calls.contains(&InferenceCall::Stop("coder-14b".to_string())));
        assert!(calls.contains(&InferenceCall::Stop("router".to_string())));
        assert!(!calls.contains(&InferenceCall::Stop("pre-running".to_string())));
    }

    #[test]
    fn compose_args_shapes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("docker-models/router/compose.yaml");
        let f = file.to_string_lossy().into_owned();

        assert_eq!(
            compose_args(&file, Some("llama"), ComposeOp::Up),
            vec!["compose", "-f", &f, "up", "-d", "llama"]
        );
        assert_eq!(
            compose_args(&file, None, ComposeOp::Up),
            vec!["compose", "-f", &f, "up", "-d"]
        );
        assert_eq!(
            compose_args(&file, None, ComposeOp::Down),
            vec!["compose", "-f", &f, "down"]
        );
        assert_eq!(
            compose_args(&file, Some("llama"), ComposeOp::Down),
            vec!["compose", "-f", &f, "stop", "llama"]
        );
        assert_eq!(
            compose_args(&file, Some("llama"), ComposeOp::Ps),
            vec![
                "compose", "-f", &f, "ps", "--status", "running", "-q", "llama"
            ]
        );
        assert_eq!(
            compose_args(&file, None, ComposeOp::Ps),
            vec!["compose", "-f", &f, "ps", "--status", "running", "-q"]
        );
    }

    #[tokio::test]
    async fn compose_cli_rejects_relative_file() {
        let cli = DockerComposeCli;
        let rel = Path::new("compose.yaml");

        for res in [
            cli.up(rel, None).await,
            cli.down(rel, Some("llama")).await,
            cli.is_running(rel, None).await.map(|_| ()),
        ] {
            let err = res.unwrap_err();
            match err {
                Error::Validation { field, message } => {
                    assert_eq!(field, "compose");
                    assert_eq!(
                        message,
                        "compose file must be resolved by ConfigStore::load_inference"
                    );
                }
                other => panic!("expected Error::Validation, got {other:?}"),
            }
        }
    }

    #[test]
    fn create_body_maps_image_cmd_env_memory() {
        let spec = ContainerSpec {
            name: "orrery-cli-12345678-coder".to_string(),
            image: "ghcr.io/example/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            cmd: vec!["run".to_string(), "--prompt".to_string()],
            env: vec![
                ("ORRERY_MODE".to_string(), EnvValue::Plain("test".to_string())),
                ("LOG_LEVEL".to_string(), EnvValue::Plain("info".to_string())),
            ],
            mounts: Vec::new(),
            memory_limit_mb: 512,
            auto_remove: true,
        };

        let body = create_body(&spec);
        assert_eq!(body.image.as_deref(), Some(spec.image.as_str()));
        assert_eq!(
            body.cmd,
            Some(vec!["run".to_string(), "--prompt".to_string()])
        );
        assert_eq!(
            body.env,
            Some(vec![
                "ORRERY_MODE=test".to_string(),
                "LOG_LEVEL=info".to_string(),
            ])
        );
        assert_eq!(body.open_stdin, Some(true));
        assert_eq!(body.attach_stdin, Some(true));
        assert_eq!(body.stdin_once, Some(true));

        let hc = body.host_config.expect("host_config should be populated");
        assert_eq!(hc.memory, Some(512 * 1024 * 1024));
        assert_eq!(hc.auto_remove, Some(true));
    }

    #[test]
    fn create_body_binds_readonly_with_ro_suffix() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws_host = tmp.path().join("project");
        let out_host = tmp.path().join("out");

        let spec = ContainerSpec {
            name: "orrery-cli-12345678-coder".to_string(),
            image: "ghcr.io/example/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            cmd: vec!["agent".to_string()],
            env: Vec::new(),
            mounts: vec![
                MountSpec::Bind {
                    host: ws_host.clone(),
                    container: "/workspace".to_string(),
                    read_only: true,
                },
                MountSpec::Bind {
                    host: out_host.clone(),
                    container: "/orrery/out".to_string(),
                    read_only: false,
                },
            ],
            memory_limit_mb: 256,
            auto_remove: false,
        };

        let body = create_body(&spec);
        let hc = body.host_config.expect("host_config should be populated");
        let binds = hc.binds.expect("binds should be populated");
        assert_eq!(
            binds,
            vec![
                format!("{}:/workspace:ro", ws_host.to_string_lossy()),
                format!("{}:/orrery/out", out_host.to_string_lossy()),
            ]
        );
    }

    #[test]
    fn create_body_sets_no_gpu_or_ports() {
        let spec = ContainerSpec {
            name: "orrery-cli-12345678-coder".to_string(),
            image: "ghcr.io/example/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            cmd: vec!["agent".to_string()],
            env: Vec::new(),
            mounts: Vec::new(),
            memory_limit_mb: 1024,
            auto_remove: false,
        };

        let body = create_body(&spec);
        assert!(body.exposed_ports.is_none());
        let hc = body.host_config.expect("host_config should be populated");
        assert!(hc.device_requests.is_none());
        assert!(hc.devices.is_none());
        assert!(hc.port_bindings.is_none());

        assert!(is_not_found(
            &bollard::errors::Error::DockerResponseServerError {
                status_code: 404,
                message: "not found".to_string(),
            }
        ));
        assert!(!is_not_found(
            &bollard::errors::Error::DockerResponseServerError {
                status_code: 500,
                message: "internal".to_string(),
            }
        ));
        assert_eq!(
            map_inspect_state(Some(&bollard::models::ContainerState {
                running: Some(true),
                ..Default::default()
            })),
            ContainerState::Running
        );
        assert_eq!(
            map_inspect_state(Some(&bollard::models::ContainerState {
                running: Some(false),
                status: Some(ContainerStateStatusEnum::CREATED),
                ..Default::default()
            })),
            ContainerState::Created
        );
        assert_eq!(
            map_inspect_state(Some(&bollard::models::ContainerState {
                running: Some(false),
                status: Some(ContainerStateStatusEnum::EXITED),
                exit_code: Some(42),
                ..Default::default()
            })),
            ContainerState::Exited { code: 42 }
        );
        assert_eq!(map_inspect_state(None), ContainerState::Exited { code: 0 });
    }

    #[test]
    fn secret_env_only_in_create_body() {
        let secret_val = "super-secret-value-998877";
        let spec = ContainerSpec {
            name: "orrery-cli-12345678-coder".to_string(),
            image: "ghcr.io/example/cli@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            cmd: vec!["agent".to_string()],
            env: vec![
                (
                    "ANTHROPIC_API_KEY".to_string(),
                    EnvValue::Secret(SecretValue::new(secret_val)),
                ),
                ("PLAIN_VAR".to_string(), EnvValue::Plain("visible".to_string())),
            ],
            mounts: Vec::new(),
            memory_limit_mb: 512,
            auto_remove: false,
        };

        let dbg = format!("{spec:?}");
        assert!(
            !dbg.contains(secret_val),
            "ContainerSpec Debug output must never leak SecretValue"
        );

        let body = create_body(&spec);
        let env = body.env.expect("env should be populated");
        assert!(env.contains(&format!("ANTHROPIC_API_KEY={secret_val}")));
        assert!(env.contains(&"PLAIN_VAR=visible".to_string()));
    }
}
