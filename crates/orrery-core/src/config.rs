use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

static ATOMIC_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub const MIN_STAGES: usize = 2;
pub const MAX_STAGES: usize = 15;
pub const MAX_STAGE_AGENTS: usize = 15;
pub const MAX_MAPPED_TOOL_NAME_LEN: usize = 64;
pub const AGENT_CLI_GUARDRAIL_NOTE: &str = "Guardrails: The MCP gatekeeper does not see a CLI's built-in tools. Guardrails are: workspace_mount, container isolation, memory/time limits, and whatever tool config the image ships.";

const EMBEDDED_SECRET_PATTERNS: [&str; 5] = ["sk-", "Bearer ", "AIza", "ghp_", "xox"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    #[default]
    Run,
    Loop,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationMode {
    #[default]
    Auto,
    Agents,
    GateAndOperator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageConfig {
    pub role_id: String,
    #[serde(default)]
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    StepStart,
    StepPass,
    StepRetry,
    Halt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookFailure {
    #[default]
    Halt,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookConfig {
    pub tool: String,
    #[serde(default)]
    pub on_failure: HookFailure,
}

impl HookConfig {
    pub fn validate(&self, field: &str) -> Result<()> {
        validate_concrete_tool_ref(&self.tool, field)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HooksConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_step_start: Option<HookConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_step_pass: Option<HookConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_step_retry: Option<HookConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_halt: Option<HookConfig>,
}

impl HooksConfig {
    pub fn is_empty(&self) -> bool {
        self.on_step_start.is_none()
            && self.on_step_pass.is_none()
            && self.on_step_retry.is_none()
            && self.on_halt.is_none()
    }

    pub fn get(&self, event: HookEvent) -> Option<&HookConfig> {
        match event {
            HookEvent::StepStart => self.on_step_start.as_ref(),
            HookEvent::StepPass => self.on_step_pass.as_ref(),
            HookEvent::StepRetry => self.on_step_retry.as_ref(),
            HookEvent::Halt => self.on_halt.as_ref(),
        }
    }

    pub fn set(&mut self, event: HookEvent, hook: HookConfig) {
        match event {
            HookEvent::StepStart => self.on_step_start = Some(hook),
            HookEvent::StepPass => self.on_step_pass = Some(hook),
            HookEvent::StepRetry => self.on_step_retry = Some(hook),
            HookEvent::Halt => self.on_halt = Some(hook),
        }
    }

    pub fn clear(&mut self, event: HookEvent) {
        match event {
            HookEvent::StepStart => self.on_step_start = None,
            HookEvent::StepPass => self.on_step_pass = None,
            HookEvent::StepRetry => self.on_step_retry = None,
            HookEvent::Halt => self.on_halt = None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if let Some(hook) = &self.on_step_start {
            hook.validate("hooks.on_step_start.tool")?;
        }
        if let Some(hook) = &self.on_step_pass {
            hook.validate("hooks.on_step_pass.tool")?;
        }
        if let Some(hook) = &self.on_step_retry {
            hook.validate("hooks.on_step_retry.tool")?;
        }
        if let Some(hook) = &self.on_halt {
            hook.validate("hooks.on_halt.tool")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeLimits {
    pub max_tool_calls: usize,
    pub model_idle_timeout_secs: u64,
    pub live_stream_ring_capacity: usize,
    pub live_stream_max_file_bytes: u64,
    pub live_stream_max_files: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_tool_calls: 50,
            model_idle_timeout_secs: 300,
            live_stream_ring_capacity: 2_000,
            live_stream_max_file_bytes: 10 * 1024 * 1024,
            live_stream_max_files: 3,
        }
    }
}

impl RuntimeLimits {
    pub fn validate(&self) -> Result<()> {
        if !(1..=1000).contains(&self.max_tool_calls) {
            return Err(Error::Validation {
                field: "limits.max_tool_calls".to_string(),
                message: "max_tool_calls must be in 1..=1000".to_string(),
            });
        }
        if !(1..=86_400).contains(&self.model_idle_timeout_secs) {
            return Err(Error::Validation {
                field: "limits.model_idle_timeout_secs".to_string(),
                message: "model_idle_timeout_secs must be in 1..=86_400".to_string(),
            });
        }
        if self.live_stream_ring_capacity < 1 {
            return Err(Error::Validation {
                field: "limits.live_stream_ring_capacity".to_string(),
                message: "live_stream_ring_capacity must be >= 1".to_string(),
            });
        }
        if self.live_stream_max_file_bytes < 4096 {
            return Err(Error::Validation {
                field: "limits.live_stream_max_file_bytes".to_string(),
                message: "live_stream_max_file_bytes must be >= 4096".to_string(),
            });
        }
        if !(1..=10).contains(&self.live_stream_max_files) {
            return Err(Error::Validation {
                field: "limits.live_stream_max_files".to_string(),
                message: "live_stream_max_files must be in 1..=10".to_string(),
            });
        }
        Ok(())
    }

    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub project_name: String,
    pub plan_path: PathBuf,
    pub step_marker: String,
    pub execution_mode: ExecutionMode,
    pub cycle_count: usize,
    pub max_concurrent_agents: usize,
    pub max_step_retries: usize,
    pub validation_mode: ValidationMode,
    #[serde(default)]
    pub gate_tools: Vec<String>,
    pub stages: Vec<StageConfig>,
    #[serde(default, skip_serializing_if = "HooksConfig::is_empty")]
    pub hooks: HooksConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inference: Vec<InferenceConfig>,
    #[serde(default, skip_serializing_if = "RuntimeLimits::is_default")]
    pub limits: RuntimeLimits,
}

impl ProjectConfig {
    pub fn default_template(project_name: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            plan_path: PathBuf::from("STEPPED_IMPLEMENTATION_PLAN.md"),
            step_marker: "## Step ".to_string(),
            execution_mode: ExecutionMode::Run,
            cycle_count: 0,
            max_concurrent_agents: 2,
            max_step_retries: 3,
            validation_mode: ValidationMode::Auto,
            gate_tools: Vec::new(),
            stages: vec![
                StageConfig {
                    role_id: "orchestrator".to_string(),
                    agent_ids: Vec::new(),
                },
                StageConfig {
                    role_id: "coder".to_string(),
                    agent_ids: Vec::new(),
                },
                StageConfig {
                    role_id: "validator".to_string(),
                    agent_ids: Vec::new(),
                },
            ],
            hooks: HooksConfig::default(),
            inference: Vec::new(),
            limits: RuntimeLimits::default(),
        }
    }

    pub fn effective_validation_mode(&self, roles: &[RoleConfig]) -> ValidationMode {
        match self.validation_mode {
            ValidationMode::Auto => {
                let has_validator = self.stages.iter().any(|stage| {
                    roles
                        .iter()
                        .find(|r| r.role_id == stage.role_id)
                        .is_some_and(RoleConfig::is_validator)
                });
                if has_validator {
                    ValidationMode::Agents
                } else {
                    ValidationMode::GateAndOperator
                }
            }
            mode => mode,
        }
    }

    pub fn validate_staging(&self) -> Result<()> {
        if self.project_name.trim().is_empty() {
            return Err(Error::Validation {
                field: "project_name".to_string(),
                message: "project_name must not be empty".to_string(),
            });
        }

        if !(MIN_STAGES..=MAX_STAGES).contains(&self.stages.len()) {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!(
                    "stage count must be in {MIN_STAGES}..={MAX_STAGES}, got {}",
                    self.stages.len()
                ),
            });
        }

        if self.step_marker.trim().is_empty() {
            return Err(Error::Validation {
                field: "step_marker".to_string(),
                message: "step_marker must contain at least one non-whitespace character"
                    .to_string(),
            });
        }

        validate_safe_relative_path(&self.plan_path, "plan_path")?;

        if self.max_concurrent_agents < 1 {
            return Err(Error::Validation {
                field: "max_concurrent_agents".to_string(),
                message: "max_concurrent_agents must be >= 1".to_string(),
            });
        }

        for (idx, stage) in self.stages.iter().enumerate() {
            if stage.role_id.trim().is_empty() {
                return Err(Error::Validation {
                    field: format!("stages[{idx}].role_id"),
                    message: "role_id must not be empty".to_string(),
                });
            }
            if idx == 0 {
                if stage.agent_ids.len() > 1 {
                    return Err(Error::Validation {
                        field: "stages[0].agent_ids".to_string(),
                        message: format!(
                            "orchestrator stage 0 allows at most 1 agent, got {}",
                            stage.agent_ids.len()
                        ),
                    });
                }
            } else {
                if stage.role_id == "orchestrator" {
                    return Err(Error::Validation {
                        field: format!("stages[{idx}].role_id"),
                        message: "orchestrator role is only allowed at stage 0".to_string(),
                    });
                }
                if stage.agent_ids.len() > MAX_STAGE_AGENTS {
                    return Err(Error::Validation {
                        field: format!("stages[{idx}].agent_ids"),
                        message: format!(
                            "stage {idx} allows at most {MAX_STAGE_AGENTS} agents, got {}",
                            stage.agent_ids.len()
                        ),
                    });
                }
            }
            for agent_id in &stage.agent_ids {
                if agent_id.trim().is_empty() {
                    return Err(Error::Validation {
                        field: format!("stages[{idx}].agent_ids"),
                        message: "agent_id must not be empty".to_string(),
                    });
                }
            }
        }

        for (idx, tool_ref) in self.gate_tools.iter().enumerate() {
            validate_concrete_tool_ref(tool_ref, &format!("gate_tools[{idx}]"))?;
        }

        self.hooks.validate()?;
        self.limits.validate()?;

        for inf in &self.inference {
            inf.validate()?;
        }

        Ok(())
    }

    pub fn validate(&self, roles: &[RoleConfig], agents: &[AgentConfig]) -> Result<Vec<Warning>> {
        self.validate_staging()?;

        for role in roles {
            role.validate()?;
        }
        for agent in agents {
            agent.validate()?;
        }

        let mut orchestrator_stages = Vec::new();
        let mut has_validator = false;

        for (idx, stage) in self.stages.iter().enumerate() {
            let role = roles
                .iter()
                .find(|r| r.role_id == stage.role_id)
                .ok_or_else(|| Error::Validation {
                    field: format!("stages[{idx}].role_id"),
                    message: format!("unknown role_id '{}'", stage.role_id),
                })?;

            if role.is_orchestrator() {
                orchestrator_stages.push(idx);
            }
            if role.is_validator() {
                has_validator = true;
            }

            for agent_id in &stage.agent_ids {
                if !agents.iter().any(|a| &a.agent_id == agent_id) {
                    return Err(Error::Validation {
                        field: format!("stages[{idx}].agent_ids"),
                        message: format!("unknown agent_id '{agent_id}'"),
                    });
                }
            }
        }

        if orchestrator_stages.as_slice() != [0] {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!(
                    "exactly one stage must have an orchestrator role and it must be stage 0 (found at {orchestrator_stages:?})"
                ),
            });
        }

        if self.validation_mode == ValidationMode::Agents && !has_validator {
            return Err(Error::Validation {
                field: "validation_mode".to_string(),
                message: "Agents validation mode requires at least one validator stage".to_string(),
            });
        }

        let mut warnings = Vec::new();
        let effective_mode = self.effective_validation_mode(roles);
        if effective_mode == ValidationMode::GateAndOperator && self.gate_tools.is_empty() {
            warnings.push(Warning {
                code: "EMPTY_GATE_TOOLS".to_string(),
                message:
                    "GateAndOperator validation mode has no gate_tools configured (operator-only validation)"
                        .to_string(),
            });
        }

        Ok(warnings)
    }

    pub fn validate_for_run(
        &self,
        roles: &[RoleConfig],
        agents: &[AgentConfig],
    ) -> Result<Vec<Warning>> {
        let warnings = self.validate(roles, agents)?;

        if self.stages[0].agent_ids.len() != 1 {
            return Err(Error::Validation {
                field: "stages[0].agent_ids".to_string(),
                message: "orchestrator stage 0 must have exactly 1 agent when running".to_string(),
            });
        }

        for (idx, stage) in self.stages.iter().enumerate().skip(1) {
            if stage.agent_ids.is_empty() || stage.agent_ids.len() > MAX_STAGE_AGENTS {
                return Err(Error::Validation {
                    field: format!("stages[{idx}].agent_ids"),
                    message: format!(
                        "stage {idx} must have 1..={MAX_STAGE_AGENTS} agents when running, got {}",
                        stage.agent_ids.len()
                    ),
                });
            }
        }

        Ok(warnings)
    }

    pub fn set_stage_count(&mut self, count: usize) -> Result<()> {
        if !(MIN_STAGES..=MAX_STAGES).contains(&count) {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!("stage count must be in {MIN_STAGES}..={MAX_STAGES}, got {count}"),
            });
        }

        let mut next = self.clone();
        if count > next.stages.len() {
            while next.stages.len() < count {
                next.stages.push(StageConfig {
                    role_id: "coder".to_string(),
                    agent_ids: Vec::new(),
                });
            }
        } else {
            next.stages.truncate(count);
        }

        next.validate_staging()?;
        *self = next;
        Ok(())
    }

    pub fn set_stage_role(&mut self, index: usize, role_id: impl Into<String>) -> Result<()> {
        if index >= self.stages.len() {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!(
                    "stage index {index} out of bounds (len {})",
                    self.stages.len()
                ),
            });
        }

        let mut next = self.clone();
        next.stages[index].role_id = role_id.into();
        next.validate_staging()?;
        *self = next;
        Ok(())
    }

    pub fn assign_agent(&mut self, index: usize, agent_id: impl Into<String>) -> Result<()> {
        if index >= self.stages.len() {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!(
                    "stage index {index} out of bounds (len {})",
                    self.stages.len()
                ),
            });
        }

        let mut next = self.clone();
        next.stages[index].agent_ids.push(agent_id.into());
        next.validate_staging()?;
        *self = next;
        Ok(())
    }

    pub fn remove_agent(&mut self, index: usize, agent_id: &str) -> Result<()> {
        if index >= self.stages.len() {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!(
                    "stage index {index} out of bounds (len {})",
                    self.stages.len()
                ),
            });
        }

        let mut next = self.clone();
        let pos = next.stages[index]
            .agent_ids
            .iter()
            .position(|id| id == agent_id)
            .ok_or_else(|| Error::Validation {
                field: format!("stages[{index}].agent_ids"),
                message: format!("agent '{agent_id}' is not assigned to stage {index}"),
            })?;
        next.stages[index].agent_ids.remove(pos);
        next.validate_staging()?;
        *self = next;
        Ok(())
    }

    pub fn clear_stage(&mut self, index: usize) -> Result<()> {
        if index >= self.stages.len() {
            return Err(Error::Validation {
                field: "stages".to_string(),
                message: format!(
                    "stage index {index} out of bounds (len {})",
                    self.stages.len()
                ),
            });
        }

        let mut next = self.clone();
        next.stages[index].agent_ids.clear();
        next.validate_staging()?;
        *self = next;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleKind {
    Orchestrator,
    Implementer,
    Validator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleConfig {
    pub role_id: String,
    pub display_name: String,
    pub kind: RoleKind,
    pub rules_file: String,
    #[serde(default)]
    pub allowed_mcp_tools: Vec<String>,
}

impl RoleConfig {
    pub fn is_orchestrator(&self) -> bool {
        self.kind == RoleKind::Orchestrator
    }

    pub fn is_validator(&self) -> bool {
        self.kind == RoleKind::Validator
    }

    pub fn is_implementer(&self) -> bool {
        self.kind == RoleKind::Implementer
    }

    pub fn is_writer(&self, servers: &[McpServerConfig], stage_agents: &[AgentConfig]) -> bool {
        if stage_agents.iter().any(|agent| {
            matches!(
                agent.backend,
                AgentBackend::AgentCli {
                    workspace_mount: WorkspaceMount::ReadWrite,
                    ..
                }
            )
        }) {
            return true;
        }

        for server in servers {
            for write_tool in &server.write_tools {
                let exact = format!("{}:{}", server.server_id, write_tool.tool);
                let wildcard = format!("{}:*", server.server_id);
                if self
                    .allowed_mcp_tools
                    .iter()
                    .any(|allowed| allowed == &exact || allowed == &wildcard)
                {
                    return true;
                }
            }
        }

        false
    }

    pub fn validate(&self) -> Result<()> {
        validate_config_file_id(&self.role_id, "role_id")?;
        if self.role_id.trim().is_empty() {
            return Err(Error::Validation {
                field: "role_id".to_string(),
                message: "role_id must not be empty".to_string(),
            });
        }
        if self.display_name.trim().is_empty() {
            return Err(Error::Validation {
                field: "display_name".to_string(),
                message: "display_name must not be empty".to_string(),
            });
        }
        validate_safe_relative_path(Path::new(&self.rules_file), "rules_file")?;

        for (idx, pattern) in self.allowed_mcp_tools.iter().enumerate() {
            validate_role_tool_pattern(pattern, &format!("allowed_mcp_tools[{idx}]"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceMount {
    #[default]
    None,
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum AgentBackend {
    LocalModel {
        inference_id: String,
        model: String,
        temperature: f64,
        max_context_tokens: usize,
    },
    AgentCli {
        image: String,
        command: Vec<String>,
        workspace_mount: WorkspaceMount,
        memory_limit_mb: u64,
        timeout_secs: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keychain_service: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keychain_account: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret_env: Option<String>,
    },
    CloudApi {
        endpoint_url: String,
        model: String,
        temperature: f64,
        max_context_tokens: usize,
        keychain_service: String,
        keychain_account: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    pub agent_id: String,
    pub display_name: String,
    pub backend: AgentBackend,
}

impl AgentConfig {
    pub fn validate_no_embedded_secrets(&self) -> Result<()> {
        check_no_embedded_secret("agent_id", &self.agent_id)?;
        check_no_embedded_secret("display_name", &self.display_name)?;

        match &self.backend {
            AgentBackend::LocalModel {
                inference_id,
                model,
                ..
            } => {
                check_no_embedded_secret("backend.inference_id", inference_id)?;
                check_no_embedded_secret("backend.model", model)?;
            }
            AgentBackend::AgentCli {
                image,
                command,
                keychain_service,
                keychain_account,
                secret_env,
                ..
            } => {
                check_no_embedded_secret("backend.image", image)?;
                for (idx, arg) in command.iter().enumerate() {
                    check_no_embedded_secret(&format!("backend.command[{idx}]"), arg)?;
                }
                if let Some(val) = keychain_service {
                    check_no_embedded_secret("backend.keychain_service", val)?;
                }
                if let Some(val) = keychain_account {
                    check_no_embedded_secret("backend.keychain_account", val)?;
                }
                if let Some(val) = secret_env {
                    check_no_embedded_secret("backend.secret_env", val)?;
                }
            }
            AgentBackend::CloudApi {
                endpoint_url,
                model,
                keychain_service,
                keychain_account,
                ..
            } => {
                check_no_embedded_secret("backend.endpoint_url", endpoint_url)?;
                check_no_embedded_secret("backend.model", model)?;
                check_no_embedded_secret("backend.keychain_service", keychain_service)?;
                check_no_embedded_secret("backend.keychain_account", keychain_account)?;
            }
        }

        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        validate_config_file_id(&self.agent_id, "agent_id")?;
        self.validate_no_embedded_secrets()?;

        if self.agent_id.trim().is_empty() {
            return Err(Error::Validation {
                field: "agent_id".to_string(),
                message: "agent_id must not be empty".to_string(),
            });
        }
        if self.display_name.trim().is_empty() {
            return Err(Error::Validation {
                field: "display_name".to_string(),
                message: "display_name must not be empty".to_string(),
            });
        }

        match &self.backend {
            AgentBackend::LocalModel {
                inference_id,
                model,
                max_context_tokens,
                ..
            } => {
                if inference_id.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "backend.inference_id".to_string(),
                        message: "inference_id must not be empty".to_string(),
                    });
                }
                if model.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "backend.model".to_string(),
                        message: "model must not be empty".to_string(),
                    });
                }
                if *max_context_tokens == 0 {
                    return Err(Error::Validation {
                        field: "backend.max_context_tokens".to_string(),
                        message: "max_context_tokens must be > 0".to_string(),
                    });
                }
            }
            AgentBackend::AgentCli {
                image,
                command,
                memory_limit_mb,
                timeout_secs,
                keychain_service,
                keychain_account,
                secret_env,
                ..
            } => {
                validate_container_image(image, "backend.image")?;
                if command.is_empty() || command.iter().any(|c| c.trim().is_empty()) {
                    return Err(Error::Validation {
                        field: "backend.command".to_string(),
                        message: "command must be non-empty".to_string(),
                    });
                }
                if *memory_limit_mb == 0 {
                    return Err(Error::Validation {
                        field: "backend.memory_limit_mb".to_string(),
                        message: "memory_limit_mb must be > 0".to_string(),
                    });
                }
                if *timeout_secs == 0 {
                    return Err(Error::Validation {
                        field: "backend.timeout_secs".to_string(),
                        message: "timeout_secs must be > 0".to_string(),
                    });
                }
                let secret_fields = [
                    keychain_service.as_deref(),
                    keychain_account.as_deref(),
                    secret_env.as_deref(),
                ];
                let present_count = secret_fields.iter().filter(|f| f.is_some()).count();
                if present_count != 0 && present_count != 3 {
                    return Err(Error::Validation {
                        field: "backend.secret_env".to_string(),
                        message: "keychain_service, keychain_account, and secret_env must all be set together or all omitted".to_string(),
                    });
                }
                if present_count == 3
                    && secret_fields
                        .iter()
                        .any(|f| f.is_none_or(|s| s.trim().is_empty()))
                {
                    return Err(Error::Validation {
                        field: "backend.secret_env".to_string(),
                        message:
                            "keychain_service, keychain_account, and secret_env must not be empty"
                                .to_string(),
                    });
                }
            }
            AgentBackend::CloudApi {
                endpoint_url,
                model,
                max_context_tokens,
                keychain_service,
                keychain_account,
                ..
            } => {
                if endpoint_url.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "backend.endpoint_url".to_string(),
                        message: "endpoint_url must not be empty".to_string(),
                    });
                }
                if model.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "backend.model".to_string(),
                        message: "model must not be empty".to_string(),
                    });
                }
                if *max_context_tokens == 0 {
                    return Err(Error::Validation {
                        field: "backend.max_context_tokens".to_string(),
                        message: "max_context_tokens must be > 0".to_string(),
                    });
                }
                if keychain_service.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "backend.keychain_service".to_string(),
                        message: "keychain_service must not be empty".to_string(),
                    });
                }
                if keychain_account.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "backend.keychain_account".to_string(),
                        message: "keychain_account must not be empty".to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

