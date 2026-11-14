use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::checkpoint::{AgentReport, StampedReport, SubtaskAllocation, report_json_schema};
use crate::config::{
    AgentBackend, AgentConfig, InferenceConfig, RoleConfig, RoleKind, format_mapped_tool_name,
};
use crate::mcp::{McpGatekeeper, McpToolSchema, WriteScope};
use crate::plan::PlanStep;
use crate::stream_log::LiveStreamHub;
use crate::{Error, Result};

pub const SUBMIT_REPORT_TOOL: &str = "submit_report";
pub const CLI_REPORT_PATH: &str = "/orrery/out/report.toml";

#[derive(Debug, Clone, PartialEq)]
pub struct AgentInvocation {
    pub run_id: uuid::Uuid,
    pub step_number: usize,
    pub stage_index: usize,
    pub stage_count: usize,
    pub round: usize,
    pub role: RoleConfig,
    pub agent: AgentConfig,
    pub system_prompt: String,
    pub user_prompt: String,
    pub tools: Vec<McpToolSchema>,
    pub write_scope: WriteScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSummary {
    pub index: usize,
    pub role_id: String,
    pub kind: RoleKind,
    pub agent_ids: Vec<String>,
}

pub fn build_system_prompt(
    role: &RoleConfig,
    rules_markdown: &str,
    backend: &AgentBackend,
) -> String {
    let reporting = match backend {
        AgentBackend::LocalModel { .. } | AgentBackend::CloudApi { .. } => {
            format!("End your turn by calling the `{SUBMIT_REPORT_TOOL}` tool exactly once.")
        }
        AgentBackend::AgentCli { .. } => {
            let schema_json =
                serde_json::to_string_pretty(&report_json_schema()).unwrap_or_default();
            format!(
                "End your turn by writing your report as TOML to {CLI_REPORT_PATH}.\n\n```json\n{schema_json}\n```"
            )
        }
    };

    format!(
        "# Role: {}\n\n{}\n\n# Reporting\n{}",
        role.display_name, rules_markdown, reporting
    )
}

fn format_prior_reports_section(prior: &[StampedReport]) -> Option<String> {
    if prior.is_empty() {
        return None;
    }
    let lines: Vec<String> = prior
        .iter()
        .map(|r| {
            let mut line = format!(
                "- round {}, stage {}, {}: {:?}: {}",
                r.round, r.stage_index, r.agent_id, r.report.status, r.report.summary
            );
            if let Some(feedback) = &r.report.feedback {
                line.push_str(&format!("  feedback: {feedback}"));
            }
            line
        })
        .collect();
    Some(format!("# Prior reports\n{}", lines.join("\n")))
}

fn format_notes_section(notes: &[String]) -> Option<String> {
    if notes.is_empty() {
        return None;
    }
    let lines: Vec<String> = notes.iter().map(|note| format!("- {note}")).collect();
    Some(format!("# Notes from Orrery\n{}", lines.join("\n")))
}

pub fn build_user_prompt(
    step: &PlanStep,
    allocation: Option<&SubtaskAllocation>,
    prior: &[StampedReport],
    notes: &[String],
) -> String {
    let mut sections = vec![format!(
        "# Plan step {}: {}\n\n{}",
        step.number, step.title, step.body
    )];

    if let Some(alloc) = allocation {
        let scope_text = if alloc.write_scope.is_empty() {
            "whole project".to_string()
        } else {
            alloc.write_scope.join(", ")
        };
        sections.push(format!(
            "# Your assignment\n{}\nWrite scope: {}",
            alloc.instructions, scope_text
        ));
    }

    if let Some(prior_section) = format_prior_reports_section(prior) {
        sections.push(prior_section);
    }

    if let Some(notes_section) = format_notes_section(notes) {
        sections.push(notes_section);
    }

    sections.join("\n\n")
}

pub fn build_orchestrator_prompt(
    step: &PlanStep,
    stages: &[StageSummary],
    prior: &[StampedReport],
    notes: &[String],
) -> String {
    let mut sections = vec![format!(
        "# Plan step {}: {}\n\n{}",
        step.number, step.title, step.body
    )];

    if !stages.is_empty() {
        let lines: Vec<String> = stages
            .iter()
            .map(|stage| {
                format!(
                    "- {}: {} ({:?}) agents: {}",
                    stage.index,
                    stage.role_id,
                    stage.kind,
                    stage.agent_ids.join(", ")
                )
            })
            .collect();
        sections.push(format!("# Stages\n{}", lines.join("\n")));
    }

    if let Some(prior_section) = format_prior_reports_section(prior) {
        sections.push(prior_section);
    }

    if let Some(notes_section) = format_notes_section(notes) {
        sections.push(notes_section);
    }

    sections.join("\n\n")
}

pub fn parse_mapped_tool_name(name: &str) -> Result<(String, String)> {
    let (server_id, tool) = name.split_once("__").ok_or_else(|| Error::Validation {
        field: "tool".to_string(),
        message: format!("mapped tool name '{name}' must contain '__' delimiter"),
    })?;

    format_mapped_tool_name(server_id, tool)?;
    Ok((server_id.to_string(), tool.to_string()))
}

#[async_trait]
pub trait AgentDriver: Send + Sync {
    async fn run_turn(
        &self,
        inv: &AgentInvocation,
        mcp: &McpGatekeeper,
        stream: &LiveStreamHub,
    ) -> Result<AgentReport>;
}

#[derive(Debug, Default)]
pub struct MockAgentDriver {
    scripts: Mutex<HashMap<String, VecDeque<Result<AgentReport>>>>,
    invocations: Mutex<Vec<AgentInvocation>>,
    delay: Mutex<Duration>,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
}

impl MockAgentDriver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, agent_id: &str, report: Result<AgentReport>) {
        let mut scripts = match self.scripts.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        scripts
            .entry(agent_id.to_string())
            .or_default()
            .push_back(report);
    }

    pub fn set_delay(&self, delay: Duration) {
        let mut guard = match self.delay.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = delay;
    }

    pub fn invocations(&self) -> Vec<AgentInvocation> {
        let guard = match self.invocations.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.clone()
    }

    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight.load(Ordering::SeqCst)
    }
}