fn default_true() -> bool {
    true
}

fn default_startup_timeout() -> u64 {
    300
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Project,
    Machine,
}

pub fn resolve_compose_file(
    file: &Path,
    scope: ConfigScope,
    project_root: &Path,
    global_dir: Option<&Path>,
) -> Result<PathBuf> {
    if file.as_os_str().is_empty() || file.to_string_lossy().trim().is_empty() {
        return Err(Error::Validation {
            field: "lifecycle.file".to_string(),
            message: "compose file must not be empty".to_string(),
        });
    }

    let candidate = match (scope, file.is_absolute()) {
        (ConfigScope::Project, true) => {
            return Err(Error::Validation {
                field: "lifecycle.file".to_string(),
                message: "project config must use a path relative to .orrery/".to_string(),
            });
        }
        (ConfigScope::Project, false) => {
            resolve_safe_within_dir(&project_root.join(".orrery"), file, "lifecycle.file")?
        }
        (ConfigScope::Machine, false) => {
            let Some(gdir) = global_dir else {
                return Err(Error::Validation {
                    field: "lifecycle.file".to_string(),
                    message: "global_dir is required to resolve a relative machine compose file"
                        .to_string(),
                });
            };
            gdir.join(file)
        }
        (ConfigScope::Machine, true) => file.to_path_buf(),
    };

    let canonical_file = fs::canonicalize(&candidate).map_err(|source| Error::Io {
        path: candidate,
        source,
    })?;

    let meta = fs::metadata(&canonical_file).map_err(|source| Error::Io {
        path: canonical_file.clone(),
        source,
    })?;
    if !meta.is_file() {
        return Err(Error::Validation {
            field: "lifecycle.file".to_string(),
            message: format!(
                "compose file '{}' must be a regular file",
                canonical_file.display()
            ),
        });
    }

    let canonical_root = fs::canonicalize(project_root).map_err(|source| Error::Io {
        path: project_root.to_path_buf(),
        source,
    })?;
    let canonical_orrery = canonical_root.join(".orrery");
    if canonical_file.starts_with(&canonical_root) && !canonical_file.starts_with(&canonical_orrery)
    {
        return Err(Error::Security {
            message: "compose file is inside the agent-writable workspace".to_string(),
        });
    }

    Ok(canonical_file)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InferenceMode {
    #[default]
    Single,
    Router,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum InferenceLifecycle {
    #[default]
    External,
    Container {
        name: String,
    },
    Compose {
        file: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        service: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceConfig {
    pub inference_id: String,
    pub endpoint_url: String,
    #[serde(default)]
    pub mode: InferenceMode,
    #[serde(default)]
    pub lifecycle: InferenceLifecycle,
    #[serde(default = "default_true")]
    pub unload_after_stage: bool,
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout_secs: u64,
}

impl InferenceConfig {
    pub fn chat_url(&self) -> String {
        format!(
            "{}/v1/chat/completions",
            self.endpoint_url.trim_end_matches('/')
        )
    }

    pub fn validate_project_scope(&self) -> Result<()> {
        if let InferenceLifecycle::Compose { file, .. } = &self.lifecycle
            && file.is_absolute()
        {
            return Err(Error::Validation {
                field: "lifecycle.file".to_string(),
                message: "project config must use a path relative to .orrery/".to_string(),
            });
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        validate_config_file_id(&self.inference_id, "inference_id")?;
        check_no_embedded_secret("inference_id", &self.inference_id)?;
        check_no_embedded_secret("endpoint_url", &self.endpoint_url)?;

        if self.inference_id.trim().is_empty() {
            return Err(Error::Validation {
                field: "inference_id".to_string(),
                message: "inference_id must not be empty".to_string(),
            });
        }

        let rest = self
            .endpoint_url
            .strip_prefix("http://")
            .or_else(|| self.endpoint_url.strip_prefix("https://"));
        if rest.is_none_or(|r| r.trim().is_empty()) {
            return Err(Error::Validation {
                field: "endpoint_url".to_string(),
                message: "endpoint_url must start with http:// or https://".to_string(),
            });
        }

        match &self.lifecycle {
            InferenceLifecycle::External => {}
            InferenceLifecycle::Container { name } => {
                check_no_embedded_secret("lifecycle.name", name)?;
                if name.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "lifecycle.name".to_string(),
                        message: "container name must not be empty".to_string(),
                    });
                }
            }
            InferenceLifecycle::Compose { file, service } => {
                if let Some(s) = file.to_str() {
                    check_no_embedded_secret("lifecycle.file", s)?;
                }
                if file.as_os_str().is_empty() || file.to_string_lossy().trim().is_empty() {
                    return Err(Error::Validation {
                        field: "lifecycle.file".to_string(),
                        message: "compose file must not be empty".to_string(),
                    });
                }
                if let Some(svc) = service {
                    check_no_embedded_secret("lifecycle.service", svc)?;
                    if svc.trim().is_empty() {
                        return Err(Error::Validation {
                            field: "lifecycle.service".to_string(),
                            message: "compose service must not be empty when set".to_string(),
                        });
                    }
                }
            }
        }

        if self.startup_timeout_secs < 1 {
            return Err(Error::Validation {
                field: "startup_timeout_secs".to_string(),
                message: "startup_timeout_secs must be >= 1".to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceFile {
    #[serde(default)]
    pub inference: Vec<InferenceConfig>,
}

impl InferenceFile {
    pub fn validate(&self) -> Result<()> {
        for inf in &self.inference {
            inf.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum McpTransportConfig {
    Stdio {
        command: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    StreamableHttp {
        endpoint_url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keychain_service: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keychain_account: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteToolSpec {
    pub tool: String,
    pub path_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServerConfig {
    pub server_id: String,
    pub transport: McpTransportConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_tools: Vec<WriteToolSpec>,
}

impl McpServerConfig {
    pub fn validate_project_scope(&self) -> Result<()> {
        if let McpTransportConfig::Stdio { command, args } = &self.transport {
            if command.contains('/') || command.contains('\\') {
                return Err(Error::Validation {
                    field: "transport.command".to_string(),
                    message: "project MCP stdio command must not contain path separators"
                        .to_string(),
                });
            }
            if args.iter().any(|a| Path::new(a).is_absolute()) {
                return Err(Error::Validation {
                    field: "transport.args".to_string(),
                    message: "project MCP stdio args must not contain absolute paths".to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        validate_tool_identifier_part(&self.server_id, "server_id")?;
        if self.server_id.len() + 3 > MAX_MAPPED_TOOL_NAME_LEN {
            return Err(Error::Validation {
                field: "server_id".to_string(),
                message: format!(
                    "server_id '{}' is too long to map as <server>__<tool> within {MAX_MAPPED_TOOL_NAME_LEN} chars",
                    self.server_id
                ),
            });
        }

        match &self.transport {
            McpTransportConfig::Stdio { command, .. } => {
                if command.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "transport.command".to_string(),
                        message: "command must not be empty".to_string(),
                    });
                }
            }
            McpTransportConfig::StreamableHttp {
                endpoint_url,
                keychain_service,
                keychain_account,
            } => {
                if endpoint_url.trim().is_empty() {
                    return Err(Error::Validation {
                        field: "transport.endpoint_url".to_string(),
                        message: "endpoint_url must not be empty".to_string(),
                    });
                }
                match (keychain_service, keychain_account) {
                    (None, None) => {}
                    (Some(svc), Some(acc)) => {
                        if svc.trim().is_empty() || acc.trim().is_empty() {
                            return Err(Error::Validation {
                                field: "transport.keychain_service".to_string(),
                                message: "keychain_service and keychain_account must not be empty"
                                    .to_string(),
                            });
                        }
                    }
                    _ => {
                        return Err(Error::Validation {
                            field: "transport.keychain_service".to_string(),
                            message: "keychain_service and keychain_account must both be set or both omitted"
                                .to_string(),
                        });
                    }
                }
            }
        }

        for (idx, wt) in self.write_tools.iter().enumerate() {
            format_mapped_tool_name(&self.server_id, &wt.tool)?;
            if wt.path_args.iter().any(|arg| arg.trim().is_empty()) {
                return Err(Error::Validation {
                    field: format!("write_tools[{idx}].path_args"),
                    message: "path_args entries must not be empty".to_string(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServersFile {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

impl McpServersFile {
    pub fn validate(&self) -> Result<()> {
        for server in &self.servers {
            server.validate()?;
        }
        Ok(())
    }
}

pub fn format_mapped_tool_name(server_id: &str, tool: &str) -> Result<String> {
    validate_tool_identifier_part(server_id, "server_id")?;
    validate_tool_identifier_part(tool, "tool")?;

    let mapped = format!("{server_id}__{tool}");
    if mapped.is_empty() || mapped.len() > MAX_MAPPED_TOOL_NAME_LEN {
        return Err(Error::Validation {
            field: "tool".to_string(),
            message: format!(
                "mapped tool name '{mapped}' length {} exceeds {MAX_MAPPED_TOOL_NAME_LEN}",
                mapped.len()
            ),
        });
    }

    Ok(mapped)
}

pub fn validate_concrete_tool_ref<'a>(
    tool_ref: &'a str,
    field: &str,
) -> Result<(&'a str, &'a str)> {
    let (server_id, tool) = tool_ref.split_once(':').ok_or_else(|| Error::Validation {
        field: field.to_string(),
        message: format!("expected '<server>:<tool>' in '{tool_ref}'"),
    })?;

    format_mapped_tool_name(server_id, tool).map_err(|err| match err {
        Error::Validation { message, .. } => Error::Validation {
            field: field.to_string(),
            message,
        },
        other => other,
    })?;

    Ok((server_id, tool))
}

pub fn validate_role_tool_pattern(pattern: &str, field: &str) -> Result<()> {
    let (server_id, tool) = pattern.split_once(':').ok_or_else(|| Error::Validation {
        field: field.to_string(),
        message: format!("expected '<server>:<tool>' or '<server>:*' in '{pattern}'"),
    })?;

    if server_id == "*" {
        return Err(Error::Validation {
            field: field.to_string(),
            message: "wildcard server pattern '*:*' is not supported".to_string(),
        });
    }

    if tool == "*" {
        validate_tool_identifier_part(server_id, field)?;
        if server_id.len() + 3 > MAX_MAPPED_TOOL_NAME_LEN {
            return Err(Error::Validation {
                field: field.to_string(),
                message: format!(
                    "server_id '{server_id}' is too long to map as <server>__<tool> within {MAX_MAPPED_TOOL_NAME_LEN} chars"
                ),
            });
        }
        return Ok(());
    }

    validate_concrete_tool_ref(pattern, field)?;
    Ok(())
}

fn is_valid_tool_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn validate_tool_identifier_part(part: &str, field: &str) -> Result<()> {
    if part.is_empty() {
        return Err(Error::Validation {
            field: field.to_string(),
            message: "tool identifier segment must not be empty".to_string(),
        });
    }
    if part.contains("__") {
        return Err(Error::Validation {
            field: field.to_string(),
            message: format!("'{part}' must not contain reserved delimiter '__'"),
        });
    }
    if !part.chars().all(is_valid_tool_char) {
        return Err(Error::Validation {
            field: field.to_string(),
            message: format!("'{part}' contains characters outside [A-Za-z0-9_-]"),
        });
    }
    Ok(())
}

pub(crate) fn validate_safe_relative_path(path: &Path, field: &str) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(Error::Validation {
            field: field.to_string(),
            message: format!("{field} must not be empty"),
        });
    }

    let mut has_normal = false;
    for comp in path.components() {
        match comp {
            Component::Normal(_) => has_normal = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::PathTraversal {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    if !has_normal {
        return Err(Error::Validation {
            field: field.to_string(),
            message: format!("{field} must name a file inside the project"),
        });
    }

    Ok(())
}

fn validate_container_image(image: &str, field: &str) -> Result<()> {
    let trimmed = image.trim();
    if trimmed.is_empty() {
        return Err(Error::Validation {
            field: field.to_string(),
            message: "container image must not be empty".to_string(),
        });
    }

    let last_segment = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if let Some((name, digest)) = last_segment.split_once('@') {
        if name.is_empty() || digest.is_empty() || name.ends_with(":latest") {
            return Err(Error::Validation {
                field: field.to_string(),
                message: format!(
                    "container image '{image}' must specify an exact tag or digest and never 'latest'"
                ),
            });
        }
        return Ok(());
    }

    if let Some((name, tag)) = last_segment.split_once(':')
        && !name.is_empty()
        && !tag.is_empty()
        && tag != "latest"
    {
        return Ok(());
    }

    Err(Error::Validation {
        field: field.to_string(),
        message: format!(
            "container image '{image}' must specify an exact tag or digest and never 'latest'"
        ),
    })
}

fn check_no_embedded_secret(field: &str, value: &str) -> Result<()> {
    for pattern in EMBEDDED_SECRET_PATTERNS {
        if value.contains(pattern) {
            return Err(Error::Security {
                message: format!(
                    "field '{field}' appears to contain an embedded secret matching '{pattern}'"
                ),
            });
        }
    }
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| Error::Validation {
        field: "path".to_string(),
        message: format!("path '{}' has no parent directory", path.display()),
    })?;
    fs::create_dir_all(parent).map_err(|source| Error::Io {
        path: parent.to_path_buf(),
        source,
    })?;

    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("orrery.tmp");
    let pid = std::process::id();

    let (mut tmp_file, tmp_path) = loop {
        let seq = ATOMIC_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{file_name}.tmp.{pid}.{seq}"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (file, candidate),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(Error::Io {
                    path: candidate,
                    source,
                });
            }
        }
    };

    let write_and_sync = (|| -> io::Result<()> {
        tmp_file.write_all(bytes)?;
        tmp_file.flush()?;
        tmp_file.sync_all()?;
        Ok(())
    })();

    drop(tmp_file);

    if let Err(source) = write_and_sync {
        let _ = fs::remove_file(&tmp_path);
        return Err(Error::Io {
            path: tmp_path,
            source,
        });
    }

    if let Err(source) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(Error::Io {
            path: path.to_path_buf(),
            source,
        });
    }

    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }

    Ok(())
}

fn validate_config_file_id(id: &str, field: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(Error::Validation {
            field: field.to_string(),
            message: format!("{field} must not be empty"),
        });
    }
    if id.contains('/') || id.contains('\\') || id == "." || id == ".." || id.contains("..") {
        return Err(Error::PathTraversal {
            path: PathBuf::from(id),
        });
    }
    if id.starts_with('.') {
        return Err(Error::PathTraversal {
            path: PathBuf::from(id),
        });
    }
    Ok(())
}

fn resolve_safe_within_dir(base_dir: &Path, rel: &Path, field: &str) -> Result<PathBuf> {
    validate_safe_relative_path(rel, field)?;

    let canonical_base = fs::canonicalize(base_dir).map_err(|source| Error::Io {
        path: base_dir.to_path_buf(),
        source,
    })?;

    let mut current = canonical_base.clone();
    for comp in rel.components() {
        if let Component::Normal(part) = comp {
            let next = current.join(part);
            match fs::symlink_metadata(&next) {
                Ok(meta) => {
                    let canonical_next = match fs::canonicalize(&next) {
                        Ok(c) => c,
                        Err(source) => {
                            if meta.file_type().is_symlink() {
                                return Err(Error::PathTraversal {
                                    path: rel.to_path_buf(),
                                });
                            }
                            return Err(Error::Io { path: next, source });
                        }
                    };
                    if !canonical_next.starts_with(&canonical_base) {
                        return Err(Error::PathTraversal {
                            path: rel.to_path_buf(),
                        });
                    }
                    current = canonical_next;
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    current = next;
                }
                Err(source) => {
                    return Err(Error::Io { path: next, source });
                }
            }
        }
    }

    if !current.starts_with(&canonical_base) {
        return Err(Error::PathTraversal {
            path: rel.to_path_buf(),
        });
    }

    Ok(current)
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    project_root: PathBuf,
    xdg_base: Option<PathBuf>,
}

impl ConfigStore {
    pub fn new(project_root: impl Into<PathBuf>, xdg_override: Option<PathBuf>) -> Result<Self> {
        let raw_root = project_root.into();
        if raw_root.as_os_str().is_empty() {
            return Err(Error::Validation {
                field: "project_root".to_string(),
                message: "project_root must not be empty".to_string(),
            });
        }

        let project_root = if raw_root.exists() {
            fs::canonicalize(&raw_root).map_err(|source| Error::Io {
                path: raw_root,
                source,
            })?
        } else {
            raw_root
        };

        let xdg_base = match xdg_override {
            Some(dir) => Some(dir),
            None => dirs::config_dir(),
        };

        Ok(Self {
            project_root,
            xdg_base,
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn orrery_dir(&self) -> PathBuf {
        self.project_root.join(".orrery")
    }

    pub fn global_dir(&self) -> Option<PathBuf> {
        let base = self.xdg_base.as_ref()?;
        if base.file_name() == Some(OsStr::new("orrery")) {
            return Some(base.clone());
        }
        let orrery_sub = base.join("orrery");
        if orrery_sub.exists() {
            return Some(orrery_sub);
        }
        if base.join("roles").exists()
            || base.join("agents").exists()
            || base.join("mcp").exists()
            || base.join("inference.toml").exists()
        {
            return Some(base.clone());
        }
        Some(orrery_sub)
    }

    pub fn resolve_safe_path(&self, rel: impl AsRef<Path>) -> Result<PathBuf> {
        resolve_safe_within_dir(&self.project_root, rel.as_ref(), "path")
    }

    pub fn init_project(&self, name: impl Into<String>) -> Result<ProjectConfig> {
        let project_name = name.into();
        let project_cfg = ProjectConfig::default_template(project_name);
        project_cfg.validate_staging()?;

        let orrery_dir = self.orrery_dir();
        for sub in ["roles", "agents", "mcp", "checkpoints", "agent_io", "logs"] {
            let dir = orrery_dir.join(sub);
            fs::create_dir_all(&dir).map_err(|source| Error::Io { path: dir, source })?;
        }

        for (role, rules_md) in default_roles_with_rules() {
            self.save_role(&role, rules_md)?;
        }

        self.save_project(&project_cfg)?;
        Ok(project_cfg)
    }

    pub fn load_project(&self) -> Result<ProjectConfig> {
        let path = self.orrery_dir().join("project.toml");
        self.ensure_file_within_project(&path)?;
        let content = fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let config: ProjectConfig =
            toml::from_str(&content).map_err(|source| Error::TomlDe { path, source })?;
        config.validate_staging()?;
        for inf in &config.inference {
            inf.validate_project_scope()?;
        }
        self.resolve_safe_path(&config.plan_path)?;
        Ok(config)
    }

    pub fn save_project(&self, config: &ProjectConfig) -> Result<()> {
        config.validate_staging()?;
        for inf in &config.inference {
            inf.validate_project_scope()?;
        }
        self.resolve_safe_path(&config.plan_path)?;
        let path = self.orrery_dir().join("project.toml");
        let serialized = toml::to_string_pretty(config)?;
        atomic_write(&path, serialized.as_bytes())
    }

    pub fn load_roles(&self) -> Result<Vec<RoleConfig>> {
        let mut layered: Vec<RoleConfig> = Vec::new();

        if let Some(global_dir) = self.global_dir() {
            let global_roles_dir = global_dir.join("roles");
            for role in Self::read_roles_from_dir(&global_roles_dir)? {
                upsert_by_key(&mut layered, role, |r| &r.role_id);
            }
        }

        let project_roles_dir = self.orrery_dir().join("roles");
        for role in Self::read_roles_from_dir(&project_roles_dir)? {
            upsert_by_key(&mut layered, role, |r| &r.role_id);
        }

        Ok(layered)
    }

    pub fn load_role_rules(&self, role_id: &str) -> Result<String> {
        validate_config_file_id(role_id, "role_id")?;

        let project_roles_dir = self.orrery_dir().join("roles");
        let project_role_toml = project_roles_dir.join(format!("{role_id}.toml"));
        if project_role_toml.exists() {
            let role = Self::read_single_role(&project_roles_dir, &project_role_toml)?;
            let rules_path = resolve_safe_within_dir(
                &project_roles_dir,
                Path::new(&role.rules_file),
                "rules_file",
            )?;
            return fs::read_to_string(&rules_path).map_err(|source| Error::Io {
                path: rules_path,
                source,
            });
        }

        if let Some(global_dir) = self.global_dir() {
            let global_roles_dir = global_dir.join("roles");
            let global_role_toml = global_roles_dir.join(format!("{role_id}.toml"));
            if global_role_toml.exists() {
                let role = Self::read_single_role(&global_roles_dir, &global_role_toml)?;
                let rules_path = resolve_safe_within_dir(
                    &global_roles_dir,
                    Path::new(&role.rules_file),
                    "rules_file",
                )?;
                return fs::read_to_string(&rules_path).map_err(|source| Error::Io {
                    path: rules_path,
                    source,
                });
            }
        }

        Err(Error::Io {
            path: project_role_toml,
            source: io::Error::new(
                io::ErrorKind::NotFound,
                format!("role '{role_id}' not found"),
            ),
        })
    }

    pub fn save_role(&self, role: &RoleConfig, rules_markdown: &str) -> Result<()> {
        validate_config_file_id(&role.role_id, "role_id")?;
        role.validate()?;

        let roles_dir = self.orrery_dir().join("roles");
        fs::create_dir_all(&roles_dir).map_err(|source| Error::Io {
            path: roles_dir.clone(),
            source,
        })?;

        let rules_path =
            resolve_safe_within_dir(&roles_dir, Path::new(&role.rules_file), "rules_file")?;
        atomic_write(&rules_path, rules_markdown.as_bytes())?;

        let role_toml_path = resolve_safe_within_dir(
            &roles_dir,
            Path::new(&format!("{}.toml", role.role_id)),
            "role_id",
        )?;
        let serialized = toml::to_string_pretty(role)?;
        atomic_write(&role_toml_path, serialized.as_bytes())
    }

    pub fn remove_role(&self, role_id: &str) -> Result<()> {
        validate_config_file_id(role_id, "role_id")?;
        let roles_dir = self.orrery_dir().join("roles");
        let role_toml_path = roles_dir.join(format!("{role_id}.toml"));
        if !role_toml_path.exists() {
            return Err(Error::Io {
                path: role_toml_path,
                source: io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("role '{role_id}' not found in project"),
                ),
            });
        }

        let role = Self::read_single_role(&roles_dir, &role_toml_path)?;
        fs::remove_file(&role_toml_path).map_err(|source| Error::Io {
            path: role_toml_path,
            source,
        })?;

        if let Ok(rules_path) =
            resolve_safe_within_dir(&roles_dir, Path::new(&role.rules_file), "rules_file")
            && rules_path.exists()
        {
            let still_used = Self::read_roles_from_dir(&roles_dir)?
                .iter()
                .any(|other| other.rules_file == role.rules_file);
            if !still_used {
                fs::remove_file(&rules_path).map_err(|source| Error::Io {
                    path: rules_path,
                    source,
                })?;
            }
        }

        Ok(())
    }

    pub fn load_agents(&self) -> Result<Vec<AgentConfig>> {
        let mut layered: Vec<AgentConfig> = Vec::new();

        if let Some(global_dir) = self.global_dir() {
            let global_agents_dir = global_dir.join("agents");
            for agent in Self::read_agents_from_dir(&global_agents_dir)? {
                upsert_by_key(&mut layered, agent, |a| &a.agent_id);
            }
        }

        let project_agents_dir = self.orrery_dir().join("agents");
        for agent in Self::read_agents_from_dir(&project_agents_dir)? {
            upsert_by_key(&mut layered, agent, |a| &a.agent_id);
        }

        Ok(layered)
    }

    pub fn save_agent(&self, agent: &AgentConfig) -> Result<()> {
        validate_config_file_id(&agent.agent_id, "agent_id")?;
        agent.validate()?;

        let agents_dir = self.orrery_dir().join("agents");
        fs::create_dir_all(&agents_dir).map_err(|source| Error::Io {
            path: agents_dir.clone(),
            source,
        })?;

        let agent_toml_path = resolve_safe_within_dir(
            &agents_dir,
            Path::new(&format!("{}.toml", agent.agent_id)),
            "agent_id",
        )?;
        let serialized = toml::to_string_pretty(agent)?;
        atomic_write(&agent_toml_path, serialized.as_bytes())
    }

    pub fn remove_agent(&self, agent_id: &str) -> Result<()> {
        validate_config_file_id(agent_id, "agent_id")?;
        let agents_dir = self.orrery_dir().join("agents");
        let agent_toml_path = agents_dir.join(format!("{agent_id}.toml"));
        if !agent_toml_path.exists() {
            return Err(Error::Io {
                path: agent_toml_path,
                source: io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("agent '{agent_id}' not found in project"),
                ),
            });
        }
        self.ensure_file_within_project(&agent_toml_path)?;
        fs::remove_file(&agent_toml_path).map_err(|source| Error::Io {
            path: agent_toml_path,
            source,
        })
    }

    pub fn load_mcp_servers(&self) -> Result<Vec<McpServerConfig>> {
        let mut layered: Vec<McpServerConfig> = Vec::new();

        if let Some(global_dir) = self.global_dir() {
            let global_mcp_path = global_dir.join("mcp").join("servers.toml");
            for server in Self::read_mcp_servers_file(&global_dir, &global_mcp_path)? {
                upsert_by_key(&mut layered, server, |s| &s.server_id);
            }
        }

        let project_orrery = self.orrery_dir();
        let project_mcp_path = project_orrery.join("mcp").join("servers.toml");
        for server in Self::read_mcp_servers_file(&project_orrery, &project_mcp_path)? {
            server.validate_project_scope()?;
            upsert_by_key(&mut layered, server, |s| &s.server_id);
        }

        Ok(layered)
    }

    pub fn save_mcp_server(&self, server: &McpServerConfig) -> Result<()> {
        server.validate()?;
        server.validate_project_scope()?;
        let project_orrery = self.orrery_dir();
        let mcp_dir = project_orrery.join("mcp");
        fs::create_dir_all(&mcp_dir).map_err(|source| Error::Io {
            path: mcp_dir.clone(),
            source,
        })?;
        let servers_path = mcp_dir.join("servers.toml");

        let mut servers = Self::read_mcp_servers_file(&project_orrery, &servers_path)?;
        upsert_by_key(&mut servers, server.clone(), |s| &s.server_id);

        let file = McpServersFile { servers };
        file.validate()?;
        for s in &file.servers {
            s.validate_project_scope()?;
        }
        let serialized = toml::to_string_pretty(&file)?;
        atomic_write(&servers_path, serialized.as_bytes())
    }

    pub fn remove_mcp_server(&self, server_id: &str) -> Result<()> {
        validate_config_file_id(server_id, "server_id")?;
        let project_orrery = self.orrery_dir();
        let servers_path = project_orrery.join("mcp").join("servers.toml");
        if !servers_path.exists() {
            return Err(Error::Io {
                path: servers_path,
                source: io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("MCP server '{server_id}' not found in project"),
                ),
            });
        }

        let mut servers = Self::read_mcp_servers_file(&project_orrery, &servers_path)?;
        let pos = servers
            .iter()
            .position(|s| s.server_id == server_id)
            .ok_or_else(|| Error::Validation {
                field: "server_id".to_string(),
                message: format!("MCP server '{server_id}' not found in project"),
            })?;
        servers.remove(pos);

        let file = McpServersFile { servers };
        let serialized = toml::to_string_pretty(&file)?;
        atomic_write(&servers_path, serialized.as_bytes())
    }

    pub fn load_inference(&self) -> Result<Vec<InferenceConfig>> {
        let mut layered: Vec<InferenceConfig> = Vec::new();
        let global_dir = self.global_dir();

        if let Some(ref gdir) = global_dir {
            let global_inf_path = gdir.join("inference.toml");
            for mut inf in Self::read_inference_file(gdir, &global_inf_path)? {
                if let InferenceLifecycle::Compose { ref mut file, .. } = inf.lifecycle {
                    *file = resolve_compose_file(
                        file,
                        ConfigScope::Machine,
                        &self.project_root,
                        Some(gdir.as_path()),
                    )?;
                }
                upsert_by_key(&mut layered, inf, |i| &i.inference_id);
            }
        }

        let project_orrery = self.orrery_dir();
        let project_inf_path = project_orrery.join("inference.toml");
        for mut inf in Self::read_inference_file(&project_orrery, &project_inf_path)? {
            inf.validate_project_scope()?;
            if let InferenceLifecycle::Compose { ref mut file, .. } = inf.lifecycle {
                *file = resolve_compose_file(
                    file,
                    ConfigScope::Project,
                    &self.project_root,
                    global_dir.as_deref(),
                )?;
            }
            upsert_by_key(&mut layered, inf, |i| &i.inference_id);
        }

        let project_toml_path = project_orrery.join("project.toml");
        if project_toml_path.exists() {
            let project = self.load_project()?;
            for mut inf in project.inference {
                inf.validate_project_scope()?;
                if let InferenceLifecycle::Compose { ref mut file, .. } = inf.lifecycle {
                    *file = resolve_compose_file(
                        file,
                        ConfigScope::Project,
                        &self.project_root,
                        global_dir.as_deref(),
                    )?;
                }
                upsert_by_key(&mut layered, inf, |i| &i.inference_id);
            }
        }

        Ok(layered)
    }

    pub fn save_inference(&self, inference: &InferenceConfig) -> Result<()> {
        validate_config_file_id(&inference.inference_id, "inference_id")?;
        inference.validate()?;
        inference.validate_project_scope()?;

        let project_orrery = self.orrery_dir();
        let project_toml_path = project_orrery.join("project.toml");
        if project_toml_path.exists() {
            let mut project = self.load_project()?;
            upsert_by_key(&mut project.inference, inference.clone(), |i| {
                &i.inference_id
            });
            return self.save_project(&project);
        }

        fs::create_dir_all(&project_orrery).map_err(|source| Error::Io {
            path: project_orrery.clone(),
            source,
        })?;
        let inf_path = project_orrery.join("inference.toml");
        let mut list = Self::read_inference_file(&project_orrery, &inf_path)?;
        upsert_by_key(&mut list, inference.clone(), |i| &i.inference_id);
        let file = InferenceFile { inference: list };
        file.validate()?;
        for inf in &file.inference {
            inf.validate_project_scope()?;
        }
        let serialized = toml::to_string_pretty(&file)?;
        atomic_write(&inf_path, serialized.as_bytes())
    }

    pub fn remove_inference(&self, inference_id: &str) -> Result<()> {
        validate_config_file_id(inference_id, "inference_id")?;
        let project_orrery = self.orrery_dir();
        let project_toml_path = project_orrery.join("project.toml");
        let mut removed = false;

        if project_toml_path.exists() {
            let mut project = self.load_project()?;
            if let Some(pos) = project
                .inference
                .iter()
                .position(|i| i.inference_id == inference_id)
            {
                project.inference.remove(pos);
                self.save_project(&project)?;
                removed = true;
            }
        }

        let inf_path = project_orrery.join("inference.toml");
        if inf_path.exists() {
            let mut list = Self::read_inference_file(&project_orrery, &inf_path)?;
            if let Some(pos) = list.iter().position(|i| i.inference_id == inference_id) {
                list.remove(pos);
                let file = InferenceFile { inference: list };
                let serialized = toml::to_string_pretty(&file)?;
                atomic_write(&inf_path, serialized.as_bytes())?;
                removed = true;
            }
        }

        if !removed {
            return Err(Error::Validation {
                field: "inference_id".to_string(),
                message: format!("inference '{inference_id}' not found in project"),
            });
        }

        Ok(())
    }

    fn ensure_file_within_project(&self, path: &Path) -> Result<()> {
        if let Ok(rel) = path.strip_prefix(&self.project_root) {
            self.resolve_safe_path(rel)?;
        }
        Ok(())
    }

    fn sorted_toml_entries(dir: &Path) -> Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let canonical_dir = fs::canonicalize(dir).map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;

        let mut paths = Vec::new();
        let entries = fs::read_dir(dir).map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;

        for entry_res in entries {
            let entry = entry_res.map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if path.extension() != Some(OsStr::new("toml")) {
                continue;
            }
            let file_name = entry.file_name();
            if file_name.to_string_lossy().starts_with('.') {
                continue;
            }
            let canonical_file = fs::canonicalize(&path).map_err(|source| {
                if fs::symlink_metadata(&path)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    Error::PathTraversal { path: path.clone() }
                } else {
                    Error::Io {
                        path: path.clone(),
                        source,
                    }
                }
            })?;
            if !canonical_file.starts_with(&canonical_dir) {
                return Err(Error::PathTraversal { path });
            }
            paths.push(path);
        }

        paths.sort();
        Ok(paths)
    }

    fn read_single_role(roles_dir: &Path, path: &Path) -> Result<RoleConfig> {
        let file_name = path.file_name().ok_or_else(|| Error::Validation {
            field: "role_id".to_string(),
            message: "invalid role file path".to_string(),
        })?;
        resolve_safe_within_dir(roles_dir, Path::new(file_name), "role_id")?;

        let content = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let role: RoleConfig = toml::from_str(&content).map_err(|source| Error::TomlDe {
            path: path.to_path_buf(),
            source,
        })?;
        role.validate()?;
        let stem = path.file_stem().and_then(|s| s.to_str());
        if stem != Some(role.role_id.as_str()) {
            let stem = stem.unwrap_or_default();
            let id = &role.role_id;
            return Err(Error::Validation {
                field: "role_id".to_string(),
                message: format!(
                    "file '{stem}.toml' declares id '{id}'; the file must be named '{id}.toml'"
                ),
            });
        }

        let rules_path =
            resolve_safe_within_dir(roles_dir, Path::new(&role.rules_file), "rules_file")?;
        if !rules_path.exists() {
            return Err(Error::Io {
                path: rules_path,
                source: io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "rules_file '{}' for role '{}' does not exist",
                        role.rules_file, role.role_id
                    ),
                ),
            });
        }

        Ok(role)
    }

    fn read_roles_from_dir(roles_dir: &Path) -> Result<Vec<RoleConfig>> {
        let mut roles = Vec::new();
        for path in Self::sorted_toml_entries(roles_dir)? {
            let role = Self::read_single_role(roles_dir, &path)?;
            upsert_by_key(&mut roles, role, |r| &r.role_id);
        }
        Ok(roles)
    }

    fn read_agents_from_dir(agents_dir: &Path) -> Result<Vec<AgentConfig>> {
        let mut agents = Vec::new();
        for path in Self::sorted_toml_entries(agents_dir)? {
            let content = fs::read_to_string(&path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
            let agent: AgentConfig = toml::from_str(&content).map_err(|source| Error::TomlDe {
                path: path.clone(),
                source,
            })?;
            agent.validate()?;
            let stem = path.file_stem().and_then(|s| s.to_str());
            if stem != Some(agent.agent_id.as_str()) {
                let stem = stem.unwrap_or_default();
                let id = &agent.agent_id;
                return Err(Error::Validation {
                    field: "agent_id".to_string(),
                    message: format!(
                        "file '{stem}.toml' declares id '{id}'; the file must be named '{id}.toml'"
                    ),
                });
            }
            upsert_by_key(&mut agents, agent, |a| &a.agent_id);
        }
        Ok(agents)
    }

    fn read_mcp_servers_file(base_dir: &Path, path: &Path) -> Result<Vec<McpServerConfig>> {
        if !path.exists() {
            if fs::symlink_metadata(path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return Err(Error::PathTraversal {
                    path: path.to_path_buf(),
                });
            }
            return Ok(Vec::new());
        }
        if let Ok(rel) = path.strip_prefix(base_dir) {
            resolve_safe_within_dir(base_dir, rel, "mcp/servers.toml")?;
        }
        let content = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let file: McpServersFile = toml::from_str(&content).map_err(|source| Error::TomlDe {
            path: path.to_path_buf(),
            source,
        })?;
        file.validate()?;
        Ok(file.servers)
    }

    fn read_inference_file(base_dir: &Path, path: &Path) -> Result<Vec<InferenceConfig>> {
        if !path.exists() {
            if fs::symlink_metadata(path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return Err(Error::PathTraversal {
                    path: path.to_path_buf(),
                });
            }
            return Ok(Vec::new());
        }
        if let Ok(rel) = path.strip_prefix(base_dir) {
            resolve_safe_within_dir(base_dir, rel, "inference.toml")?;
        }
        let content = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let file: InferenceFile = toml::from_str(&content).map_err(|source| Error::TomlDe {
            path: path.to_path_buf(),
            source,
        })?;
        file.validate()?;
        Ok(file.inference)
    }
}

fn upsert_by_key<T, F>(items: &mut Vec<T>, item: T, key_fn: F)
where
    F: Fn(&T) -> &str,
{
    let new_key = key_fn(&item);
    if let Some(existing) = items
        .iter_mut()
        .find(|existing| key_fn(existing) == new_key)
    {
        *existing = item;
    } else {
        items.push(item);
    }
}

fn default_roles_with_rules() -> [(RoleConfig, &'static str); 3] {
    [
        (
            RoleConfig {
                role_id: "orchestrator".to_string(),
                display_name: "Orchestrator".to_string(),
                kind: RoleKind::Orchestrator,
                rules_file: "orchestrator.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            "# Orchestrator Role Rules\n\nCoordinate stage execution and dispatch tasks per the stepped implementation plan.\n",
        ),
        (
            RoleConfig {
                role_id: "coder".to_string(),
                display_name: "Implementation".to_string(),
                kind: RoleKind::Implementer,
                rules_file: "coder.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            "# Implementation Role Rules\n\nImplement the allocated plan step changes within the assigned write scope.\n",
        ),
        (
            RoleConfig {
                role_id: "validator".to_string(),
                display_name: "Validator".to_string(),
                kind: RoleKind::Validator,
                rules_file: "validator.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            "# Validator Role Rules\n\nVerify that all requirements and quality gates for the current plan step are satisfied.\n",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn sample_roles() -> Vec<RoleConfig> {
        vec![
            RoleConfig {
                role_id: "orchestrator".to_string(),
                display_name: "Orchestrator".to_string(),
                kind: RoleKind::Orchestrator,
                rules_file: "orchestrator.md".to_string(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "coder".to_string(),
                display_name: "Implementation".to_string(),
                kind: RoleKind::Implementer,
                rules_file: "coder.md".to_string(),
                allowed_mcp_tools: vec![
                    "filesystem:read_file".to_string(),
                    "filesystem:write_file".to_string(),
                    "cargo:check".to_string(),
                ],
            },
            RoleConfig {
                role_id: "validator".to_string(),
                display_name: "Validator".to_string(),
                kind: RoleKind::Validator,
                rules_file: "validator.md".to_string(),
                allowed_mcp_tools: vec!["cargo:*".to_string()],
            },
        ]
    }

    fn sample_agents() -> Vec<AgentConfig> {
        vec![
            AgentConfig {
                agent_id: "cloud-orchestrator".to_string(),
                display_name: "Cloud Orchestrator".to_string(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "example-orchestrator".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 65536,
                    keychain_service: "orrery.agent.cloud-orchestrator".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            },
            AgentConfig {
                agent_id: "local-coder".to_string(),
                display_name: "Local Coder".to_string(),
                backend: AgentBackend::LocalModel {
                    inference_id: "coder-14b".to_string(),
                    model: "Qwen2.5-Coder-14B-Instruct-Q4_K_M".to_string(),
                    temperature: 0.1,
                    max_context_tokens: 32768,
                },
            },
            AgentConfig {
                agent_id: "cloud-validator".to_string(),
                display_name: "Cloud Validator".to_string(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "example-model".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 65536,
                    keychain_service: "orrery.agent.cloud-validator".to_string(),
                    keychain_account: "testuser".to_string(),
                },
            },
        ]
    }

    #[test]
    fn toml_round_trip_spec_4_1_project_config() {
        let toml_input = r###"
project_name = "my-service"
plan_path = "STEPPED_IMPLEMENTATION_PLAN.md"
step_marker = "## Step "
execution_mode = "Run"
cycle_count = 0
max_concurrent_agents = 2
max_step_retries = 3
validation_mode = "Auto"
gate_tools = []

[[stages]]
role_id = "orchestrator"
agent_ids = ["cloud-orchestrator"]

[[stages]]
role_id = "coder"
agent_ids = ["local-coder"]

[[stages]]
role_id = "validator"
agent_ids = ["cloud-validator"]

[hooks]
on_step_start = { tool = "git:create_branch", on_failure = "Halt" }
on_step_pass  = { tool = "git:commit", on_failure = "Halt" }
on_step_retry = { tool = "git:restore", on_failure = "Warn" }
on_halt       = { tool = "git:stash", on_failure = "Warn" }
"###;

        let parsed: ProjectConfig = toml::from_str(toml_input).unwrap();
        let roles = sample_roles();
        let agents = sample_agents();
        let warnings = parsed.validate_for_run(&roles, &agents).unwrap();
        assert!(warnings.is_empty());

        let serialized = toml::to_string(&parsed).unwrap();
        let reparsed: ProjectConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn toml_round_trip_spec_4_2_role_config() {
        let toml_input = r#"
role_id = "coder"
display_name = "Implementation"
kind = "Implementer"
rules_file = "coder.md"
allowed_mcp_tools = ["filesystem:read_file", "filesystem:write_file", "cargo:check"]
"#;

        let parsed: RoleConfig = toml::from_str(toml_input).unwrap();
        parsed.validate().unwrap();
        assert!(parsed.is_implementer());
        assert!(!parsed.is_orchestrator());
        assert!(!parsed.is_validator());

        let serialized = toml::to_string(&parsed).unwrap();
        let reparsed: RoleConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn toml_round_trip_spec_4_3_agents_and_inference() {
        let local_toml = r#"
agent_id = "local-coder"
display_name = "Local Coder"
[backend]
type = "LocalModel"
inference_id = "coder-14b"
model = "Qwen2.5-Coder-14B-Instruct-Q4_K_M"
temperature = 0.1
max_context_tokens = 32768
"#;
        let local_agent: AgentConfig = toml::from_str(local_toml).unwrap();
        local_agent.validate().unwrap();
        let local_reparsed: AgentConfig =
            toml::from_str(&toml::to_string(&local_agent).unwrap()).unwrap();
        assert_eq!(local_agent, local_reparsed);

        let cli_toml = r#"
agent_id = "cli-validator"
display_name = "CLI Validator"
[backend]
type = "AgentCli"
image = "registry.example.com/agents/claude-code@sha256:<digest>"
command = ["claude", "-p", "--output-format", "stream-json"]
workspace_mount = "ReadOnly"
memory_limit_mb = 4096
timeout_secs = 1800
keychain_service = "orrery.agent.cli-validator"
keychain_account = "testuser"
secret_env = "ANTHROPIC_API_KEY"
"#;
        let cli_agent: AgentConfig = toml::from_str(cli_toml).unwrap();
        cli_agent.validate().unwrap();
        let cli_reparsed: AgentConfig =
            toml::from_str(&toml::to_string(&cli_agent).unwrap()).unwrap();
        assert_eq!(cli_agent, cli_reparsed);

        let cloud_toml = r#"
agent_id = "cloud-validator"
display_name = "Cloud Validator"
[backend]
type = "CloudApi"
endpoint_url = "https://api.example.com/v1/chat/completions"
model = "example-model"
temperature = 0.0
max_context_tokens = 65536
keychain_service = "orrery.agent.cloud-validator"
keychain_account = "testuser"
"#;
        let cloud_agent: AgentConfig = toml::from_str(cloud_toml).unwrap();
        cloud_agent.validate().unwrap();
        let cloud_reparsed: AgentConfig =
            toml::from_str(&toml::to_string(&cloud_agent).unwrap()).unwrap();
        assert_eq!(cloud_agent, cloud_reparsed);

        let inference_toml = r#"
[[inference]]
inference_id = "coder-14b"
endpoint_url = "http://127.0.0.1:8080"
mode = "Single"
lifecycle = { type = "Container", name = "llama-coder-14b" }
unload_after_stage = true
"#;
        let inf_file: InferenceFile = toml::from_str(inference_toml).unwrap();
        inf_file.validate().unwrap();
        let inf_reparsed: InferenceFile =
            toml::from_str(&toml::to_string(&inf_file).unwrap()).unwrap();
        assert_eq!(inf_file, inf_reparsed);
    }

    #[test]
    fn toml_round_trip_spec_4_4_mcp_servers() {
        let mcp_toml = r#"
[[servers]]
server_id = "filesystem"
transport = { type = "Stdio", command = "mcp-filesystem", args = ["--root", "."] }
write_tools = [{ tool = "write_file", path_args = ["path"] }]

[[servers]]
server_id = "remote"
transport = { type = "StreamableHttp", endpoint_url = "https://mcp.example.com/mcp", keychain_service = "orrery.mcp.remote", keychain_account = "testuser" }
"#;

        let parsed: McpServersFile = toml::from_str(mcp_toml).unwrap();
        parsed.validate().unwrap();
        assert_eq!(parsed.servers.len(), 2);

        let serialized = toml::to_string(&parsed).unwrap();
        let reparsed: McpServersFile = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn stage_count_boundaries_1_and_16_rejected_2_and_15_accepted() {
        let roles = sample_roles();
        let agents = sample_agents();
        let mut cfg = ProjectConfig::default_template("boundary-test");
        assert_eq!(cfg.stages.len(), 3);

        // 1 stage via set_stage_count and direct validate -> rejected
        assert!(cfg.set_stage_count(1).is_err());
        let mut one_stage = cfg.clone();
        one_stage.stages.truncate(1);
        assert!(one_stage.validate(&roles, &agents).is_err());

        // 16 stages via set_stage_count and direct validate -> rejected
        assert!(cfg.set_stage_count(16).is_err());
        let mut sixteen_stages = cfg.clone();
        while sixteen_stages.stages.len() < 16 {
            sixteen_stages.stages.push(StageConfig {
                role_id: "coder".to_string(),
                agent_ids: Vec::new(),
            });
        }
        assert!(sixteen_stages.validate(&roles, &agents).is_err());

        // 2 stages -> accepted
        cfg.set_stage_count(2).unwrap();
        assert_eq!(cfg.stages.len(), 2);
        assert_eq!(cfg.stages[0].role_id, "orchestrator");
        assert!(cfg.validate(&roles, &agents).is_ok());

        // 15 stages -> accepted
        cfg.set_stage_count(15).unwrap();
        assert_eq!(cfg.stages.len(), 15);
        assert!(cfg.validate(&roles, &agents).is_ok());
    }

    #[test]
    fn orchestrator_role_placement_and_uniqueness_invariants() {
        let mut roles = sample_roles();
        roles.push(RoleConfig {
            role_id: "lead-orch".to_string(),
            display_name: "Lead Orchestrator".to_string(),
            kind: RoleKind::Orchestrator,
            rules_file: "orchestrator.md".to_string(),
            allowed_mcp_tools: Vec::new(),
        });
        let agents = sample_agents();

        // Orchestrator not at stage 0 (stage 0 is coder, stage 1 is lead-orch) -> rejected
        let mut not_at_zero = ProjectConfig::default_template("orch-test");
        not_at_zero.stages[0].role_id = "coder".to_string();
        not_at_zero.stages[1].role_id = "lead-orch".to_string();
        assert!(not_at_zero.validate(&roles, &agents).is_err());

        // No orchestrator stage at all -> rejected
        let mut no_orch = ProjectConfig::default_template("orch-test");
        no_orch.stages[0].role_id = "coder".to_string();
        assert!(no_orch.validate(&roles, &agents).is_err());

        // Two orchestrator stages (stage 0 and stage 1 both have orchestrator kind) -> rejected
        let mut two_orchs = ProjectConfig::default_template("orch-test");
        two_orchs.stages[1].role_id = "lead-orch".to_string();
        assert!(two_orchs.validate(&roles, &agents).is_err());

        // Setting literal "orchestrator" on stage 1 via set_stage_role -> rejected
        let mut cfg = ProjectConfig::default_template("orch-test");
        assert!(cfg.set_stage_role(1, "orchestrator").is_err());
    }

    #[test]
    fn stage_agent_count_boundaries_and_run_validation() {
        let roles = sample_roles();
        let agents = sample_agents();
        let mut cfg = ProjectConfig::default_template("agent-count-test");

        // Staging allows 0 agents, but validate_for_run rejects 0 agents
        assert!(cfg.validate(&roles, &agents).is_ok());
        assert!(cfg.validate_for_run(&roles, &agents).is_err());

        // Stage 0 (orchestrator) allows 1 agent, rejects a 2nd agent
        cfg.assign_agent(0, "cloud-orchestrator").unwrap();
        assert!(cfg.assign_agent(0, "cloud-orchestrator").is_err());

        // Stage 1 (implementer) accepts up to 15 agents, rejects the 16th
        for _ in 0..15 {
            cfg.assign_agent(1, "local-coder").unwrap();
        }
        assert_eq!(cfg.stages[1].agent_ids.len(), 15);
        assert!(cfg.validate(&roles, &agents).is_ok());
        assert!(cfg.assign_agent(1, "local-coder").is_err());

        // Direct mutation to 16 agents is also rejected by validate
        let mut sixteen_agents = cfg.clone();
        sixteen_agents
            .stages
            .get_mut(1)
            .unwrap()
            .agent_ids
            .push("local-coder".to_string());
        assert!(sixteen_agents.validate(&roles, &agents).is_err());

        // Stage 2 still has 0 agents, so validate_for_run fails until stage 2 gets at least 1 agent
        assert!(cfg.validate_for_run(&roles, &agents).is_err());
        cfg.assign_agent(2, "cloud-validator").unwrap();
        assert!(cfg.validate_for_run(&roles, &agents).is_ok());

        // remove_agent and clear_stage work and maintain invariants
        cfg.remove_agent(1, "local-coder").unwrap();
        assert_eq!(cfg.stages[1].agent_ids.len(), 14);
        assert!(cfg.remove_agent(1, "nonexistent-agent").is_err());
        cfg.clear_stage(1).unwrap();
        assert!(cfg.stages[1].agent_ids.is_empty());
        assert!(cfg.validate(&roles, &agents).is_ok());
        assert!(cfg.validate_for_run(&roles, &agents).is_err());
    }

    #[test]
    fn effective_validation_mode_and_gate_warnings() {
        let roles = sample_roles();
        let agents = sample_agents();

        // 3-stage template includes validator -> Auto resolves to Agents, no warnings
        let cfg_with_validator = ProjectConfig::default_template("val-test");
        assert_eq!(
            cfg_with_validator.effective_validation_mode(&roles),
            ValidationMode::Agents
        );
        let warnings = cfg_with_validator.validate(&roles, &agents).unwrap();
        assert!(warnings.is_empty());

        // 2-stage workflow without validator -> Auto resolves to GateAndOperator
        let mut cfg_without_validator = ProjectConfig::default_template("val-test");
        cfg_without_validator.set_stage_count(2).unwrap();
        assert_eq!(
            cfg_without_validator.effective_validation_mode(&roles),
            ValidationMode::GateAndOperator
        );

        // Empty gate_tools in GateAndOperator emits a warning
        let warnings = cfg_without_validator.validate(&roles, &agents).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].to_string().contains("EMPTY_GATE_TOOLS"));

        // Non-empty gate_tools in GateAndOperator clears the warning
        cfg_without_validator.gate_tools = vec!["cargo:test".to_string()];
        let warnings = cfg_without_validator.validate(&roles, &agents).unwrap();
        assert!(warnings.is_empty());

        // Explicit Agents mode without a validator stage is rejected
        cfg_without_validator.validation_mode = ValidationMode::Agents;
        assert!(cfg_without_validator.validate(&roles, &agents).is_err());
    }

    #[test]
    fn embedded_secrets_rejected_across_agent_backends() {
        let base_local = AgentConfig {
            agent_id: "local-coder".to_string(),
            display_name: "Local Coder".to_string(),
            backend: AgentBackend::LocalModel {
                inference_id: "coder-14b".to_string(),
                model: "Qwen2.5-Coder-14B-Instruct-Q4_K_M".to_string(),
                temperature: 0.1,
                max_context_tokens: 32768,
            },
        };
        base_local.validate_no_embedded_secrets().unwrap();

        for secret in [
            "sk-live-secret-12345",
            "Bearer eyJhbGciOi",
            "AIzaSyDa1234567890",
            "ghp_1234567890abcdef",
            "xoxb-slack-token",
        ] {
            let mut bad_cloud = AgentConfig {
                agent_id: "cloud-1".to_string(),
                display_name: "Cloud".to_string(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                    model: "example-model".to_string(),
                    temperature: 0.0,
                    max_context_tokens: 8192,
                    keychain_service: "orrery.agent.cloud".to_string(),
                    keychain_account: secret.to_string(),
                },
            };
            assert!(
                matches!(
                    bad_cloud.validate_no_embedded_secrets(),
                    Err(Error::Security { .. })
                ),
                "expected embedded secret '{secret}' to be rejected"
            );

            bad_cloud.backend = AgentBackend::AgentCli {
                image: "registry.example.com/agent:1.0.0".to_string(),
                command: vec!["agent".to_string(), format!("--token={secret}")],
                workspace_mount: WorkspaceMount::ReadOnly,
                memory_limit_mb: 1024,
                timeout_secs: 60,
                keychain_service: None,
                keychain_account: None,
                secret_env: None,
            };
            assert!(matches!(
                bad_cloud.validate_no_embedded_secrets(),
                Err(Error::Security { .. })
            ));
        }
    }

    #[test]
    fn tool_id_mapping_and_validation_rules() {
        // Valid mapping
        let mapped = format_mapped_tool_name("filesystem", "read_file").unwrap();
        assert_eq!(mapped, "filesystem__read_file");

        // Exact 64-character boundary: 30 + 2 ("__") + 32 = 64 -> accepted
        let server_30 = "s".repeat(30);
        let tool_32 = "t".repeat(32);
        let mapped_64 = format_mapped_tool_name(&server_30, &tool_32).unwrap();
        assert_eq!(mapped_64.len(), 64);

        // 65-character boundary: 30 + 2 ("__") + 33 = 65 -> rejected
        let tool_33 = "t".repeat(33);
        assert!(format_mapped_tool_name(&server_30, &tool_33).is_err());

        // Double underscore inside server_id or tool is rejected
        assert!(format_mapped_tool_name("bad__srv", "tool").is_err());
        assert!(format_mapped_tool_name("srv", "bad__tool").is_err());

        // Invalid characters rejected
        assert!(format_mapped_tool_name("srv.1", "tool").is_err());
        assert!(format_mapped_tool_name("srv", "tool/name").is_err());

        // Role tool patterns: server:tool and server:* accepted, *:* rejected
        assert!(validate_role_tool_pattern("cargo:check", "allowed").is_ok());
        assert!(validate_role_tool_pattern("cargo:*", "allowed").is_ok());
        assert!(validate_role_tool_pattern("*:*", "allowed").is_err());
        assert!(validate_role_tool_pattern("*:check", "allowed").is_err());

        // Concrete tool refs (gate_tools / hooks) reject wildcards
        assert!(validate_concrete_tool_ref("cargo:*", "gate_tools").is_err());
    }

    #[test]
    fn project_invariants_step_marker_plan_path_concurrency_and_unknown_refs() {
        let roles = sample_roles();
        let agents = sample_agents();

        let mut bad_marker = ProjectConfig::default_template("p");
        bad_marker.step_marker = "   ".to_string();
        assert!(bad_marker.validate(&roles, &agents).is_err());

        let mut bad_path = ProjectConfig::default_template("p");
        bad_path.plan_path = PathBuf::from("../outside.md");
        assert!(matches!(
            bad_path.validate(&roles, &agents),
            Err(Error::PathTraversal { .. })
        ));

        let mut abs_path = ProjectConfig::default_template("p");
        abs_path.plan_path = PathBuf::from("/etc/passwd");
        assert!(matches!(
            abs_path.validate(&roles, &agents),
            Err(Error::PathTraversal { .. })
        ));

        let mut bad_concurrency = ProjectConfig::default_template("p");
        bad_concurrency.max_concurrent_agents = 0;
        assert!(bad_concurrency.validate(&roles, &agents).is_err());

        let mut unknown_role = ProjectConfig::default_template("p");
        unknown_role.stages[1].role_id = "missing-role".to_string();
        assert!(unknown_role.validate(&roles, &agents).is_err());

        let mut unknown_agent = ProjectConfig::default_template("p");
        unknown_agent.stages[1].agent_ids = vec!["missing-agent".to_string()];
        assert!(unknown_agent.validate(&roles, &agents).is_err());
    }

    #[test]
    fn deny_unknown_fields_enforced_on_all_schemas() {
        assert!(
            toml::from_str::<ProjectConfig>(
                r###"
project_name = "x"
plan_path = "PLAN.md"
step_marker = "## Step "
execution_mode = "Run"
cycle_count = 0
max_concurrent_agents = 1
max_step_retries = 1
validation_mode = "Auto"
unexpected_field = true
[[stages]]
role_id = "orchestrator"
[[stages]]
role_id = "coder"
"###
            )
            .is_err()
        );

        assert!(
            toml::from_str::<RoleConfig>(
                r#"
role_id = "coder"
display_name = "Coder"
kind = "Implementer"
rules_file = "coder.md"
extra = "nope"
"#
            )
            .is_err()
        );

        assert!(
            toml::from_str::<AgentConfig>(
                r#"
agent_id = "a"
display_name = "A"
extra = 1
[backend]
type = "LocalModel"
inference_id = "coder-14b"
model = "m"
temperature = 0.0
max_context_tokens = 1024
"#
            )
            .is_err()
        );

        assert!(
            toml::from_str::<McpServersFile>(
                r#"
[[servers]]
server_id = "fs"
transport = { type = "Stdio", command = "mcp-fs" }
unknown = 123
"#
            )
            .is_err()
        );
    }

    #[test]
    fn role_is_writer_detects_write_tools_and_rw_agent_cli() {
        let servers = vec![McpServerConfig {
            server_id: "filesystem".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "mcp-filesystem".to_string(),
                args: Vec::new(),
            },
            write_tools: vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        }];

        let read_only_role = RoleConfig {
            role_id: "reader".to_string(),
            display_name: "Reader".to_string(),
            kind: RoleKind::Validator,
            rules_file: "reader.md".to_string(),
            allowed_mcp_tools: vec!["filesystem:read_file".to_string()],
        };
        assert!(!read_only_role.is_writer(&servers, &[]));

        let exact_writer_role = RoleConfig {
            allowed_mcp_tools: vec!["filesystem:write_file".to_string()],
            ..read_only_role.clone()
        };
        assert!(exact_writer_role.is_writer(&servers, &[]));

        let wildcard_writer_role = RoleConfig {
            allowed_mcp_tools: vec!["filesystem:*".to_string()],
            ..read_only_role.clone()
        };
        assert!(wildcard_writer_role.is_writer(&servers, &[]));

        let rw_cli_agent = AgentConfig {
            agent_id: "cli-rw".to_string(),
            display_name: "CLI RW".to_string(),
            backend: AgentBackend::AgentCli {
                image: "registry.example.com/cli:1.0.0".to_string(),
                command: vec!["cli".to_string()],
                workspace_mount: WorkspaceMount::ReadWrite,
                memory_limit_mb: 2048,
                timeout_secs: 300,
                keychain_service: None,
                keychain_account: None,
                secret_env: None,
            },
        };
        assert!(read_only_role.is_writer(&servers, &[rw_cli_agent]));
    }

    #[test]
    fn hooks_and_inference_validation_edges() {
        let mut hooks = HooksConfig::default();
        assert!(hooks.is_empty());

        let hook = HookConfig {
            tool: "git:commit".to_string(),
            on_failure: HookFailure::Halt,
        };
        for event in [
            HookEvent::StepStart,
            HookEvent::StepPass,
            HookEvent::StepRetry,
            HookEvent::Halt,
        ] {
            hooks.set(event, hook.clone());
            assert_eq!(hooks.get(event), Some(&hook));
            hooks.validate().unwrap();
            hooks.clear(event);
            assert!(hooks.get(event).is_none());
        }

        let mut inf = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: "llama-coder-14b".to_string(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        inf.validate().unwrap();

        inf.inference_id = String::new();
        assert!(inf.validate().is_err());
        inf.inference_id = "coder-14b".to_string();

        inf.startup_timeout_secs = 0;
        assert!(inf.validate().is_err());
        inf.startup_timeout_secs = 300;
    }

    #[test]
    fn inference_defaults() {
        let toml_input = r#"
inference_id = "coder-14b"
endpoint_url = "http://127.0.0.1:8080"
"#;
        let inf: InferenceConfig = toml::from_str(toml_input).unwrap();
        assert_eq!(inf.mode, InferenceMode::Single);
        assert_eq!(inf.lifecycle, InferenceLifecycle::External);
        assert!(inf.unload_after_stage);
        assert_eq!(inf.startup_timeout_secs, 300);
        inf.validate().unwrap();
    }

    #[test]
    fn inference_spec_examples_round_trip() {
        let toml_input = r#"
# Single-model llama-server in an existing container
[[inference]]
inference_id = "coder-14b"
endpoint_url = "http://127.0.0.1:8080"
mode = "Single"
lifecycle = { type = "Container", name = "llama-coder-14b" }
unload_after_stage = true

# Router-mode llama-server defined in a compose file
[[inference]]
inference_id = "router"
endpoint_url = "http://127.0.0.1:8081"
mode = "Router"
lifecycle = { type = "Compose", file = "docker-models/router/compose.yaml", service = "llama" }

# Already running; Orrery only talks HTTP
[[inference]]
inference_id = "always-on"
endpoint_url = "http://127.0.0.1:8082"
lifecycle = { type = "External" }
"#;
        let parsed: InferenceFile = toml::from_str(toml_input).unwrap();
        parsed.validate().unwrap();
        assert_eq!(parsed.inference.len(), 3);
        assert_eq!(parsed.inference[0].inference_id, "coder-14b");
        assert_eq!(parsed.inference[0].mode, InferenceMode::Single);
        assert_eq!(
            parsed.inference[0].lifecycle,
            InferenceLifecycle::Container {
                name: "llama-coder-14b".to_string()
            }
        );
        assert_eq!(parsed.inference[1].inference_id, "router");
        assert_eq!(parsed.inference[1].mode, InferenceMode::Router);
        assert_eq!(
            parsed.inference[1].lifecycle,
            InferenceLifecycle::Compose {
                file: PathBuf::from("docker-models/router/compose.yaml"),
                service: Some("llama".to_string()),
            }
        );
        assert_eq!(parsed.inference[2].inference_id, "always-on");
        assert_eq!(parsed.inference[2].mode, InferenceMode::Single);
        assert_eq!(parsed.inference[2].lifecycle, InferenceLifecycle::External);

        let serialized = toml::to_string(&parsed).unwrap();
        let reparsed: InferenceFile = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn inference_chat_url_trims_slash() {
        let with_slash = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080/".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        assert_eq!(
            with_slash.chat_url(),
            "http://127.0.0.1:8080/v1/chat/completions"
        );

        let without_slash = InferenceConfig {
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            ..with_slash
        };
        assert_eq!(
            without_slash.chat_url(),
            "http://127.0.0.1:8080/v1/chat/completions"
        );
    }

    #[test]
    fn inference_rejects_bad_endpoint() {
        for bad_endpoint in ["127.0.0.1:8080", "ftp://x", "http://", "https://", ""] {
            let inf = InferenceConfig {
                inference_id: "coder-14b".to_string(),
                endpoint_url: bad_endpoint.to_string(),
                mode: InferenceMode::Single,
                lifecycle: InferenceLifecycle::External,
                unload_after_stage: true,
                startup_timeout_secs: 300,
            };
            assert!(
                matches!(inf.validate(), Err(Error::Validation { .. })),
                "expected endpoint '{bad_endpoint}' to be rejected"
            );
        }
    }

    #[test]
    fn inference_rejects_empty_container_and_compose() {
        let empty_container = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: String::new(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        assert!(matches!(
            empty_container.validate(),
            Err(Error::Validation { .. })
        ));

        let empty_compose = InferenceConfig {
            lifecycle: InferenceLifecycle::Compose {
                file: PathBuf::from(""),
                service: None,
            },
            ..empty_container.clone()
        };
        assert!(matches!(
            empty_compose.validate(),
            Err(Error::Validation { .. })
        ));

        let empty_compose_service = InferenceConfig {
            lifecycle: InferenceLifecycle::Compose {
                file: PathBuf::from("docker-models/router/compose.yaml"),
                service: Some("   ".to_string()),
            },
            ..empty_container
        };
        assert!(matches!(
            empty_compose_service.validate(),
            Err(Error::Validation { .. })
        ));
    }

    #[test]
    fn inference_rejects_removed_fields() {
        for removed_line in [
            r#"image = "registry.example.com/llama-server:v0.9.0""#,
            r#"gpu = "All""#,
            r#"host_port = 8080"#,
            concat!("models_", r#"volume = "models""#),
        ] {
            let toml_input = format!(
                "inference_id = \"coder-14b\"\nendpoint_url = \"http://127.0.0.1:8080\"\n{removed_line}\n"
            );
            assert!(
                toml::from_str::<InferenceConfig>(&toml_input).is_err(),
                "expected removed field line '{removed_line}' to fail deserialization"
            );
        }
    }

    #[test]
    fn inference_rejects_embedded_secret_in_endpoint() {
        let inf = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080?key=sk-ABCDEFGH12345678".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        assert!(matches!(inf.validate(), Err(Error::Security { .. })));
    }

    #[test]
    fn config_store_init_project_layout_matches_spec_4() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();

        let created_cfg = store.init_project("demo-service").unwrap();
        assert_eq!(created_cfg.project_name, "demo-service");
        assert_eq!(created_cfg.stages.len(), 3);

        let orrery_dir = store.orrery_dir();
        assert!(orrery_dir.join("project.toml").is_file());
        for sub in ["roles", "agents", "mcp", "checkpoints", "agent_io", "logs"] {
            assert!(
                orrery_dir.join(sub).is_dir(),
                "expected directory .orrery/{sub}"
            );
        }

        for role_id in ["orchestrator", "coder", "validator"] {
            assert!(
                orrery_dir
                    .join("roles")
                    .join(format!("{role_id}.toml"))
                    .is_file()
            );
            assert!(
                orrery_dir
                    .join("roles")
                    .join(format!("{role_id}.md"))
                    .is_file()
            );
            let rules_content = store.load_role_rules(role_id).unwrap();
            assert!(!rules_content.trim().is_empty());
        }

        let loaded_proj = store.load_project().unwrap();
        assert_eq!(loaded_proj, created_cfg);

        let loaded_roles = store.load_roles().unwrap();
        assert_eq!(loaded_roles.len(), 3);
        let warnings = loaded_proj.validate(&loaded_roles, &[]).unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn config_store_missing_optional_files_load_as_empty() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();

        assert!(store.load_roles().unwrap().is_empty());
        assert!(store.load_agents().unwrap().is_empty());
        assert!(store.load_mcp_servers().unwrap().is_empty());
        assert!(store.load_inference().unwrap().is_empty());
    }

    #[test]
    fn config_store_layering_global_first_project_replaces_by_id() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();

        let global_dir = store.global_dir().unwrap();
        let global_roles_dir = global_dir.join("roles");
        let global_agents_dir = global_dir.join("agents");
        let global_mcp_dir = global_dir.join("mcp");
        fs::create_dir_all(&global_roles_dir).unwrap();
        fs::create_dir_all(&global_agents_dir).unwrap();
        fs::create_dir_all(&global_mcp_dir).unwrap();

        // Global roles: "coder" (with global tools) and "auditor" (global-only)
        let global_coder = RoleConfig {
            role_id: "coder".to_string(),
            display_name: "Global Coder".to_string(),
            kind: RoleKind::Implementer,
            rules_file: "coder.md".to_string(),
            allowed_mcp_tools: vec!["filesystem:read_file".to_string()],
        };
        let global_auditor = RoleConfig {
            role_id: "auditor".to_string(),
            display_name: "Global Auditor".to_string(),
            kind: RoleKind::Validator,
            rules_file: "auditor.md".to_string(),
            allowed_mcp_tools: vec!["cargo:clippy".to_string()],
        };
        atomic_write(
            &global_roles_dir.join("coder.toml"),
            toml::to_string_pretty(&global_coder).unwrap().as_bytes(),
        )
        .unwrap();
        atomic_write(&global_roles_dir.join("coder.md"), b"global coder rules").unwrap();
        atomic_write(
            &global_roles_dir.join("auditor.toml"),
            toml::to_string_pretty(&global_auditor).unwrap().as_bytes(),
        )
        .unwrap();
        atomic_write(
            &global_roles_dir.join("auditor.md"),
            b"global auditor rules",
        )
        .unwrap();

        // Project overrides "coder" (different display_name, allowed_mcp_tools empty -> replaces, does NOT merge!)
        let project_coder = RoleConfig {
            role_id: "coder".to_string(),
            display_name: "Project Coder".to_string(),
            kind: RoleKind::Implementer,
            rules_file: "coder.md".to_string(),
            allowed_mcp_tools: vec!["cargo:check".to_string()],
        };
        store
            .save_role(&project_coder, "project coder rules")
            .unwrap();

        let roles = store.load_roles().unwrap();
        assert_eq!(roles.len(), 2);
        assert_eq!(roles[0].role_id, "auditor");
        assert_eq!(roles[0].display_name, "Global Auditor");
        assert_eq!(roles[1].role_id, "coder");
        assert_eq!(roles[1].display_name, "Project Coder");
        assert_eq!(roles[1].allowed_mcp_tools, vec!["cargo:check".to_string()]);
        assert_eq!(
            store.load_role_rules("coder").unwrap(),
            "project coder rules"
        );
        assert_eq!(
            store.load_role_rules("auditor").unwrap(),
            "global auditor rules"
        );

        // Global agents: "shared-agent" and "global-only-agent"
        let global_shared_agent = AgentConfig {
            agent_id: "shared-agent".to_string(),
            display_name: "Global Shared".to_string(),
            backend: AgentBackend::LocalModel {
                inference_id: "coder-14b".to_string(),
                model: "Qwen2.5-Coder-7B-Instruct-Q4_K_M".to_string(),
                temperature: 0.2,
                max_context_tokens: 16384,
            },
        };
        let global_only_agent = AgentConfig {
            agent_id: "global-only-agent".to_string(),
            display_name: "Global Only".to_string(),
            backend: AgentBackend::LocalModel {
                inference_id: "coder-14b".to_string(),
                model: "Qwen2.5-Coder-14B-Instruct-Q4_K_M".to_string(),
                temperature: 0.1,
                max_context_tokens: 32768,
            },
        };
        atomic_write(
            &global_agents_dir.join("shared-agent.toml"),
            toml::to_string_pretty(&global_shared_agent)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        atomic_write(
            &global_agents_dir.join("global-only-agent.toml"),
            toml::to_string_pretty(&global_only_agent)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

        // Project overrides "shared-agent" with CloudApi backend
        let project_shared_agent = AgentConfig {
            agent_id: "shared-agent".to_string(),
            display_name: "Project Shared Cloud".to_string(),
            backend: AgentBackend::CloudApi {
                endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                model: "example-pro".to_string(),
                temperature: 0.0,
                max_context_tokens: 65536,
                keychain_service: "orrery.agent.shared".to_string(),
                keychain_account: "testuser".to_string(),
            },
        };
        store.save_agent(&project_shared_agent).unwrap();

        let agents = store.load_agents().unwrap();
        assert_eq!(agents.len(), 2);
        let resolved_shared = agents
            .iter()
            .find(|a| a.agent_id == "shared-agent")
            .unwrap();
        assert_eq!(resolved_shared, &project_shared_agent);
        assert!(agents.iter().any(|a| a.agent_id == "global-only-agent"));

        // Global MCP servers: "filesystem" and "git"
        let global_mcp = McpServersFile {
            servers: vec![
                McpServerConfig {
                    server_id: "filesystem".to_string(),
                    transport: McpTransportConfig::Stdio {
                        command: "global-mcp-fs".to_string(),
                        args: vec!["--readonly".to_string()],
                    },
                    write_tools: Vec::new(),
                },
                McpServerConfig {
                    server_id: "git".to_string(),
                    transport: McpTransportConfig::Stdio {
                        command: "mcp-git".to_string(),
                        args: Vec::new(),
                    },
                    write_tools: Vec::new(),
                },
            ],
        };
        atomic_write(
            &global_mcp_dir.join("servers.toml"),
            toml::to_string_pretty(&global_mcp).unwrap().as_bytes(),
        )
        .unwrap();

        // Project overrides "filesystem" and adds "cargo"
        let project_fs = McpServerConfig {
            server_id: "filesystem".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "project-mcp-fs".to_string(),
                args: vec!["--root".to_string(), ".".to_string()],
            },
            write_tools: vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        };
        let project_cargo = McpServerConfig {
            server_id: "cargo".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "mcp-cargo".to_string(),
                args: Vec::new(),
            },
            write_tools: Vec::new(),
        };
        store.save_mcp_server(&project_fs).unwrap();
        store.save_mcp_server(&project_cargo).unwrap();

        let mcp_servers = store.load_mcp_servers().unwrap();
        assert_eq!(mcp_servers.len(), 3);
        assert_eq!(mcp_servers[0], project_fs);
        assert_eq!(mcp_servers[1].server_id, "git");
        assert_eq!(mcp_servers[2], project_cargo);

        // Global inference: "coder-14b" (port 8080) and "router" (port 8081)
        let router_compose_path = global_dir
            .join("docker-models")
            .join("router")
            .join("compose.yaml");
        atomic_write(&router_compose_path, b"services:\n  llama:\n    image: x\n").unwrap();
        let canonical_router_compose = fs::canonicalize(&router_compose_path).unwrap();

        let global_inf = InferenceFile {
            inference: vec![
                InferenceConfig {
                    inference_id: "coder-14b".to_string(),
                    endpoint_url: "http://127.0.0.1:8080".to_string(),
                    mode: InferenceMode::Single,
                    lifecycle: InferenceLifecycle::Container {
                        name: "llama-coder-14b".to_string(),
                    },
                    unload_after_stage: true,
                    startup_timeout_secs: 300,
                },
                InferenceConfig {
                    inference_id: "router".to_string(),
                    endpoint_url: "http://127.0.0.1:8081".to_string(),
                    mode: InferenceMode::Router,
                    lifecycle: InferenceLifecycle::Compose {
                        file: PathBuf::from("docker-models/router/compose.yaml"),
                        service: Some("llama".to_string()),
                    },
                    unload_after_stage: true,
                    startup_timeout_secs: 300,
                },
            ],
        };
        atomic_write(
            &global_dir.join("inference.toml"),
            toml::to_string_pretty(&global_inf).unwrap().as_bytes(),
        )
        .unwrap();

        // Project overrides "coder-14b" via save_inference
        let project_coder_inf = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:18080".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: "project-llama-coder-14b".to_string(),
            },
            unload_after_stage: false,
            startup_timeout_secs: 120,
        };
        store.save_inference(&project_coder_inf).unwrap();

        let loaded_inf = store.load_inference().unwrap();
        assert_eq!(loaded_inf.len(), 2);
        assert_eq!(loaded_inf[0], project_coder_inf);
        assert_eq!(loaded_inf[1].inference_id, "router");
        assert_eq!(
            loaded_inf[1].lifecycle,
            InferenceLifecycle::Compose {
                file: canonical_router_compose,
                service: Some("llama".to_string()),
            }
        );
    }

    #[test]
    fn config_store_rejects_parent_traversal_and_symlink_escapes() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let outside_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("symlink-test").unwrap();

        // Safe relative path (existing and not-yet-existing) succeeds
        let resolved_plan = store
            .resolve_safe_path("STEPPED_IMPLEMENTATION_PLAN.md")
            .unwrap();
        assert!(resolved_plan.starts_with(store.project_root()));
        let resolved_nested = store.resolve_safe_path("docs/sub/plan.md").unwrap();
        assert!(resolved_nested.starts_with(store.project_root()));

        // Parent traversal `..` and absolute paths rejected
        assert!(matches!(
            store.resolve_safe_path("../outside.md"),
            Err(Error::PathTraversal { .. })
        ));
        assert!(matches!(
            store.resolve_safe_path("docs/../../outside.md"),
            Err(Error::PathTraversal { .. })
        ));
        assert!(matches!(
            store.resolve_safe_path("/etc/passwd"),
            Err(Error::PathTraversal { .. })
        ));

        // Direct file symlink escaping project_root
        let outside_file = outside_tmp.path().join("secret_plan.md");
        fs::write(&outside_file, "# Outside Plan").unwrap();
        let escape_link = proj_tmp.path().join("escape_plan.md");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_file, &escape_link).unwrap();

        assert!(matches!(
            store.resolve_safe_path("escape_plan.md"),
            Err(Error::PathTraversal { .. })
        ));

        // Directory symlink escaping project_root (even for a non-existent child file)
        let escape_dir_link = proj_tmp.path().join("linked_dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside_tmp.path(), &escape_dir_link).unwrap();

        assert!(matches!(
            store.resolve_safe_path("linked_dir/future_file.md"),
            Err(Error::PathTraversal { .. })
        ));

        // Dangling symlink rejected
        let dangling_link = proj_tmp.path().join("dangling.md");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside_tmp.path().join("does_not_exist.md"), &dangling_link)
            .unwrap();

        assert!(matches!(
            store.resolve_safe_path("dangling.md"),
            Err(Error::PathTraversal { .. })
        ));

        // Symlinked agent .toml inside .orrery/agents/ pointing outside is rejected on load_agents
        let outside_agent = outside_tmp.path().join("rogue.toml");
        fs::write(
            &outside_agent,
            r#"
agent_id = "rogue"
display_name = "Rogue"
[backend]
type = "LocalModel"
inference_id = "coder-14b"
model = "m"
temperature = 0.0
max_context_tokens = 1024
"#,
        )
        .unwrap();
        let agent_link = store.orrery_dir().join("agents").join("rogue.toml");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_agent, &agent_link).unwrap();
        assert!(matches!(
            store.load_agents(),
            Err(Error::PathTraversal { .. })
        ));
    }

    #[test]
    fn config_store_atomic_save_leaves_no_temp_files_and_supports_remove() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("atomic-test").unwrap();

        let agent = sample_agents().into_iter().next().unwrap();
        store.save_agent(&agent).unwrap();

        let mcp = McpServerConfig {
            server_id: "filesystem".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "mcp-filesystem".to_string(),
                args: Vec::new(),
            },
            write_tools: Vec::new(),
        };
        store.save_mcp_server(&mcp).unwrap();

        let inf = InferenceConfig {
            inference_id: "coder-14b".to_string(),
            endpoint_url: "http://127.0.0.1:8080".to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::Container {
                name: "llama-coder-14b".to_string(),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        store.save_inference(&inf).unwrap();

        // Check that no temporary files remain in any .orrery subdirectory
        for dir in [
            store.orrery_dir(),
            store.orrery_dir().join("roles"),
            store.orrery_dir().join("agents"),
            store.orrery_dir().join("mcp"),
        ] {
            for entry in fs::read_dir(&dir).unwrap() {
                let name = entry.unwrap().file_name().to_string_lossy().into_owned();
                assert!(
                    !name.contains(".tmp."),
                    "unexpected leftover temp file '{name}' in {}",
                    dir.display()
                );
            }
        }

        // Remove agent, role, MCP server, and inference; verify removal and error on second remove
        store.remove_agent(&agent.agent_id).unwrap();
        assert!(store.load_agents().unwrap().is_empty());
        assert!(store.remove_agent(&agent.agent_id).is_err());

        store.remove_role("validator").unwrap();
        assert_eq!(store.load_roles().unwrap().len(), 2);
        assert!(
            !store
                .orrery_dir()
                .join("roles")
                .join("validator.md")
                .exists()
        );
        assert!(store.remove_role("validator").is_err());

        store.remove_mcp_server("filesystem").unwrap();
        assert!(store.load_mcp_servers().unwrap().is_empty());
        assert!(store.remove_mcp_server("filesystem").is_err());

        store.remove_inference("coder-14b").unwrap();
        assert!(store.load_inference().unwrap().is_empty());
        assert!(store.remove_inference("coder-14b").is_err());
    }

    #[test]
    fn project_compose_resolves_under_orrery() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("compose-proj").unwrap();

        let compose_path = store.orrery_dir().join("compose").join("router.yaml");
        atomic_write(&compose_path, b"services:\n  llama:\n    image: x\n").unwrap();
        let expected_canonical = fs::canonicalize(&compose_path).unwrap();

        let resolved = resolve_compose_file(
            Path::new("compose/router.yaml"),
            ConfigScope::Project,
            proj_tmp.path(),
            None,
        )
        .unwrap();
        assert_eq!(resolved, expected_canonical);

        let router_inf = InferenceConfig {
            inference_id: "router".to_string(),
            endpoint_url: "http://127.0.0.1:8081".to_string(),
            mode: InferenceMode::Router,
            lifecycle: InferenceLifecycle::Compose {
                file: PathBuf::from("compose/router.yaml"),
                service: Some("llama".to_string()),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        store.save_inference(&router_inf).unwrap();

        let loaded = store.load_inference().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].lifecycle,
            InferenceLifecycle::Compose {
                file: expected_canonical,
                service: Some("llama".to_string()),
            }
        );

        // Saved project.toml still holds the relative path, never the canonical path
        let saved_proj = store.load_project().unwrap();
        assert_eq!(
            saved_proj.inference[0].lifecycle,
            InferenceLifecycle::Compose {
                file: PathBuf::from("compose/router.yaml"),
                service: Some("llama".to_string()),
            }
        );
    }

    #[test]
    fn project_compose_absolute_rejected() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let outside_tmp = tempfile::TempDir::new().unwrap();
        let abs_compose = outside_tmp.path().join("compose.yaml");
        fs::write(&abs_compose, "services: {}\n").unwrap();

        let err = resolve_compose_file(&abs_compose, ConfigScope::Project, proj_tmp.path(), None)
            .unwrap_err();
        match err {
            Error::Validation { field, message } => {
                assert_eq!(field, "lifecycle.file");
                assert_eq!(
                    message,
                    "project config must use a path relative to .orrery/"
                );
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }

        let inf = InferenceConfig {
            inference_id: "router".to_string(),
            endpoint_url: "http://127.0.0.1:8081".to_string(),
            mode: InferenceMode::Router,
            lifecycle: InferenceLifecycle::Compose {
                file: abs_compose,
                service: Some("llama".to_string()),
            },
            unload_after_stage: true,
            startup_timeout_secs: 300,
        };
        let scope_err = inf.validate_project_scope().unwrap_err();
        match scope_err {
            Error::Validation { field, message } => {
                assert_eq!(field, "lifecycle.file");
                assert_eq!(
                    message,
                    "project config must use a path relative to .orrery/"
                );
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }

        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("abs-compose-proj").unwrap();
        assert!(matches!(
            store.save_inference(&inf),
            Err(Error::Validation { ref field, .. }) if field == "lifecycle.file"
        ));
        let mut proj = store.load_project().unwrap();
        proj.inference.push(inf);
        assert!(matches!(
            store.save_project(&proj),
            Err(Error::Validation { ref field, .. }) if field == "lifecycle.file"
        ));
    }

    #[test]
    fn project_compose_escape_rejected() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(proj_tmp.path().join(".orrery")).unwrap();

        let err = resolve_compose_file(
            Path::new("../outside.yaml"),
            ConfigScope::Project,
            proj_tmp.path(),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, Error::PathTraversal { .. }));
    }

    #[test]
    fn machine_compose_absolute_outside_workspace_allowed() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let outside_tmp = tempfile::TempDir::new().unwrap();
        let abs_compose = outside_tmp.path().join("compose.yaml");
        fs::write(&abs_compose, "services: {}\n").unwrap();
        let expected = fs::canonicalize(&abs_compose).unwrap();

        let resolved =
            resolve_compose_file(&abs_compose, ConfigScope::Machine, proj_tmp.path(), None)
                .unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn compose_inside_workspace_rejected() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let workspace_compose = proj_tmp.path().join("compose.yaml");
        fs::write(&workspace_compose, "services: {}\n").unwrap();

        let err = resolve_compose_file(
            &workspace_compose,
            ConfigScope::Machine,
            proj_tmp.path(),
            None,
        )
        .unwrap_err();
        match err {
            Error::Security { message } => {
                assert_eq!(
                    message,
                    "compose file is inside the agent-writable workspace"
                );
            }
            other => panic!("expected Error::Security, got {other:?}"),
        }

        let orrery_compose = proj_tmp.path().join(".orrery").join("compose.yaml");
        atomic_write(&orrery_compose, b"services: {}\n").unwrap();
        let expected = fs::canonicalize(&orrery_compose).unwrap();

        let ok = resolve_compose_file(&orrery_compose, ConfigScope::Machine, proj_tmp.path(), None)
            .unwrap();
        assert_eq!(ok, expected);
    }

    #[test]
    fn compose_missing_file_is_io_error() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let outside_tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(proj_tmp.path().join(".orrery")).unwrap();

        let err_proj = resolve_compose_file(
            Path::new("missing.yaml"),
            ConfigScope::Project,
            proj_tmp.path(),
            None,
        )
        .unwrap_err();
        assert!(matches!(err_proj, Error::Io { .. }));

        let err_machine = resolve_compose_file(
            &outside_tmp.path().join("missing.yaml"),
            ConfigScope::Machine,
            proj_tmp.path(),
            None,
        )
        .unwrap_err();
        assert!(matches!(err_machine, Error::Io { .. }));
    }

    #[test]
    fn project_stdio_command_with_separator_rejected() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();

        for bad_cmd in ["bin/server", "bin\\server"] {
            let server = McpServerConfig {
                server_id: "filesystem".to_string(),
                transport: McpTransportConfig::Stdio {
                    command: bad_cmd.to_string(),
                    args: Vec::new(),
                },
                write_tools: Vec::new(),
            };
            let err = store.save_mcp_server(&server).unwrap_err();
            match err {
                Error::Validation { field, .. } => assert_eq!(field, "transport.command"),
                other => panic!("expected Error::Validation, got {other:?}"),
            }
        }
        assert!(!store.orrery_dir().join("mcp").join("servers.toml").exists());
    }

    #[test]
    fn project_stdio_absolute_arg_rejected() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let arg_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();

        let abs_arg = arg_tmp.path().to_string_lossy().into_owned();
        let file = McpServersFile {
            servers: vec![McpServerConfig {
                server_id: "filesystem".to_string(),
                transport: McpTransportConfig::Stdio {
                    command: "mcp-filesystem".to_string(),
                    args: vec!["--root".to_string(), abs_arg],
                },
                write_tools: Vec::new(),
            }],
        };
        let servers_toml = store.orrery_dir().join("mcp").join("servers.toml");
        atomic_write(
            &servers_toml,
            toml::to_string_pretty(&file).unwrap().as_bytes(),
        )
        .unwrap();

        let err = store.load_mcp_servers().unwrap_err();
        match err {
            Error::Validation { field, .. } => assert_eq!(field, "transport.args"),
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[test]
    fn limits_default_when_omitted() {
        let project = ProjectConfig::default_template("p");
        let serialized = toml::to_string(&project).unwrap();
        assert!(!serialized.contains("[limits]"));

        let parsed: ProjectConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed.limits, RuntimeLimits::default());
        assert!(parsed.limits.is_default());
    }

    #[test]
    fn limits_round_trip() {
        let mut project = ProjectConfig::default_template("p");
        project.limits.max_tool_calls = 7;
        project.limits.model_idle_timeout_secs = 42;
        assert!(!project.limits.is_default());

        let serialized = toml::to_string(&project).unwrap();
        assert!(serialized.contains("[limits]"));

        let parsed: ProjectConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed, project);
    }

    #[test]
    fn limits_validation_bounds() {
        fn check_invalid(limits: RuntimeLimits, expected_field: &str) {
            match limits.validate().unwrap_err() {
                Error::Validation { field, .. } => assert_eq!(field, expected_field),
                other => panic!("expected Error::Validation, got {other:?}"),
            }
        }

        assert!(RuntimeLimits::default().validate().is_ok());

        check_invalid(
            RuntimeLimits {
                max_tool_calls: 0,
                ..RuntimeLimits::default()
            },
            "limits.max_tool_calls",
        );
        check_invalid(
            RuntimeLimits {
                max_tool_calls: 1001,
                ..RuntimeLimits::default()
            },
            "limits.max_tool_calls",
        );
        check_invalid(
            RuntimeLimits {
                model_idle_timeout_secs: 0,
                ..RuntimeLimits::default()
            },
            "limits.model_idle_timeout_secs",
        );
        check_invalid(
            RuntimeLimits {
                model_idle_timeout_secs: 86_401,
                ..RuntimeLimits::default()
            },
            "limits.model_idle_timeout_secs",
        );
        check_invalid(
            RuntimeLimits {
                live_stream_ring_capacity: 0,
                ..RuntimeLimits::default()
            },
            "limits.live_stream_ring_capacity",
        );
        check_invalid(
            RuntimeLimits {
                live_stream_max_file_bytes: 4095,
                ..RuntimeLimits::default()
            },
            "limits.live_stream_max_file_bytes",
        );
        check_invalid(
            RuntimeLimits {
                live_stream_max_files: 0,
                ..RuntimeLimits::default()
            },
            "limits.live_stream_max_files",
        );
        check_invalid(
            RuntimeLimits {
                live_stream_max_files: 11,
                ..RuntimeLimits::default()
            },
            "limits.live_stream_max_files",
        );

        let mut project = ProjectConfig::default_template("p");
        project.limits.live_stream_max_files = 11;
        match project.validate_staging().unwrap_err() {
            Error::Validation { field, .. } => assert_eq!(field, "limits.live_stream_max_files"),
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[test]
    fn limits_unknown_key_rejected() {
        let base = toml::to_string(&ProjectConfig::default_template("p")).unwrap();
        let with_unknown = format!("{base}\n[limits]\nmax_tools = 3\n");
        assert!(toml::from_str::<ProjectConfig>(&with_unknown).is_err());
    }

    #[test]
    fn validate_rejects_path_ids() {
        for bad_id in ["../x", "a/b", ".hidden"] {
            let mut agent = sample_agents().into_iter().next().unwrap();
            agent.agent_id = bad_id.to_string();
            assert!(
                matches!(agent.validate(), Err(Error::PathTraversal { .. })),
                "expected AgentConfig::validate to reject '{bad_id}'"
            );

            let mut role = sample_roles().into_iter().next().unwrap();
            role.role_id = bad_id.to_string();
            assert!(
                matches!(role.validate(), Err(Error::PathTraversal { .. })),
                "expected RoleConfig::validate to reject '{bad_id}'"
            );

            let inf = InferenceConfig {
                inference_id: bad_id.to_string(),
                endpoint_url: "http://127.0.0.1:8080".to_string(),
                mode: InferenceMode::Single,
                lifecycle: InferenceLifecycle::External,
                unload_after_stage: true,
                startup_timeout_secs: 300,
            };
            assert!(
                matches!(inf.validate(), Err(Error::PathTraversal { .. })),
                "expected InferenceConfig::validate to reject '{bad_id}'"
            );
        }
    }

    #[test]
    fn load_rejects_mismatched_file_stem() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("stem-mismatch").unwrap();

        let roles_dir = store.orrery_dir().join("roles");
        atomic_write(&roles_dir.join("reviewer.md"), b"# Reviewer rules\n").unwrap();
        let mismatched_role = RoleConfig {
            role_id: "reviewer".to_string(),
            display_name: "Reviewer".to_string(),
            kind: RoleKind::Validator,
            rules_file: "reviewer.md".to_string(),
            allowed_mcp_tools: Vec::new(),
        };
        atomic_write(
            &roles_dir.join("custom.toml"),
            toml::to_string_pretty(&mismatched_role).unwrap().as_bytes(),
        )
        .unwrap();

        match store.load_roles().unwrap_err() {
            Error::Validation { field, message } => {
                assert_eq!(field, "role_id");
                assert_eq!(
                    message,
                    "file 'custom.toml' declares id 'reviewer'; the file must be named 'reviewer.toml'"
                );
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }

        let agents_dir = store.orrery_dir().join("agents");
        let mut mismatched_agent = sample_agents().into_iter().next().unwrap();
        mismatched_agent.agent_id = "a2".to_string();
        atomic_write(
            &agents_dir.join("a1.toml"),
            toml::to_string_pretty(&mismatched_agent)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

        match store.load_agents().unwrap_err() {
            Error::Validation { field, message } => {
                assert_eq!(field, "agent_id");
                assert_eq!(
                    message,
                    "file 'a1.toml' declares id 'a2'; the file must be named 'a2.toml'"
                );
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[test]
    fn load_rejects_hand_edited_traversal_id() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("traversal-id").unwrap();

        let agents_dir = store.orrery_dir().join("agents");
        let mut bad_agent = sample_agents().into_iter().next().unwrap();
        bad_agent.agent_id = "../x".to_string();
        atomic_write(
            &agents_dir.join("x.toml"),
            toml::to_string_pretty(&bad_agent).unwrap().as_bytes(),
        )
        .unwrap();

        assert!(matches!(
            store.load_agents(),
            Err(Error::PathTraversal { .. })
        ));
    }

    #[test]
    fn remove_role_keeps_rules_when_peer_unreadable() {
        let proj_tmp = tempfile::TempDir::new().unwrap();
        let xdg_tmp = tempfile::TempDir::new().unwrap();
        let store = ConfigStore::new(proj_tmp.path(), Some(xdg_tmp.path().to_path_buf())).unwrap();
        store.init_project("shared-rules").unwrap();

        let role_one = RoleConfig {
            role_id: "role-one".to_string(),
            display_name: "Role One".to_string(),
            kind: RoleKind::Implementer,
            rules_file: "shared.md".to_string(),
            allowed_mcp_tools: Vec::new(),
        };
        let role_two = RoleConfig {
            role_id: "role-two".to_string(),
            display_name: "Role Two".to_string(),
            kind: RoleKind::Implementer,
            rules_file: "shared.md".to_string(),
            allowed_mcp_tools: Vec::new(),
        };
        store.save_role(&role_one, "# Shared rules\n").unwrap();
        store.save_role(&role_two, "# Shared rules\n").unwrap();

        let roles_dir = store.orrery_dir().join("roles");
        atomic_write(&roles_dir.join("broken.toml"), b"role_id = [\n").unwrap();

        assert!(store.remove_role("role-one").is_err());
        assert!(roles_dir.join("shared.md").exists());
    }
}