struct InFlightGuard<'a> {
    in_flight: &'a AtomicUsize,
}

impl<'a> InFlightGuard<'a> {
    fn enter(in_flight: &'a AtomicUsize, max_in_flight: &AtomicUsize) -> Self {
        let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        max_in_flight.fetch_max(current, Ordering::SeqCst);
        Self { in_flight }
    }
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl AgentDriver for MockAgentDriver {
    async fn run_turn(
        &self,
        inv: &AgentInvocation,
        _mcp: &McpGatekeeper,
        _stream: &LiveStreamHub,
    ) -> Result<AgentReport> {
        let _in_flight_guard = InFlightGuard::enter(&self.in_flight, &self.max_in_flight);

        {
            let mut invocations = self.invocations.lock().map_err(|_| Error::Security {
                message: "MockAgentDriver lock poisoned".to_string(),
            })?;
            invocations.push(inv.clone());
        }

        let delay = {
            let guard = self.delay.lock().map_err(|_| Error::Security {
                message: "MockAgentDriver lock poisoned".to_string(),
            })?;
            *guard
        };

        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }

        let next = {
            let mut scripts = self.scripts.lock().map_err(|_| Error::Security {
                message: "MockAgentDriver lock poisoned".to_string(),
            })?;
            scripts
                .get_mut(&inv.agent.agent_id)
                .and_then(VecDeque::pop_front)
        };

        match next {
            Some(result) => result,
            None => Err(Error::Agent {
                agent_id: inv.agent.agent_id.clone(),
                message: "mock script exhausted".to_string(),
            }),
        }
    }
}

pub fn endpoint_for(agent: &AgentConfig, inference: &[InferenceConfig]) -> Result<String> {
    match &agent.backend {
        AgentBackend::LocalModel { inference_id, .. } => {
            let inf = inference
                .iter()
                .find(|entry| &entry.inference_id == inference_id)
                .ok_or_else(|| Error::Validation {
                    field: "backend.inference_id".to_string(),
                    message: format!("unknown inference_id '{inference_id}'"),
                })?;
            Ok(inf.chat_url())
        }
        AgentBackend::CloudApi { endpoint_url, .. } => Ok(endpoint_url.clone()),
        AgentBackend::AgentCli { .. } => Err(Error::Validation {
            field: "backend".to_string(),
            message: format!(
                "agent '{}' uses AgentCli backend, which does not support HTTP chat",
                agent.agent_id
            ),
        }),
    }
}

pub fn build_chat_request(
    inv: &AgentInvocation,
    messages: &[serde_json::Value],
) -> serde_json::Value {
    let (model, temperature) = match &inv.agent.backend {
        AgentBackend::LocalModel {
            model, temperature, ..
        }
        | AgentBackend::CloudApi {
            model, temperature, ..
        } => (model.as_str(), *temperature),
        AgentBackend::AgentCli { .. } => ("", 0.0),
    };

    let mut tools: Vec<serde_json::Value> = inv
        .tools
        .iter()
        .map(|tool| {
            let mapped_name = format_mapped_tool_name(&tool.server_id, &tool.name)
                .unwrap_or_else(|_| format!("{}__{}", tool.server_id, tool.name));
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": mapped_name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            })
        })
        .collect();

    tools.push(serde_json::json!({
        "type": "function",
        "function": {
            "name": SUBMIT_REPORT_TOOL,
            "description": "Submit the structured agent report to end your turn.",
            "parameters": report_json_schema(),
        }
    }));

    serde_json::json!({
        "model": model,
        "temperature": temperature,
        "stream": true,
        "messages": messages,
        "tools": tools,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatChunk {
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCallDelta>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseItem {
    Chunk(ChatChunk),
    Done,
}

#[derive(Debug, Default)]
pub struct SseParser {
    // Raw bytes: a network chunk can end inside a multi-byte UTF-8 char, so
    // only complete lines (split on b'\n', never inside a char) are decoded.
    buf: Vec<u8>,
}

#[derive(Deserialize)]
struct RawSsePayload {
    #[serde(default)]
    choices: Vec<RawChoice>,
}

#[derive(Deserialize)]
struct RawChoice {
    #[serde(default)]
    delta: RawDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Vec<RawToolCallDelta>,
}

#[derive(Deserialize)]
struct RawToolCallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
    #[serde(default)]
    function: Option<RawFunctionDelta>,
}

#[derive(Deserialize)]
struct RawFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

impl SseParser {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseItem>> {
        self.buf.extend_from_slice(bytes);

        let mut items = Vec::new();
        while let Some(newline_pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.buf.drain(..=newline_pos).collect();
            let line = String::from_utf8_lossy(&line_bytes[..newline_pos])
                .trim_end_matches('\r')
                .to_string();

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some(data) = trimmed.strip_prefix("data:") else {
                continue;
            };
            let payload = data.trim();
            if payload.is_empty() {
                continue;
            }
            if payload == "[DONE]" {
                items.push(SseItem::Done);
                continue;
            }

            let parsed: RawSsePayload = serde_json::from_str(payload)?;
            if let Some(choice) = parsed.choices.into_iter().next() {
                let reasoning = choice.delta.reasoning_content.or(choice.delta.reasoning);
                let tool_calls = choice
                    .delta
                    .tool_calls
                    .into_iter()
                    .map(|tc| {
                        let name = tc
                            .name
                            .or_else(|| tc.function.as_ref().and_then(|f| f.name.clone()));
                        let arguments = tc
                            .arguments
                            .or_else(|| tc.function.and_then(|f| f.arguments))
                            .unwrap_or_default();
                        ToolCallDelta {
                            index: tc.index,
                            id: tc.id,
                            name,
                            arguments,
                        }
                    })
                    .collect();

                items.push(SseItem::Chunk(ChatChunk {
                    content: choice.delta.content,
                    reasoning,
                    tool_calls,
                    finish_reason: choice.finish_reason,
                }));
            } else {
                items.push(SseItem::Chunk(ChatChunk {
                    content: None,
                    reasoning: None,
                    tool_calls: Vec::new(),
                    finish_reason: None,
                }));
            }
        }

        Ok(items)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Default)]
pub struct ToolCallAccumulator {
    calls: BTreeMap<usize, AssembledToolCall>,
}

impl ToolCallAccumulator {
    pub fn apply(&mut self, delta: &ToolCallDelta) {
        let entry = self
            .calls
            .entry(delta.index)
            .or_insert_with(|| AssembledToolCall {
                id: String::new(),
                name: String::new(),
                arguments: String::new(),
            });
        if let Some(id) = &delta.id
            && !id.is_empty()
        {
            entry.id.clone_from(id);
        }
        if let Some(name) = &delta.name
            && !name.is_empty()
        {
            entry.name.clone_from(name);
        }
        entry.arguments.push_str(&delta.arguments);
    }

    pub fn finish(self) -> Vec<AssembledToolCall> {
        self.calls.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::checkpoint::ReportStatus;
    use crate::config::WorkspaceMount;
    use crate::security::SecretRedactor;
    use crate::stream_log::{LiveStreamConfig, StreamEvent, StreamEventKind};

    fn sample_step() -> PlanStep {
        PlanStep {
            index: 0,
            number: 1,
            title: "1: Implement parser".to_string(),
            body: "Build the parser and add unit tests.".to_string(),
            sha256_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
        }
    }

    fn sample_role(kind: RoleKind) -> RoleConfig {
        RoleConfig {
            role_id: "coder".to_string(),
            display_name: "Lead Coder".to_string(),
            kind,
            rules_file: "roles/coder.md".to_string(),
            allowed_mcp_tools: vec!["filesystem:*".to_string()],
        }
    }

    fn sample_local_agent(agent_id: &str) -> AgentConfig {
        AgentConfig {
            agent_id: agent_id.to_string(),
            display_name: format!("Agent {agent_id}"),
            backend: AgentBackend::LocalModel {
                inference_id: "local-1".to_string(),
                model: "qwen-coder".to_string(),
                temperature: 0.2,
                max_context_tokens: 8192,
            },
        }
    }

    fn sample_invocation(agent_id: &str) -> AgentInvocation {
        let role = sample_role(RoleKind::Implementer);
        let agent = sample_local_agent(agent_id);
        AgentInvocation {
            run_id: Uuid::new_v4(),
            step_number: 1,
            stage_index: 1,
            stage_count: 3,
            round: 1,
            system_prompt: build_system_prompt(&role, "Follow Rust rules.", &agent.backend),
            user_prompt: build_user_prompt(&sample_step(), None, &[], &[]),
            role,
            agent,
            tools: Vec::new(),
            write_scope: WriteScope::anywhere(),
        }
    }

    fn sample_stamped_report(
        round: usize,
        stage_index: usize,
        agent_id: &str,
        status: ReportStatus,
        summary: &str,
        feedback: Option<&str>,
    ) -> StampedReport {
        StampedReport {
            step_number: 1,
            plan_step_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            round,
            stage_index,
            role_id: "coder".to_string(),
            agent_id: agent_id.to_string(),
            recorded_at: Utc::now(),
            report: AgentReport {
                status,
                summary: summary.to_string(),
                target_stage: None,
                allocations: Vec::new(),
                feedback: feedback.map(ToString::to_string),
                artifacts: Vec::new(),
            },
        }
    }

    #[test]
    fn user_prompt_contains_step_allocation_prior_and_notes() {
        let step = sample_step();
        let alloc = SubtaskAllocation {
            target_agent_id: "coder-1".to_string(),
            instructions: "Write src/parser.rs and its tests.".to_string(),
            write_scope: vec!["src/parser.rs".to_string(), "tests/parser.rs".to_string()],
        };
        let prior = vec![
            sample_stamped_report(
                1,
                1,
                "coder-1",
                ReportStatus::Complete,
                "Initial parser implementation",
                None,
            ),
            sample_stamped_report(
                1,
                2,
                "validator-1",
                ReportStatus::Denied,
                "Edge case failed",
                Some("Handle empty input on line 42"),
            ),
        ];
        let notes = vec![
            "gate cargo_test failed".to_string(),
            "retry 1 of 3".to_string(),
        ];

        let prompt = build_user_prompt(&step, Some(&alloc), &prior, &notes);

        assert!(prompt.contains(
            "# Plan step 1: 1: Implement parser\n\nBuild the parser and add unit tests."
        ));
        assert!(prompt.contains(
            "# Your assignment\nWrite src/parser.rs and its tests.\nWrite scope: src/parser.rs, tests/parser.rs"
        ));
        assert!(prompt.contains(
            "# Prior reports\n- round 1, stage 1, coder-1: Complete: Initial parser implementation\n- round 1, stage 2, validator-1: Denied: Edge case failed  feedback: Handle empty input on line 42"
        ));
        assert!(prompt.contains("# Notes from Orrery\n- gate cargo_test failed\n- retry 1 of 3"));

        let whole_project_alloc = SubtaskAllocation {
            target_agent_id: "coder-1".to_string(),
            instructions: "Refactor workspace.".to_string(),
            write_scope: Vec::new(),
        };
        let whole_prompt = build_user_prompt(&step, Some(&whole_project_alloc), &[], &[]);
        assert!(whole_prompt.contains("Write scope: whole project"));
    }

    #[test]
    fn user_prompt_omits_empty_sections() {
        let step = sample_step();
        let prompt = build_user_prompt(&step, None, &[], &[]);

        assert_eq!(
            prompt,
            "# Plan step 1: 1: Implement parser\n\nBuild the parser and add unit tests."
        );
        assert!(!prompt.contains("# Your assignment"));
        assert!(!prompt.contains("# Prior reports"));
        assert!(!prompt.contains("# Notes from Orrery"));

        let orch_prompt = build_orchestrator_prompt(&step, &[], &[], &[]);
        assert_eq!(
            orch_prompt,
            "# Plan step 1: 1: Implement parser\n\nBuild the parser and add unit tests."
        );
        assert!(!orch_prompt.contains("# Stages"));
        assert!(!orch_prompt.contains("# Prior reports"));
        assert!(!orch_prompt.contains("# Notes from Orrery"));
    }

    #[test]
    fn user_prompt_never_contains_stream_text() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();

        let sentinel = "STREAM-SENTINEL-7f3a";
        hub.emit(StreamEvent {
            timestamp: Utc::now(),
            step_number: Some(1),
            stage_index: Some(1),
            agent_id: Some("coder-1".to_string()),
            kind: StreamEventKind::Output,
            text: sentinel.to_string(),
        })
        .unwrap();

        let recent = hub.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert!(recent[0].text.contains(sentinel));

        let step = sample_step();
        let role = sample_role(RoleKind::Implementer);
        let alloc = SubtaskAllocation {
            target_agent_id: "coder-1".to_string(),
            instructions: "Implement feature".to_string(),
            write_scope: vec!["src".to_string()],
        };
        let prior = vec![sample_stamped_report(
            1,
            1,
            "coder-1",
            ReportStatus::Complete,
            "Done",
            None,
        )];
        let notes = vec!["note 1".to_string()];
        let stages = vec![StageSummary {
            index: 1,
            role_id: "coder".to_string(),
            kind: RoleKind::Implementer,
            agent_ids: vec!["coder-1".to_string()],
        }];

        let sys_local =
            build_system_prompt(&role, "Rules body", &sample_local_agent("coder-1").backend);
        let sys_cli = build_system_prompt(
            &role,
            "Rules body",
            &AgentBackend::AgentCli {
                image: "runner:latest".to_string(),
                command: vec!["run".to_string()],
                workspace_mount: WorkspaceMount::ReadWrite,
                memory_limit_mb: 512,
                timeout_secs: 60,
                keychain_service: None,
                keychain_account: None,
                secret_env: None,
            },
        );
        let user_p = build_user_prompt(&step, Some(&alloc), &prior, &notes);
        let orch_p = build_orchestrator_prompt(&step, &stages, &prior, &notes);

        for prompt in [&sys_local, &sys_cli, &user_p, &orch_p] {
            assert!(!prompt.contains(sentinel));
        }
    }

    #[test]
    fn system_prompt_chat_backends_mention_submit_report() {
        let role = sample_role(RoleKind::Implementer);
        let local_backend = sample_local_agent("local-1").backend;
        let cloud_backend = AgentBackend::CloudApi {
            endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
            model: "cloud-coder".to_string(),
            temperature: 0.1,
            max_context_tokens: 16384,
            keychain_service: "orrery.test".to_string(),
            keychain_account: "testuser".to_string(),
        };

        let local_prompt = build_system_prompt(&role, "Write clean Rust.", &local_backend);
        let cloud_prompt = build_system_prompt(&role, "Write clean Rust.", &cloud_backend);

        let expected = "# Role: Lead Coder\n\nWrite clean Rust.\n\n# Reporting\nEnd your turn by calling the `submit_report` tool exactly once.";
        assert_eq!(local_prompt, expected);
        assert_eq!(cloud_prompt, expected);
    }

    #[test]
    fn system_prompt_cli_mentions_report_path_and_schema() {
        let role = sample_role(RoleKind::Validator);
        let cli_backend = AgentBackend::AgentCli {
            image: "validator:latest".to_string(),
            command: vec!["validate".to_string()],
            workspace_mount: WorkspaceMount::ReadOnly,
            memory_limit_mb: 1024,
            timeout_secs: 120,
            keychain_service: None,
            keychain_account: None,
            secret_env: None,
        };

        let prompt = build_system_prompt(&role, "Verify all invariants.", &cli_backend);
        let pretty_schema = serde_json::to_string_pretty(&report_json_schema()).unwrap();

        assert!(prompt.starts_with(
            "# Role: Lead Coder\n\nVerify all invariants.\n\n# Reporting\nEnd your turn by writing your report as TOML to /orrery/out/report.toml."
        ));
        assert!(prompt.contains(CLI_REPORT_PATH));
        assert!(prompt.contains(&format!("```json\n{pretty_schema}\n```")));
    }

    #[test]
    fn orchestrator_prompt_lists_stages() {
        let step = sample_step();
        let stages = vec![
            StageSummary {
                index: 0,
                role_id: "orchestrator".to_string(),
                kind: RoleKind::Orchestrator,
                agent_ids: vec!["orch-1".to_string()],
            },
            StageSummary {
                index: 1,
                role_id: "coder".to_string(),
                kind: RoleKind::Implementer,
                agent_ids: vec!["coder-1".to_string(), "coder-2".to_string()],
            },
            StageSummary {
                index: 2,
                role_id: "validator".to_string(),
                kind: RoleKind::Validator,
                agent_ids: vec!["val-1".to_string()],
            },
        ];
        let prior = vec![sample_stamped_report(
            1,
            1,
            "coder-1",
            ReportStatus::Complete,
            "Finished part 1",
            None,
        )];
        let notes = vec!["Proceed to validation".to_string()];

        let prompt = build_orchestrator_prompt(&step, &stages, &prior, &notes);

        let step_pos = prompt.find("# Plan step 1:").unwrap();
        let stages_pos = prompt.find("# Stages\n").unwrap();
        let prior_pos = prompt.find("# Prior reports\n").unwrap();
        let notes_pos = prompt.find("# Notes from Orrery\n").unwrap();

        assert!(step_pos < stages_pos);
        assert!(stages_pos < prior_pos);
        assert!(prior_pos < notes_pos);

        assert!(prompt.contains(
            "# Stages\n- 0: orchestrator (Orchestrator) agents: orch-1\n- 1: coder (Implementer) agents: coder-1, coder-2\n- 2: validator (Validator) agents: val-1"
        ));
    }

    #[test]
    fn mapped_name_round_trip() {
        let mapped = format_mapped_tool_name("filesystem", "read_file").unwrap();
        assert_eq!(mapped, "filesystem__read_file");

        let (server_id, tool) = parse_mapped_tool_name(&mapped).unwrap();
        assert_eq!(server_id, "filesystem");
        assert_eq!(tool, "read_file");
    }

    #[test]
    fn mapped_name_rejects_invalid() {
        let overlong = format!("server__{}", "a".repeat(57));
        assert_eq!(overlong.len(), 65);

        for invalid in [
            "nodelimiter",
            "__tool",
            "server__",
            overlong.as_str(),
            "s__t__extra",
        ] {
            let err = parse_mapped_tool_name(invalid).unwrap_err();
            assert!(
                matches!(err, Error::Validation { .. }),
                "expected Validation error for '{invalid}', got {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn mock_driver_returns_scripted_reports_in_order() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();

        let driver = MockAgentDriver::new();
        let r1 = AgentReport {
            status: ReportStatus::Complete,
            summary: "First turn".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        };
        let r2 = AgentReport {
            status: ReportStatus::Blocked,
            summary: "Second turn".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: Some("Need clarification".to_string()),
            artifacts: Vec::new(),
        };

        driver.push("coder-1", Ok(r1.clone()));
        driver.push("coder-1", Ok(r2.clone()));

        let inv = sample_invocation("coder-1");
        let out1 = driver.run_turn(&inv, &mcp, &hub).await.unwrap();
        let out2 = driver.run_turn(&inv, &mcp, &hub).await.unwrap();

        assert_eq!(out1, r1);
        assert_eq!(out2, r2);
        assert_eq!(driver.invocations().len(), 2);
        assert_eq!(driver.invocations()[0], inv);
    }

    #[tokio::test]
    async fn mock_driver_exhausted_is_agent_error() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();

        let driver = MockAgentDriver::new();
        let inv = sample_invocation("coder-1");

        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "coder-1");
                assert_eq!(message, "mock script exhausted");
            }
            other => panic!("expected Error::Agent, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn mock_driver_tracks_max_in_flight() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();

        let driver = MockAgentDriver::new();
        driver.set_delay(Duration::from_millis(50));

        let report = AgentReport {
            status: ReportStatus::Complete,
            summary: "Done".to_string(),
            target_stage: None,
            allocations: Vec::new(),
            feedback: None,
            artifacts: Vec::new(),
        };
        driver.push("coder-1", Ok(report.clone()));
        driver.push("coder-2", Ok(report));

        let inv1 = sample_invocation("coder-1");
        let inv2 = sample_invocation("coder-2");

        let (res1, res2) = tokio::join!(
            driver.run_turn(&inv1, &mcp, &hub),
            driver.run_turn(&inv2, &mcp, &hub)
        );
        assert!(res1.is_ok());
        assert!(res2.is_ok());
        assert_eq!(driver.max_in_flight(), 2);
    }

    fn sample_inference(inference_id: &str, endpoint_url: &str) -> InferenceConfig {
        use crate::config::{InferenceLifecycle, InferenceMode};
        InferenceConfig {
            inference_id: inference_id.to_string(),
            endpoint_url: endpoint_url.to_string(),
            mode: InferenceMode::Single,
            lifecycle: InferenceLifecycle::External,
            unload_after_stage: true,
            startup_timeout_secs: 60,
        }
    }

    #[test]
    fn endpoint_local_model_uses_chat_url() {
        let agent = sample_local_agent("local-coder");
        let inference = vec![sample_inference("local-1", "http://192.0.2.10:8080/")];

        let url = endpoint_for(&agent, &inference).unwrap();
        assert_eq!(url, "http://192.0.2.10:8080/v1/chat/completions");
    }

    #[test]
    fn endpoint_cloud_uses_url() {
        let agent = AgentConfig {
            agent_id: "cloud-1".to_string(),
            display_name: "Cloud Agent".to_string(),
            backend: AgentBackend::CloudApi {
                endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                model: "cloud-model".to_string(),
                temperature: 0.2,
                max_context_tokens: 16384,
                keychain_service: "orrery.test".to_string(),
                keychain_account: "testuser".to_string(),
            },
        };

        let url = endpoint_for(&agent, &[]).unwrap();
        assert_eq!(url, "https://api.example.com/v1/chat/completions");
    }

    #[test]
    fn endpoint_cli_rejected() {
        let agent = AgentConfig {
            agent_id: "cli-1".to_string(),
            display_name: "CLI Agent".to_string(),
            backend: AgentBackend::AgentCli {
                image: "runner:latest".to_string(),
                command: vec!["run".to_string()],
                workspace_mount: WorkspaceMount::ReadOnly,
                memory_limit_mb: 512,
                timeout_secs: 60,
                keychain_service: None,
                keychain_account: None,
                secret_env: None,
            },
        };

        let err = endpoint_for(&agent, &[]).unwrap_err();
        assert!(matches!(
            err,
            Error::Validation { ref field, .. } if field == "backend"
        ));
    }

    #[test]
    fn endpoint_unknown_inference_rejected() {
        let agent = sample_local_agent("local-coder");
        let inference = vec![sample_inference("other-id", "http://192.0.2.10:8080")];

        let err = endpoint_for(&agent, &inference).unwrap_err();
        assert!(matches!(
            err,
            Error::Validation { ref field, .. } if field == "backend.inference_id"
        ));
    }

    #[test]
    fn request_includes_tools_and_submit_report() {
        let mut inv = sample_invocation("coder-1");
        inv.tools = vec![
            McpToolSchema {
                server_id: "filesystem".to_string(),
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"]
                }),
            },
            McpToolSchema {
                server_id: "git".to_string(),
                name: "status".to_string(),
                description: "Check git status".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ];

        let messages = vec![
            serde_json::json!({"role": "system", "content": inv.system_prompt}),
            serde_json::json!({"role": "user", "content": inv.user_prompt}),
        ];

        let req = build_chat_request(&inv, &messages);

        assert_eq!(req["model"], "qwen-coder");
        assert_eq!(req["temperature"], 0.2);
        assert_eq!(req["stream"], true);
        assert_eq!(req["messages"], serde_json::json!(messages));

        let tools = req["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);

        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "filesystem__read_file");
        assert_eq!(tools[0]["function"]["description"], "Read a file");
        assert_eq!(
            tools[0]["function"]["parameters"],
            inv.tools[0].input_schema
        );

        assert_eq!(tools[1]["type"], "function");
        assert_eq!(tools[1]["function"]["name"], "git__status");
        assert_eq!(tools[1]["function"]["description"], "Check git status");
        assert_eq!(
            tools[1]["function"]["parameters"],
            inv.tools[1].input_schema
        );

        let last_tool = &tools[2];
        assert_eq!(last_tool["type"], "function");
        assert_eq!(last_tool["function"]["name"], SUBMIT_REPORT_TOOL);
        assert_eq!(last_tool["function"]["parameters"], report_json_schema());
    }

    #[test]
    fn sse_content_chunk() {
        let mut parser = SseParser::default();
        let line = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello world\"},\"finish_reason\":null}]}\n\n";
        let items = parser.push(line).unwrap();

        assert_eq!(
            items,
            vec![SseItem::Chunk(ChatChunk {
                content: Some("Hello world".to_string()),
                reasoning: None,
                tool_calls: Vec::new(),
                finish_reason: None,
            })]
        );
    }

    #[test]
    fn sse_reasoning_content_and_reasoning_keys() {
        let mut parser = SseParser::default();
        let input = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"step 1\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning\":\"step 2\"}}]}\n",
        );
        let items = parser.push(input.as_bytes()).unwrap();

        assert_eq!(
            items,
            vec![
                SseItem::Chunk(ChatChunk {
                    content: None,
                    reasoning: Some("step 1".to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: None,
                }),
                SseItem::Chunk(ChatChunk {
                    content: None,
                    reasoning: Some("step 2".to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: None,
                }),
            ]
        );
    }

    #[test]
    fn sse_split_across_pushes() {
        let mut parser = SseParser::default();
        let part1 = b"data: {\"choices\":[{\"delta\":{\"content\":\"part";
        let part2 = b"ial line\"},\"finish_reason\":\"stop\"}]}\n";

        let first = parser.push(part1).unwrap();
        assert!(first.is_empty());

        let second = parser.push(part2).unwrap();
        assert_eq!(
            second,
            vec![SseItem::Chunk(ChatChunk {
                content: Some("partial line".to_string()),
                reasoning: None,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_string()),
            })]
        );
    }

    #[test]
    fn sse_utf8_split_across_pushes() {
        let mut parser = SseParser::default();
        let line = "data: {\"choices\":[{\"delta\":{\"content\":\"caf\u{e9} \u{2014} ok\"}}]}\n";
        let bytes = line.as_bytes();
        // Split inside the two-byte 'é' (0xC3 0xA9).
        let split = line.find('\u{e9}').unwrap() + 1;

        assert!(parser.push(&bytes[..split]).unwrap().is_empty());
        let items = parser.push(&bytes[split..]).unwrap();
        assert_eq!(
            items,
            vec![SseItem::Chunk(ChatChunk {
                content: Some("caf\u{e9} \u{2014} ok".to_string()),
                reasoning: None,
                tool_calls: Vec::new(),
                finish_reason: None,
            })]
        );
    }

    #[test]
    fn sse_done_marker() {
        let mut parser = SseParser::default();
        let items = parser.push(b"data: [DONE]\n").unwrap();
        assert_eq!(items, vec![SseItem::Done]);
    }

    #[test]
    fn sse_ignores_comments_and_blank_lines() {
        let mut parser = SseParser::default();
        let input = concat!(
            ": keep-alive comment\n",
            "\n",
            "   \r\n",
            "event: message\n",
            ": another comment\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n",
        );
        let items = parser.push(input.as_bytes()).unwrap();

        assert_eq!(
            items,
            vec![SseItem::Chunk(ChatChunk {
                content: Some("ok".to_string()),
                reasoning: None,
                tool_calls: Vec::new(),
                finish_reason: None,
            })]
        );
    }

    #[test]
    fn sse_bad_json_is_json_error() {
        let mut parser = SseParser::default();
        let err = parser.push(b"data: {not-valid-json\n").unwrap_err();
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn accumulator_joins_argument_fragments() {
        let mut parser = SseParser::default();
        let sse_stream = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"filesystem__read_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"src/lib\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\".rs\\\"}\"}}],\"finish_reason\":\"tool_calls\"}}]}\n",
        );

        let items = parser.push(sse_stream.as_bytes()).unwrap();
        assert_eq!(items.len(), 3);

        let mut acc = ToolCallAccumulator::default();
        for item in &items {
            if let SseItem::Chunk(chunk) = item {
                for delta in &chunk.tool_calls {
                    acc.apply(delta);
                }
            }
        }

        let calls = acc.finish();
        assert_eq!(
            calls,
            vec![AssembledToolCall {
                id: "call_1".to_string(),
                name: "filesystem__read_file".to_string(),
                arguments: "{\"path\":\"src/lib.rs\"}".to_string(),
            }]
        );
    }

    #[test]
    fn accumulator_keeps_parallel_calls_ordered() {
        let mut acc = ToolCallAccumulator::default();
        acc.apply(&ToolCallDelta {
            index: 1,
            id: Some("call_b".to_string()),
            name: Some("git__status".to_string()),
            arguments: "{}".to_string(),
        });
        acc.apply(&ToolCallDelta {
            index: 0,
            id: Some("call_a".to_string()),
            name: Some("filesystem__read_file".to_string()),
            arguments: "{\"path\":\"Cargo.toml\"}".to_string(),
        });

        let calls = acc.finish();
        assert_eq!(
            calls,
            vec![
                AssembledToolCall {
                    id: "call_a".to_string(),
                    name: "filesystem__read_file".to_string(),
                    arguments: "{\"path\":\"Cargo.toml\"}".to_string(),
                },
                AssembledToolCall {
                    id: "call_b".to_string(),
                    name: "git__status".to_string(),
                    arguments: "{}".to_string(),
                },
            ]
        );
    }
}
