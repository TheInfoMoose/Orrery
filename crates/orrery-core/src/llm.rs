use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use jiff::Timestamp;
use serde::Deserialize;

use crate::checkpoint::{AgentReport, StampedReport, SubtaskAllocation, report_json_schema};
use crate::config::{
    AgentBackend, AgentConfig, InferenceConfig, RoleConfig, RoleKind, WorkspaceMount, atomic_write,
    format_mapped_tool_name,
};
use crate::docker::{
    ContainerBackend, ContainerSpec, EnvValue, agent_io_dir, cli_container_name, mounts_for,
};
use crate::mcp::{McpGatekeeper, McpToolSchema, WriteScope, tool_result_text};
use crate::plan::PlanStep;
use crate::security::SecretStore;
use crate::stream_log::{LiveStreamHub, StreamEvent, StreamEventKind};
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

pub fn estimate_tokens(body: &serde_json::Value) -> usize {
    match serde_json::to_string(body) {
        Ok(s) => s.len().div_ceil(4),
        Err(_) => usize::MAX,
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
        .filter_map(|tool| {
            let mapped_name = format_mapped_tool_name(&tool.server_id, &tool.name).ok()?;
            Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": mapped_name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            }))
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

const DEFAULT_MAX_TOOL_CALLS: usize = 50;
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;
const MAX_LINE_BUFFER_BYTES: usize = 4096;

pub struct HttpChatDriver {
    client: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
    inference: Vec<InferenceConfig>,
    max_tool_calls: usize,
    idle_timeout: Duration,
}

impl HttpChatDriver {
    pub fn new(secrets: Arc<dyn SecretStore>, inference: Vec<InferenceConfig>) -> Self {
        Self {
            client: reqwest::Client::new(),
            secrets,
            inference,
            max_tool_calls: DEFAULT_MAX_TOOL_CALLS,
            idle_timeout: Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS),
        }
    }

    pub fn with_max_tool_calls(mut self, n: usize) -> Self {
        self.max_tool_calls = n;
        self
    }

    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    fn new_turn_state(&self, inv: &AgentInvocation) -> TurnState {
        TurnState::new(inv, self.max_tool_calls)
    }
}

fn auth_header_value(
    agent: &AgentConfig,
    secrets: &dyn SecretStore,
) -> Result<Option<reqwest::header::HeaderValue>> {
    match &agent.backend {
        AgentBackend::CloudApi {
            keychain_service,
            keychain_account,
            ..
        } => {
            let secret = secrets.get(keychain_service, keychain_account)?;
            let bearer = zeroize::Zeroizing::new(format!("Bearer {}", secret.expose()));
            let mut value =
                reqwest::header::HeaderValue::from_str(&bearer).map_err(|_| Error::Validation {
                    field: "keychain".to_string(),
                    message: "API key is not a valid HTTP header value".to_string(),
                })?;
            value.set_sensitive(true);
            Ok(Some(value))
        }
        AgentBackend::LocalModel { .. } | AgentBackend::AgentCli { .. } => Ok(None),
    }
}

struct LineBuffer {
    kind: StreamEventKind,
    buf: String,
}

impl LineBuffer {
    fn new(kind: StreamEventKind) -> Self {
        Self {
            kind,
            buf: String::new(),
        }
    }

    fn push(&mut self, delta: &str, inv: &AgentInvocation, stream: &LiveStreamHub) -> Result<()> {
        if delta.is_empty() {
            return Ok(());
        }
        self.buf.push_str(delta);

        loop {
            if let Some(pos) = self.buf.find('\n')
                && pos <= MAX_LINE_BUFFER_BYTES
            {
                let mut line: String = self.buf.drain(..=pos).collect();
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
                if !line.is_empty() {
                    self.emit_line(line, inv, stream)?;
                }
                continue;
            }

            if self.buf.len() > MAX_LINE_BUFFER_BYTES {
                let cut = self.buf.floor_char_boundary(MAX_LINE_BUFFER_BYTES);
                let chunk: String = self.buf.drain(..cut).collect();
                if !chunk.is_empty() {
                    self.emit_line(chunk, inv, stream)?;
                }
                continue;
            }

            break;
        }

        Ok(())
    }

    fn flush(&mut self, inv: &AgentInvocation, stream: &LiveStreamHub) -> Result<()> {
        if !self.buf.is_empty() {
            let remainder = std::mem::take(&mut self.buf);
            self.emit_line(remainder, inv, stream)?;
        }
        Ok(())
    }

    fn emit_line(&self, text: String, inv: &AgentInvocation, stream: &LiveStreamHub) -> Result<()> {
        stream.emit(StreamEvent {
            timestamp: Timestamp::now(),
            step_number: Some(inv.step_number),
            stage_index: Some(inv.stage_index),
            agent_id: Some(inv.agent.agent_id.clone()),
            kind: self.kind,
            text,
        })
    }
}

struct ResponseStreamProcessor {
    parser: SseParser,
    output_buf: LineBuffer,
    reasoning_buf: LineBuffer,
    tool_acc: ToolCallAccumulator,
    assistant_content: String,
    done: bool,
}

impl Default for ResponseStreamProcessor {
    fn default() -> Self {
        Self {
            parser: SseParser::default(),
            output_buf: LineBuffer::new(StreamEventKind::Output),
            reasoning_buf: LineBuffer::new(StreamEventKind::Reasoning),
            tool_acc: ToolCallAccumulator::default(),
            assistant_content: String::new(),
            done: false,
        }
    }
}

impl ResponseStreamProcessor {
    fn push_sse_bytes(
        &mut self,
        bytes: &[u8],
        inv: &AgentInvocation,
        stream: &LiveStreamHub,
    ) -> Result<bool> {
        let items = self.parser.push(bytes)?;
        self.push_items(&items, inv, stream)
    }

    fn push_items(
        &mut self,
        items: &[SseItem],
        inv: &AgentInvocation,
        stream: &LiveStreamHub,
    ) -> Result<bool> {
        for item in items {
            match item {
                SseItem::Chunk(chunk) => {
                    if let Some(reasoning) = &chunk.reasoning {
                        self.reasoning_buf.push(reasoning, inv, stream)?;
                    }
                    if let Some(content) = &chunk.content {
                        self.assistant_content.push_str(content);
                        self.output_buf.push(content, inv, stream)?;
                    }
                    for delta in &chunk.tool_calls {
                        self.tool_acc.apply(delta);
                    }
                }
                SseItem::Done => {
                    self.done = true;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn finish(
        mut self,
        inv: &AgentInvocation,
        stream: &LiveStreamHub,
    ) -> Result<(String, Vec<AssembledToolCall>)> {
        if !self.done && !self.parser.buf.is_empty() {
            let items = self.parser.push(b"\n")?;
            let _ = self.push_items(&items, inv, stream)?;
        }
        self.reasoning_buf.flush(inv, stream)?;
        self.output_buf.flush(inv, stream)?;
        Ok((self.assistant_content, self.tool_acc.finish()))
    }
}

struct TurnState {
    messages: Vec<serde_json::Value>,
    max_tool_calls: usize,
    tool_calls_used: usize,
    call_seq: usize,
    invalid_report_reprompted: bool,
    no_tool_call_reprompted: bool,
}

impl TurnState {
    fn new(inv: &AgentInvocation, max_tool_calls: usize) -> Self {
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": inv.system_prompt,
            }),
            serde_json::json!({
                "role": "user",
                "content": inv.user_prompt,
            }),
        ];
        Self {
            messages,
            max_tool_calls,
            tool_calls_used: 0,
            call_seq: 0,
            invalid_report_reprompted: false,
            no_tool_call_reprompted: false,
        }
    }

    fn messages(&self) -> &[serde_json::Value] {
        &self.messages
    }

    async fn finish_response(
        &mut self,
        proc: ResponseStreamProcessor,
        inv: &AgentInvocation,
        mcp: &McpGatekeeper,
        stream: &LiveStreamHub,
    ) -> Result<Option<AgentReport>> {
        let (assistant_content, mut calls) = proc.finish(inv, stream)?;
        if calls.is_empty() {
            if self.no_tool_call_reprompted {
                return Err(Error::Report {
                    message: "agent did not call submit_report after reprompt".to_string(),
                });
            }
            self.no_tool_call_reprompted = true;
            self.messages.push(serde_json::json!({
                "role": "user",
                "content": "You must call submit_report.",
            }));
            return Ok(None);
        }

        for call in &mut calls {
            self.call_seq += 1;
            if call.id.is_empty() {
                call.id = format!("orrery_call_{}", self.call_seq);
            }
        }

        let tool_calls_json: Vec<serde_json::Value> = calls
            .iter()
            .map(|call| {
                serde_json::json!({
                    "id": call.id,
                    "type": "function",
                    "function": {
                        "name": call.name,
                        "arguments": call.arguments,
                    }
                })
            })
            .collect();
        let content_val = if assistant_content.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(assistant_content)
        };
        self.messages.push(serde_json::json!({
            "role": "assistant",
            "content": content_val,
            "tool_calls": tool_calls_json,
        }));

        for call in calls {
            if call.name == SUBMIT_REPORT_TOOL {
                let parsed = serde_json::from_str::<AgentReport>(&call.arguments)
                    .map_err(|err| Error::Report {
                        message: format!("invalid submit_report JSON: {err}"),
                    })
                    .and_then(|report| {
                        report.validate_for_role(&inv.role, inv.stage_count)?;
                        Ok(report)
                    });
                match parsed {
                    Ok(report) => return Ok(Some(report)),
                    Err(err) => {
                        if self.invalid_report_reprompted {
                            return Err(err);
                        }
                        self.invalid_report_reprompted = true;
                        self.messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call.id,
                            "content": format!("error: {err}"),
                        }));
                    }
                }
                continue;
            }

            self.tool_calls_used += 1;
            if self.tool_calls_used > self.max_tool_calls {
                return Err(Error::Agent {
                    agent_id: inv.agent.agent_id.clone(),
                    message: format!(
                        "exceeded maximum of {} tool calls per turn",
                        self.max_tool_calls
                    ),
                });
            }

            let (server_id, tool_name) = match parse_mapped_tool_name(&call.name) {
                Ok(pair) => pair,
                Err(err) => {
                    stream.emit(StreamEvent {
                        timestamp: Timestamp::now(),
                        step_number: Some(inv.step_number),
                        stage_index: Some(inv.stage_index),
                        agent_id: Some(inv.agent.agent_id.clone()),
                        kind: StreamEventKind::ToolCall,
                        text: format!("{} denied: {err}", call.name),
                    })?;
                    self.messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": format!("error: {err}"),
                    }));
                    continue;
                }
            };

            let exec_res = match serde_json::from_str::<serde_json::Value>(&call.arguments) {
                Ok(args) => {
                    mcp.execute_for_role(&inv.role, &inv.write_scope, &server_id, &tool_name, args)
                        .await
                }
                Err(err) => Err(Error::from(err)),
            };

            match exec_res {
                Ok(val) => {
                    stream.emit(StreamEvent {
                        timestamp: Timestamp::now(),
                        step_number: Some(inv.step_number),
                        stage_index: Some(inv.stage_index),
                        agent_id: Some(inv.agent.agent_id.clone()),
                        kind: StreamEventKind::ToolCall,
                        text: format!("{server_id}:{tool_name} ok"),
                    })?;
                    self.messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": tool_result_text(&val),
                    }));
                }
                Err(err) => {
                    stream.emit(StreamEvent {
                        timestamp: Timestamp::now(),
                        step_number: Some(inv.step_number),
                        stage_index: Some(inv.stage_index),
                        agent_id: Some(inv.agent.agent_id.clone()),
                        kind: StreamEventKind::ToolCall,
                        text: format!("{server_id}:{tool_name} denied: {err}"),
                    })?;
                    self.messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": format!("error: {err}"),
                    }));
                }
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl AgentDriver for HttpChatDriver {
    async fn run_turn(
        &self,
        inv: &AgentInvocation,
        mcp: &McpGatekeeper,
        stream: &LiveStreamHub,
    ) -> Result<AgentReport> {
        let url = endpoint_for(&inv.agent, &self.inference)?;
        let mut state = self.new_turn_state(inv);

        loop {
            let body = build_chat_request(inv, state.messages());
            if let AgentBackend::LocalModel {
                max_context_tokens, ..
            }
            | AgentBackend::CloudApi {
                max_context_tokens, ..
            } = &inv.agent.backend
            {
                let est = estimate_tokens(&body);
                if est > *max_context_tokens {
                    return Err(Error::Agent {
                        agent_id: inv.agent.agent_id.clone(),
                        message: format!(
                            "prompt is about {est} tokens, over max_context_tokens {max_context_tokens}"
                        ),
                    });
                }
            }
            let mut req = self.client.post(&url).json(&body);
            if let Some(value) = auth_header_value(&inv.agent, self.secrets.as_ref())? {
                req = req.header(reqwest::header::AUTHORIZATION, value);
            }

            let resp = tokio::time::timeout(self.idle_timeout, req.send())
                .await
                .map_err(|_| Error::Agent {
                    agent_id: inv.agent.agent_id.clone(),
                    message: format!(
                        "no response from model within {}s",
                        self.idle_timeout.as_secs()
                    ),
                })?
                .map_err(Error::Http)?;
            let status = resp.status();
            if !status.is_success() {
                return Err(Error::Agent {
                    agent_id: inv.agent.agent_id.clone(),
                    message: format!("HTTP {status}"),
                });
            }

            let mut proc = ResponseStreamProcessor::default();
            let mut byte_stream = resp.bytes_stream();

            while let Some(chunk_res) = tokio::time::timeout(self.idle_timeout, byte_stream.next())
                .await
                .map_err(|_| Error::Agent {
                    agent_id: inv.agent.agent_id.clone(),
                    message: format!(
                        "no response from model within {}s",
                        self.idle_timeout.as_secs()
                    ),
                })?
            {
                let bytes = chunk_res.map_err(Error::Http)?;
                if proc.push_sse_bytes(&bytes, inv, stream)? {
                    break;
                }
            }

            if let Some(report) = state.finish_response(proc, inv, mcp, stream).await? {
                return Ok(report);
            }
        }
    }
}

pub struct CliContainerDriver {
    backend: Arc<dyn ContainerBackend>,
    secrets: Arc<dyn SecretStore>,
    project_root: PathBuf,
}

impl CliContainerDriver {
    pub fn new(
        backend: Arc<dyn ContainerBackend>,
        secrets: Arc<dyn SecretStore>,
        project_root: PathBuf,
    ) -> Self {
        Self {
            backend,
            secrets,
            project_root,
        }
    }
}

#[async_trait]
impl AgentDriver for CliContainerDriver {
    async fn run_turn(
        &self,
        inv: &AgentInvocation,
        _mcp: &McpGatekeeper,
        stream: &LiveStreamHub,
    ) -> Result<AgentReport> {
        let AgentBackend::AgentCli {
            image,
            command,
            workspace_mount,
            memory_limit_mb,
            timeout_secs,
            keychain_service,
            keychain_account,
            secret_env,
        } = &inv.agent.backend
        else {
            return Err(Error::Validation {
                field: "backend".to_string(),
                message: format!(
                    "agent '{}' does not use AgentCli backend",
                    inv.agent.agent_id
                ),
            });
        };

        let dir = agent_io_dir(&self.project_root, &inv.run_id, &inv.agent.agent_id);
        let in_dir = dir.join("in");
        let out_dir = dir.join("out");
        std::fs::create_dir_all(&in_dir).map_err(|source| Error::Io {
            path: in_dir.clone(),
            source,
        })?;
        std::fs::create_dir_all(&out_dir).map_err(|source| Error::Io {
            path: out_dir.clone(),
            source,
        })?;
        if *workspace_mount != WorkspaceMount::None {
            let mask_dir = dir.join("mask");
            if mask_dir.exists() {
                std::fs::remove_dir_all(&mask_dir).map_err(|source| Error::Io {
                    path: mask_dir.clone(),
                    source,
                })?;
            }
            std::fs::create_dir_all(&mask_dir).map_err(|source| Error::Io {
                path: mask_dir,
                source,
            })?;
        }

        let base_prompt = format!("{}\n\n{}", inv.system_prompt, inv.user_prompt);
        let mut current_prompt = base_prompt.clone();
        let prompt_path = in_dir.join("prompt.md");
        let report_path = out_dir.join("report.toml");
        let name = cli_container_name(&inv.run_id, &inv.agent.agent_id);
        self.backend.remove(&name).await?;
        let mounts = mounts_for(&inv.agent, &self.project_root, &dir);

        let mut last_error = Error::Report {
            message: "report not produced".to_string(),
        };

        for _attempt in 0..2 {
            // A report left from an earlier turn (or a crash) must never be
            // read as this run's output.
            match std::fs::remove_file(&report_path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(Error::Io {
                        path: report_path.clone(),
                        source,
                    });
                }
            }
            atomic_write(&prompt_path, current_prompt.as_bytes())?;

            let mut env = Vec::new();
            if let (Some(env_key), Some(service), Some(account)) =
                (secret_env, keychain_service, keychain_account)
            {
                let secret = self.secrets.get(service, account)?;
                env.push((env_key.clone(), EnvValue::Secret(secret)));
            }

            let spec = ContainerSpec {
                name: name.clone(),
                image: image.clone(),
                cmd: command.clone(),
                env,
                mounts: mounts.clone(),
                memory_limit_mb: *memory_limit_mb,
                auto_remove: false,
            };

            let on_output = |line: String| {
                let _ = stream.emit(StreamEvent {
                    timestamp: Timestamp::now(),
                    step_number: Some(inv.step_number),
                    stage_index: Some(inv.stage_index),
                    agent_id: Some(inv.agent.agent_id.clone()),
                    kind: StreamEventKind::Output,
                    text: line,
                });
            };

            let run_res = tokio::time::timeout(
                Duration::from_secs(*timeout_secs),
                self.backend.run_to_completion(
                    &spec,
                    current_prompt.as_bytes().to_vec(),
                    &on_output,
                ),
            )
            .await;

            let exit_res = match run_res {
                Ok(res) => res,
                Err(_) => {
                    let _ = self.backend.remove(&name).await;
                    return Err(Error::Agent {
                        agent_id: inv.agent.agent_id.clone(),
                        message: format!("timed out after {timeout_secs}s"),
                    });
                }
            };

            self.backend.remove(&name).await?;
            let _exit_status = exit_res?;

            let parsed_report = match std::fs::read_to_string(&report_path) {
                Ok(raw) => toml::from_str::<AgentReport>(&raw)
                    .map_err(|err| Error::Report {
                        message: format!("invalid report TOML: {err}"),
                    })
                    .and_then(|report| {
                        report.validate_for_role(&inv.role, inv.stage_count)?;
                        Ok(report)
                    }),
                // Container path only: this message goes into the retry prompt
                // and the live stream, so the host path must not appear.
                Err(err) => Err(Error::Report {
                    message: format!("missing report at {CLI_REPORT_PATH}: {}", err.kind()),
                }),
            };

            match parsed_report {
                Ok(report) => return Ok(report),
                Err(err) => {
                    let _ = std::fs::remove_file(&report_path);
                    current_prompt =
                        format!("{base_prompt}\n\n# Previous report was invalid\n{err}");
                    last_error = match err {
                        Error::Report { .. } => err,
                        other => Error::Report {
                            message: other.to_string(),
                        },
                    };
                }
            }
        }

        Err(last_error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use jiff::Timestamp;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::checkpoint::ReportStatus;
    use crate::config::WorkspaceMount;
    use crate::security::{SecretRedactor, SecretValue};
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
            recorded_at: Timestamp::now(),
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
            timestamp: Timestamp::now(),
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

    impl TurnState {
        async fn handle_sse_response(
            &mut self,
            items: &[SseItem],
            inv: &AgentInvocation,
            mcp: &McpGatekeeper,
            stream: &LiveStreamHub,
        ) -> Result<Option<AgentReport>> {
            let mut proc = ResponseStreamProcessor::default();
            let _ = proc.push_items(items, inv, stream)?;
            self.finish_response(proc, inv, mcp, stream).await
        }
    }

    fn tool_call_chunk(index: usize, id: Option<&str>, name: &str, arguments: &str) -> SseItem {
        SseItem::Chunk(ChatChunk {
            content: None,
            reasoning: None,
            tool_calls: vec![ToolCallDelta {
                index,
                id: id.map(str::to_owned),
                name: Some(name.to_string()),
                arguments: arguments.to_string(),
            }],
            finish_reason: Some("tool_calls".to_string()),
        })
    }

    fn content_chunk(text: &str) -> SseItem {
        SseItem::Chunk(ChatChunk {
            content: Some(text.to_string()),
            reasoning: None,
            tool_calls: Vec::new(),
            finish_reason: None,
        })
    }

    #[tokio::test]
    async fn submit_report_ends_turn() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        let items = vec![
            tool_call_chunk(
                0,
                Some("call_rep_1"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"Implemented feature\",\"artifacts\":[\"src/lib.rs\"]}",
            ),
            SseItem::Done,
        ];

        let report = state
            .handle_sse_response(&items, &inv, &mcp, &hub)
            .await
            .unwrap()
            .expect("valid submit_report should end turn");

        assert_eq!(report.status, ReportStatus::Complete);
        assert_eq!(report.summary, "Implemented feature");
        assert_eq!(report.artifacts, vec!["src/lib.rs".to_string()]);
    }

    #[tokio::test]
    async fn invalid_report_reprompted_once_then_error() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        // First invalid report: empty summary
        let invalid_1 = vec![
            tool_call_chunk(
                0,
                Some("call_bad_1"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"   \"}",
            ),
            SseItem::Done,
        ];
        let first_res = state
            .handle_sse_response(&invalid_1, &inv, &mcp, &hub)
            .await
            .unwrap();
        assert!(first_res.is_none());

        let last_msg = state.messages().last().unwrap();
        assert_eq!(last_msg["role"], "tool");
        assert_eq!(last_msg["tool_call_id"], "call_bad_1");
        assert!(
            last_msg["content"]
                .as_str()
                .unwrap()
                .contains("summary must not be empty")
        );

        // Second invalid report: status not allowed for Implementer
        let invalid_2 = vec![
            tool_call_chunk(
                0,
                Some("call_bad_2"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Confirmed\",\"summary\":\"wrong status for implementer\"}",
            ),
            SseItem::Done,
        ];
        let err = state
            .handle_sse_response(&invalid_2, &inv, &mcp, &hub)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Report { .. }),
            "expected Error::Report on second invalid report, got {err:?}"
        );

        // Also verify: invalid JSON on first attempt followed by valid report on second attempt succeeds
        let mut recovery_state = driver.new_turn_state(&inv);
        let bad_json = vec![
            tool_call_chunk(0, Some("call_bad_json"), SUBMIT_REPORT_TOOL, "{not-json"),
            SseItem::Done,
        ];
        assert!(
            recovery_state
                .handle_sse_response(&bad_json, &inv, &mcp, &hub)
                .await
                .unwrap()
                .is_none()
        );
        let valid_retry = vec![
            tool_call_chunk(
                0,
                Some("call_valid"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"Recovered on second try\"}",
            ),
            SseItem::Done,
        ];
        let recovered = recovery_state
            .handle_sse_response(&valid_retry, &inv, &mcp, &hub)
            .await
            .unwrap()
            .expect("second attempt with valid report should succeed");
        assert_eq!(recovered.summary, "Recovered on second try");
    }

    #[tokio::test]
    async fn no_tool_call_reprompted_once_then_error() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        let no_calls = vec![content_chunk("I am done.\n"), SseItem::Done];
        let first_res = state
            .handle_sse_response(&no_calls, &inv, &mcp, &hub)
            .await
            .unwrap();
        assert!(first_res.is_none());

        let last_msg = state.messages().last().unwrap();
        assert_eq!(last_msg["role"], "user");
        assert_eq!(last_msg["content"], "You must call submit_report.");

        let err = state
            .handle_sse_response(&no_calls, &inv, &mcp, &hub)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Report { .. }),
            "expected Error::Report on second missing tool call, got {err:?}"
        );
    }

    #[tokio::test]
    async fn disallowed_tool_returns_error_to_model_and_emits_event() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();

        let transport = Arc::new(crate::mcp::MockMcpTransport::new("git").with_tool(
            "status",
            serde_json::json!({"content": [{"type": "text", "text": "clean"}]}),
        ));
        let mut mcp = McpGatekeeper::new();
        mcp.register("git", transport.clone(), Vec::new());

        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1"); // role allows only ["filesystem:*"]
        let mut state = driver.new_turn_state(&inv);

        let items = vec![
            tool_call_chunk(0, Some("call_git_1"), "git__status", "{}"),
            SseItem::Done,
        ];
        let res = state
            .handle_sse_response(&items, &inv, &mcp, &hub)
            .await
            .unwrap();
        assert!(res.is_none());

        let tool_msg = state.messages().last().unwrap();
        assert_eq!(tool_msg["role"], "tool");
        assert_eq!(tool_msg["tool_call_id"], "call_git_1");
        assert!(
            tool_msg["content"].as_str().unwrap().starts_with("error:"),
            "expected tool message starting with 'error:', got {:?}",
            tool_msg["content"]
        );

        let events = hub.recent(10).unwrap();
        assert!(
            events.iter().any(|ev| {
                ev.kind == StreamEventKind::ToolCall
                    && ev.text.contains("git:status")
                    && ev.text.contains("denied")
            }),
            "expected ToolCall event containing 'denied', got {events:?}"
        );
        assert!(transport.calls().is_empty());

        // Also verify unmapped tool name and malformed JSON arguments produce error tool replies and denied events
        let bad_calls = vec![
            tool_call_chunk(0, Some("call_unmapped"), "unmapped_tool", "{}"),
            tool_call_chunk(
                1,
                Some("call_bad_args"),
                "filesystem__read_file",
                "{bad-json",
            ),
            SseItem::Done,
        ];
        assert!(
            state
                .handle_sse_response(&bad_calls, &inv, &mcp, &hub)
                .await
                .unwrap()
                .is_none()
        );
        let msgs = state.messages();
        assert!(
            msgs[msgs.len() - 2]["content"]
                .as_str()
                .unwrap()
                .starts_with("error:")
        );
        assert!(
            msgs[msgs.len() - 1]["content"]
                .as_str()
                .unwrap()
                .starts_with("error:")
        );
    }

    #[tokio::test]
    async fn allowed_tool_result_returned_to_model() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();

        let transport = Arc::new(crate::mcp::MockMcpTransport::new("filesystem").with_tool(
            "read_file",
            serde_json::json!({
                "content": [{"type": "text", "text": "pub fn answer() -> u32 { 42 }"}]
            }),
        ));
        let mut mcp = McpGatekeeper::new();
        mcp.register("filesystem", transport.clone(), Vec::new());

        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        let step1 = vec![
            tool_call_chunk(
                0,
                Some("call_fs_1"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            SseItem::Done,
        ];
        assert!(
            state
                .handle_sse_response(&step1, &inv, &mcp, &hub)
                .await
                .unwrap()
                .is_none()
        );

        let tool_msg = state.messages().last().unwrap();
        assert_eq!(tool_msg["role"], "tool");
        assert_eq!(tool_msg["tool_call_id"], "call_fs_1");
        assert_eq!(tool_msg["content"], "pub fn answer() -> u32 { 42 }");
        assert_eq!(transport.calls().len(), 1);

        let events = hub.recent(10).unwrap();
        assert!(
            events.iter().any(|ev| {
                ev.kind == StreamEventKind::ToolCall && ev.text == "filesystem:read_file ok"
            }),
            "expected 'filesystem:read_file ok' ToolCall event, got {events:?}"
        );
    }

    #[tokio::test]
    async fn tool_call_cap_enforced() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();

        let transport = Arc::new(crate::mcp::MockMcpTransport::new("filesystem").with_tool(
            "read_file",
            serde_json::json!({"content": [{"type": "text", "text": "ok"}]}),
        ));
        let mut mcp = McpGatekeeper::new();
        mcp.register("filesystem", transport, Vec::new());

        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new()).with_max_tool_calls(2);
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        for idx in 1..=2 {
            let items = vec![
                tool_call_chunk(
                    0,
                    Some(&format!("call_{idx}")),
                    "filesystem__read_file",
                    "{\"path\":\"src/lib.rs\"}",
                ),
                SseItem::Done,
            ];
            let res = state
                .handle_sse_response(&items, &inv, &mcp, &hub)
                .await
                .unwrap();
            assert!(res.is_none());
        }

        let third = vec![
            tool_call_chunk(
                0,
                Some("call_3"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            SseItem::Done,
        ];
        let err = state
            .handle_sse_response(&third, &inv, &mcp, &hub)
            .await
            .unwrap_err();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "coder-1");
                assert!(message.contains("2"));
            }
            other => panic!("expected Error::Agent on 3rd tool call, got {other:?}"),
        }

        // Verify that reaching max_tool_calls (2) still allows submit_report to finish the turn,
        // whereas 3 MCP calls before submit_report in the same batch still trips Error::Agent.
        let mut state_ok = driver.new_turn_state(&inv);
        let batch_within_cap = vec![
            tool_call_chunk(
                0,
                Some("call_a"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            tool_call_chunk(
                1,
                Some("call_b"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            tool_call_chunk(
                2,
                Some("call_rep"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"finished after 2 tool calls\"}",
            ),
            SseItem::Done,
        ];
        let report = state_ok
            .handle_sse_response(&batch_within_cap, &inv, &mcp, &hub)
            .await
            .unwrap()
            .expect("submit_report after 2 allowed tool calls should succeed");
        assert_eq!(report.summary, "finished after 2 tool calls");

        let mut state_over = driver.new_turn_state(&inv);
        let batch_over_cap = vec![
            tool_call_chunk(
                0,
                Some("call_1"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            tool_call_chunk(
                1,
                Some("call_2"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            tool_call_chunk(
                2,
                Some("call_3"),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            tool_call_chunk(
                3,
                Some("call_rep"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"should not reach\"}",
            ),
            SseItem::Done,
        ];
        let over_err = state_over
            .handle_sse_response(&batch_over_cap, &inv, &mcp, &hub)
            .await
            .unwrap_err();
        assert!(matches!(over_err, Error::Agent { .. }));
    }

    #[test]
    fn bearer_only_for_cloud_api() {
        let secrets = crate::security::InMemorySecretStore::new();
        secrets
            .set(
                "orrery.test",
                "testuser",
                &SecretValue::new("sk-cloud-token-99"),
            )
            .unwrap();

        let cloud_agent = AgentConfig {
            agent_id: "cloud-1".to_string(),
            display_name: "Cloud Agent".to_string(),
            backend: AgentBackend::CloudApi {
                endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                model: "cloud-coder".to_string(),
                temperature: 0.1,
                max_context_tokens: 16384,
                keychain_service: "orrery.test".to_string(),
                keychain_account: "testuser".to_string(),
            },
        };
        let local_agent = sample_local_agent("local-1");

        let cloud_hdr = auth_header_value(&cloud_agent, &secrets).unwrap().unwrap();
        assert!(cloud_hdr.is_sensitive());
        assert_eq!(cloud_hdr.to_str().unwrap(), "Bearer sk-cloud-token-99");

        let local_hdr = auth_header_value(&local_agent, &secrets).unwrap();
        assert!(local_hdr.is_none());
    }

    #[tokio::test]
    async fn cloud_missing_secret_is_keychain_error() {
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let cloud_agent = AgentConfig {
            agent_id: "cloud-1".to_string(),
            display_name: "Cloud Agent".to_string(),
            backend: AgentBackend::CloudApi {
                endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                model: "cloud-coder".to_string(),
                temperature: 0.1,
                max_context_tokens: 16384,
                keychain_service: "orrery.test".to_string(),
                keychain_account: "testuser".to_string(),
            },
        };

        let hdr_err = auth_header_value(&cloud_agent, secrets.as_ref()).unwrap_err();
        assert!(
            matches!(hdr_err, Error::Keychain(_)),
            "expected Error::Keychain from auth_header_value, got {hdr_err:?}"
        );

        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let driver = HttpChatDriver::new(secrets, Vec::new());

        let mut inv = sample_invocation("cloud-1");
        inv.agent = cloud_agent;

        let turn_err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        assert!(
            matches!(turn_err, Error::Keychain(_)),
            "expected Error::Keychain from run_turn, got {turn_err:?}"
        );
    }

    #[tokio::test]
    async fn output_emitted_per_line() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        let items = vec![
            content_chunk("hel"),
            content_chunk("lo\nwor"),
            content_chunk("ld"),
            tool_call_chunk(
                0,
                Some("call_1"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"done\"}",
            ),
            SseItem::Done,
        ];

        let report = state
            .handle_sse_response(&items, &inv, &mcp, &hub)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.summary, "done");

        let output_events: Vec<String> = hub
            .recent(10)
            .unwrap()
            .into_iter()
            .filter(|ev| ev.kind == StreamEventKind::Output)
            .map(|ev| ev.text)
            .collect();
        assert_eq!(
            output_events,
            vec!["hello".to_string(), "world".to_string()]
        );

        // Also verify: empty lines are not emitted, Reasoning is buffered per line, and >4096 bytes cuts at UTF-8 char boundary
        let hub2 = LiveStreamHub::new(
            tmp.path().join("logs2"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mut proc = ResponseStreamProcessor::default();
        // 4095 ASCII 'a' + 3-byte '—' (U+2014) = 4098 bytes; byte 4096 falls inside '—'
        let overlong = format!("{}\u{2014}tail", "a".repeat(4095));
        let edge_items = vec![
            content_chunk("\n\r\n"),
            content_chunk(&overlong),
            SseItem::Chunk(ChatChunk {
                content: None,
                reasoning: Some("think 1\nthink 2".to_string()),
                tool_calls: Vec::new(),
                finish_reason: None,
            }),
            SseItem::Done,
        ];
        let _ = proc.push_items(&edge_items, &inv, &hub2).unwrap();
        let _ = proc.finish(&inv, &hub2).unwrap();

        let recent2 = hub2.recent(10).unwrap();
        let out2: Vec<String> = recent2
            .iter()
            .filter(|ev| ev.kind == StreamEventKind::Output)
            .map(|ev| ev.text.clone())
            .collect();
        assert_eq!(out2, vec!["a".repeat(4095), "\u{2014}tail".to_string()]);
        let reas2: Vec<String> = recent2
            .iter()
            .filter(|ev| ev.kind == StreamEventKind::Reasoning)
            .map(|ev| ev.text.clone())
            .collect();
        assert_eq!(reas2, vec!["think 1".to_string(), "think 2".to_string()]);

        // Verify raw SSE byte chunks closing without trailing newline or data: [DONE]
        let hub3 = LiveStreamHub::new(
            tmp.path().join("logs3"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mut proc3 = ResponseStreamProcessor::default();
        assert!(
            !proc3
                .push_sse_bytes(
                    b"data: {\"choices\":[{\"delta\":{\"content\":\"line1\\nli\"}}]}\n",
                    &inv,
                    &hub3,
                )
                .unwrap()
        );
        // Second chunk has no trailing '\n' and no [DONE] before finish()
        assert!(
            !proc3
                .push_sse_bytes(
                    b"data: {\"choices\":[{\"delta\":{\"content\":\"ne2\"}}]}",
                    &inv,
                    &hub3,
                )
                .unwrap()
        );
        let (assistant_text, _) = proc3.finish(&inv, &hub3).unwrap();
        assert_eq!(assistant_text, "line1\nline2");
        let out3: Vec<String> = hub3
            .recent(10)
            .unwrap()
            .into_iter()
            .filter(|ev| ev.kind == StreamEventKind::Output)
            .map(|ev| ev.text)
            .collect();
        assert_eq!(out3, vec!["line1".to_string(), "line2".to_string()]);
    }

    #[tokio::test]
    async fn secret_split_across_deltas_redacted() {
        let tmp = TempDir::new().unwrap();
        let redactor = Arc::new(SecretRedactor::new());
        let secret_str = "sk-SPLITTEST-0123456789";
        redactor.register(&SecretValue::new(secret_str)).unwrap();

        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            redactor,
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        let items = vec![
            content_chunk("key sk-SPLIT"),
            content_chunk("TEST-0123456789 end\n"),
            tool_call_chunk(
                0,
                Some("call_1"),
                SUBMIT_REPORT_TOOL,
                "{\"status\":\"Complete\",\"summary\":\"done\"}",
            ),
            SseItem::Done,
        ];

        let _ = state
            .handle_sse_response(&items, &inv, &mcp, &hub)
            .await
            .unwrap();

        let recent = hub.recent(10).unwrap();
        assert!(!recent.is_empty());
        assert!(recent.iter().any(|ev| ev.text.contains("[REDACTED]")));
        for ev in &recent {
            assert!(
                !ev.text.contains(secret_str),
                "leaked secret in event: {ev:?}"
            );
        }
    }

    #[tokio::test]
    async fn empty_tool_call_id_gets_generated_id() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();

        let transport = Arc::new(crate::mcp::MockMcpTransport::new("filesystem").with_tool(
            "read_file",
            serde_json::json!({"content": [{"type": "text", "text": "file-body"}]}),
        ));
        let mut mcp = McpGatekeeper::new();
        mcp.register("filesystem", transport, Vec::new());

        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let driver = HttpChatDriver::new(secrets, Vec::new());
        let inv = sample_invocation("coder-1");
        let mut state = driver.new_turn_state(&inv);

        let items = vec![
            tool_call_chunk(
                0,
                None,
                "filesystem__read_file",
                "{\"path\":\"Cargo.toml\"}",
            ),
            SseItem::Done,
        ];
        let res = state
            .handle_sse_response(&items, &inv, &mcp, &hub)
            .await
            .unwrap();
        assert!(res.is_none());

        let msgs = state.messages();
        assert_eq!(msgs.len(), 4); // system, user, assistant, tool
        let assistant_msg = &msgs[2];
        let tool_msg = &msgs[3];

        let generated_id = assistant_msg["tool_calls"][0]["id"]
            .as_str()
            .expect("assistant tool_calls[0].id should be a string")
            .to_string();
        assert!(!generated_id.is_empty());
        assert_eq!(generated_id, "orrery_call_1");
        assert_eq!(tool_msg["role"], "tool");
        assert_eq!(tool_msg["tool_call_id"], generated_id);

        // Second empty-id tool call in the same turn gets the next sequential id
        let items_2 = vec![
            tool_call_chunk(
                0,
                Some(""),
                "filesystem__read_file",
                "{\"path\":\"src/lib.rs\"}",
            ),
            SseItem::Done,
        ];
        assert!(
            state
                .handle_sse_response(&items_2, &inv, &mcp, &hub)
                .await
                .unwrap()
                .is_none()
        );
        let msgs = state.messages();
        assert_eq!(msgs[4]["tool_calls"][0]["id"], "orrery_call_2");
        assert_eq!(msgs[5]["tool_call_id"], "orrery_call_2");
    }

    fn sample_cli_invocation(
        agent_id: &str,
        workspace_mount: WorkspaceMount,
        timeout_secs: u64,
        secret: Option<(&str, &str, &str)>,
    ) -> AgentInvocation {
        let (keychain_service, keychain_account, secret_env) = match secret {
            Some((svc, acct, env_key)) => (
                Some(svc.to_string()),
                Some(acct.to_string()),
                Some(env_key.to_string()),
            ),
            None => (None, None, None),
        };
        let role = sample_role(RoleKind::Implementer);
        let backend = AgentBackend::AgentCli {
            image: "orrery/cli-agent:1.0".to_string(),
            command: vec!["agent-cli".to_string(), "--stdin".to_string()],
            workspace_mount,
            memory_limit_mb: 1024,
            timeout_secs,
            keychain_service,
            keychain_account,
            secret_env,
        };
        let agent = AgentConfig {
            agent_id: agent_id.to_string(),
            display_name: format!("CLI {agent_id}"),
            backend: backend.clone(),
        };
        let step = sample_step();
        let system_prompt = build_system_prompt(&role, "Write clean code.", &backend);
        let user_prompt = build_user_prompt(&step, None, &[], &[]);
        AgentInvocation {
            run_id: Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0),
            step_number: 1,
            stage_index: 1,
            stage_count: 3,
            round: 1,
            role,
            agent,
            system_prompt,
            user_prompt,
            tools: Vec::new(),
            write_scope: WriteScope::anywhere(),
        }
    }

    fn find_mount_host<'a>(spec: &'a ContainerSpec, container_target: &str) -> &'a std::path::Path {
        spec.mounts
            .iter()
            .find_map(|m| match m {
                crate::docker::MountSpec::Bind {
                    host, container, ..
                } if container == container_target => Some(host.as_path()),
                crate::docker::MountSpec::Bind { .. } => None,
            })
            .expect("expected mount target not found")
    }

    #[tokio::test]
    async fn valid_report_accepted() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        backend.set_run_script(Box::new(|spec, _stdin| {
            let out_host = find_mount_host(spec, "/orrery/out");
            let report = AgentReport {
                status: ReportStatus::Complete,
                summary: "Implemented CLI container driver.".to_string(),
                feedback: None,
                artifacts: vec!["src/llm.rs".to_string()],
                target_stage: None,
                allocations: Vec::new(),
            };
            let toml_str = toml::to_string(&report).unwrap();
            std::fs::write(out_host.join("report.toml"), toml_str).unwrap();
            (Vec::new(), 0)
        }));

        let driver = CliContainerDriver::new(backend.clone(), secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 30, None);
        let expected_prompt = format!("{}\n\n{}", inv.system_prompt, inv.user_prompt);

        let report = driver.run_turn(&inv, &mcp, &hub).await.unwrap();
        assert_eq!(report.status, ReportStatus::Complete);
        assert_eq!(report.summary, "Implemented CLI container driver.");

        let io_dir = agent_io_dir(tmp.path(), &inv.run_id, &inv.agent.agent_id);
        let prompt_file = io_dir.join("in").join("prompt.md");
        assert!(prompt_file.is_file());
        let written_prompt = std::fs::read_to_string(&prompt_file).unwrap();
        assert_eq!(written_prompt, expected_prompt);

        let calls = backend.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls[0],
            crate::docker::MockCall::Remove(cli_container_name(&inv.run_id, &inv.agent.agent_id))
        );
        match &calls[1] {
            crate::docker::MockCall::RunToCompletion {
                name, stdin_len, ..
            } => {
                assert_eq!(name, &cli_container_name(&inv.run_id, &inv.agent.agent_id));
                assert_eq!(*stdin_len, written_prompt.len());
            }
            other => panic!("expected RunToCompletion, got {other:?}"),
        }
        assert_eq!(
            calls[2],
            crate::docker::MockCall::Remove(cli_container_name(&inv.run_id, &inv.agent.agent_id))
        );
    }

    #[tokio::test]
    async fn output_lines_streamed() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        backend.set_run_script(Box::new(|spec, _stdin| {
            let out_host = find_mount_host(spec, "/orrery/out");
            let report = AgentReport {
                status: ReportStatus::Complete,
                summary: "Streamed output lines.".to_string(),
                feedback: None,
                artifacts: Vec::new(),
                target_stage: None,
                allocations: Vec::new(),
            };
            std::fs::write(
                out_host.join("report.toml"),
                toml::to_string(&report).unwrap(),
            )
            .unwrap();
            (
                vec!["compiling crate".to_string(), "all tests green".to_string()],
                0,
            )
        }));

        let driver = CliContainerDriver::new(backend, secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::None, 30, None);
        let report = driver.run_turn(&inv, &mcp, &hub).await.unwrap();
        assert_eq!(report.status, ReportStatus::Complete);

        let events = hub.recent(10).unwrap();
        let output_texts: Vec<&str> = events
            .iter()
            .filter(|e| e.kind == StreamEventKind::Output)
            .map(|e| e.text.as_str())
            .collect();
        assert_eq!(output_texts, vec!["compiling crate", "all tests green"]);
    }

    #[tokio::test]
    async fn missing_report_reprompted_then_error() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        backend.set_run_script(Box::new(|_spec, _stdin| {
            (vec!["ran without report".into()], 0)
        }));

        let driver = CliContainerDriver::new(backend.clone(), secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 30, None);
        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        assert!(matches!(err, Error::Report { .. }));

        let run_calls = backend
            .calls()
            .into_iter()
            .filter(|c| matches!(c, crate::docker::MockCall::RunToCompletion { .. }))
            .count();
        assert_eq!(run_calls, 2);
    }

    #[tokio::test]
    async fn stale_report_not_reused() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());
        backend.set_run_script(Box::new(|_spec, _stdin| (Vec::new(), 0)));

        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 30, None);
        let out_dir = agent_io_dir(tmp.path(), &inv.run_id, &inv.agent.agent_id).join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let stale = AgentReport {
            status: ReportStatus::Complete,
            summary: "stale report from an earlier turn".to_string(),
            feedback: None,
            artifacts: Vec::new(),
            target_stage: None,
            allocations: Vec::new(),
        };
        std::fs::write(
            out_dir.join("report.toml"),
            toml::to_string(&stale).unwrap(),
        )
        .unwrap();

        let driver = CliContainerDriver::new(backend, secrets, tmp.path().to_path_buf());
        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        assert!(matches!(err, Error::Report { .. }));
        assert!(!out_dir.join("report.toml").exists());
    }

    #[tokio::test]
    async fn missing_report_error_uses_container_path() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());
        let prompts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let prompts_clone = prompts.clone();
        backend.set_run_script(Box::new(move |_spec, stdin| {
            prompts_clone
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(stdin).into_owned());
            (Vec::new(), 0)
        }));

        let driver = CliContainerDriver::new(backend, secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 30, None);
        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();

        let host_root = tmp.path().to_string_lossy().into_owned();
        let prompts = prompts.lock().unwrap().clone();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[1].contains(CLI_REPORT_PATH));
        assert!(!prompts[1].contains(&host_root));
        assert!(!err.to_string().contains(&host_root));
        for ev in hub.recent(100).unwrap() {
            assert!(!ev.text.contains(&host_root));
        }
    }

    #[tokio::test]
    async fn invalid_then_valid_report_accepted() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        let counter = Arc::new(AtomicUsize::new(0));
        let recorded_prompts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let counter_clone = counter.clone();
        let prompts_clone = recorded_prompts.clone();

        backend.set_run_script(Box::new(move |spec, stdin| {
            let call_idx = counter_clone.fetch_add(1, Ordering::SeqCst);
            prompts_clone
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(stdin).into_owned());

            let out_host = find_mount_host(spec, "/orrery/out");
            if call_idx == 0 {
                std::fs::write(out_host.join("report.toml"), "not = [valid toml").unwrap();
            } else {
                let report = AgentReport {
                    status: ReportStatus::Complete,
                    summary: "Fixed report on second attempt.".to_string(),
                    feedback: None,
                    artifacts: Vec::new(),
                    target_stage: None,
                    allocations: Vec::new(),
                };
                std::fs::write(
                    out_host.join("report.toml"),
                    toml::to_string(&report).unwrap(),
                )
                .unwrap();
            }
            (Vec::new(), 0)
        }));

        let driver = CliContainerDriver::new(backend, secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 30, None);
        let report = driver.run_turn(&inv, &mcp, &hub).await.unwrap();
        assert_eq!(report.status, ReportStatus::Complete);
        assert_eq!(report.summary, "Fixed report on second attempt.");

        let prompts = recorded_prompts.lock().unwrap().clone();
        assert_eq!(prompts.len(), 2);
        assert!(!prompts[0].contains("Previous report was invalid"));
        assert!(prompts[1].contains("Previous report was invalid"));

        let io_dir = agent_io_dir(tmp.path(), &inv.run_id, &inv.agent.agent_id);
        let disk_prompt = std::fs::read_to_string(io_dir.join("in").join("prompt.md")).unwrap();
        assert!(disk_prompt.contains("Previous report was invalid"));
    }

    #[tokio::test]
    async fn timeout_is_agent_error_and_removes_container() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());
        backend.set_run_delay(Duration::from_secs(2));

        let driver = CliContainerDriver::new(backend.clone(), secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 1, None);
        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "cli-coder");
                assert!(message.contains("timed out after 1s"));
            }
            other => panic!("expected Error::Agent, got {other:?}"),
        }

        let expected_name = cli_container_name(&inv.run_id, &inv.agent.agent_id);
        assert!(
            backend
                .calls()
                .contains(&crate::docker::MockCall::Remove(expected_name))
        );
    }

    #[tokio::test]
    async fn secret_env_injected_as_secret() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        secrets
            .set(
                "orrery.test",
                "testuser",
                &SecretValue::new("sk-ant-test-secret-value-1234"),
            )
            .unwrap();
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        backend.set_run_script(Box::new(|spec, _stdin| {
            let out_host = find_mount_host(spec, "/orrery/out");
            let report = AgentReport {
                status: ReportStatus::Complete,
                summary: "Used secret env.".to_string(),
                feedback: None,
                artifacts: Vec::new(),
                target_stage: None,
                allocations: Vec::new(),
            };
            std::fs::write(
                out_host.join("report.toml"),
                toml::to_string(&report).unwrap(),
            )
            .unwrap();
            (Vec::new(), 0)
        }));

        let driver = CliContainerDriver::new(backend.clone(), secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation(
            "cli-coder",
            WorkspaceMount::ReadOnly,
            30,
            Some(("orrery.test", "testuser", "ANTHROPIC_API_KEY")),
        );
        let report = driver.run_turn(&inv, &mcp, &hub).await.unwrap();
        assert_eq!(report.status, ReportStatus::Complete);

        let calls = backend.calls();
        match &calls[1] {
            crate::docker::MockCall::RunToCompletion {
                secret_env,
                plain_env,
                ..
            } => {
                assert_eq!(secret_env, &vec!["ANTHROPIC_API_KEY".to_string()]);
                assert!(
                    !plain_env
                        .iter()
                        .any(|entry| entry.contains("ANTHROPIC_API_KEY"))
                );
            }
            other => panic!("expected RunToCompletion, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_secret_fails_before_run() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        let driver = CliContainerDriver::new(backend.clone(), secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation(
            "cli-coder",
            WorkspaceMount::ReadOnly,
            30,
            Some(("orrery.test", "testuser", "ANTHROPIC_API_KEY")),
        );
        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        assert!(matches!(err, Error::Keychain(_)));

        let run_calls = backend
            .calls()
            .into_iter()
            .filter(|c| matches!(c, crate::docker::MockCall::RunToCompletion { .. }))
            .count();
        assert_eq!(run_calls, 0);
    }

    #[tokio::test]
    async fn mask_dir_exists_and_empty_during_run() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        let mask_verified = Arc::new(AtomicUsize::new(0));
        let mask_verified_clone = mask_verified.clone();

        backend.set_run_script(Box::new(move |spec, _stdin| {
            let mask_host = find_mount_host(spec, "/workspace/.orrery");
            assert!(mask_host.is_dir());
            assert!(std::fs::read_dir(mask_host).unwrap().next().is_none());
            mask_verified_clone.fetch_add(1, Ordering::SeqCst);

            let out_host = find_mount_host(spec, "/orrery/out");
            let report = AgentReport {
                status: ReportStatus::Complete,
                summary: "Verified mask mount.".to_string(),
                feedback: None,
                artifacts: Vec::new(),
                target_stage: None,
                allocations: Vec::new(),
            };
            std::fs::write(
                out_host.join("report.toml"),
                toml::to_string(&report).unwrap(),
            )
            .unwrap();
            (Vec::new(), 0)
        }));

        let driver = CliContainerDriver::new(backend, secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadWrite, 30, None);
        let report = driver.run_turn(&inv, &mcp, &hub).await.unwrap();
        assert_eq!(report.status, ReportStatus::Complete);
        assert_eq!(mask_verified.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn send_timeout_is_agent_error() {
        use tokio::io::AsyncReadExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let inference = vec![sample_inference("local-1", &format!("http://{addr}"))];
        let driver =
            HttpChatDriver::new(secrets, inference).with_idle_timeout(Duration::from_secs(1));
        let inv = sample_invocation("coder-1");

        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        server_task.abort();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "coder-1");
                assert_eq!(message, "no response from model within 1s");
            }
            other => panic!("expected Error::Agent, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stalled_stream_is_agent_error() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n")
                .await
                .unwrap();
            sock.write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n")
                .await
                .unwrap();
            sock.flush().await.unwrap();
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let inference = vec![sample_inference("local-1", &format!("http://{addr}"))];
        let driver =
            HttpChatDriver::new(secrets, inference).with_idle_timeout(Duration::from_secs(1));
        let inv = sample_invocation("coder-1");

        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        server_task.abort();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "coder-1");
                assert_eq!(message, "no response from model within 1s");
            }
            other => panic!("expected Error::Agent, got {other:?}"),
        }
    }

    #[test]
    fn auth_header_marked_sensitive() {
        let secrets = crate::security::InMemorySecretStore::new();
        secrets
            .set(
                "orrery.test",
                "testuser",
                &SecretValue::new("test-token-123"),
            )
            .unwrap();

        let cloud_agent = AgentConfig {
            agent_id: "cloud-1".to_string(),
            display_name: "Cloud Agent".to_string(),
            backend: AgentBackend::CloudApi {
                endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
                model: "cloud-coder".to_string(),
                temperature: 0.1,
                max_context_tokens: 16384,
                keychain_service: "orrery.test".to_string(),
                keychain_account: "testuser".to_string(),
            },
        };

        let hdr = auth_header_value(&cloud_agent, &secrets).unwrap();
        let v = hdr.expect("expected Some header value for CloudApi");
        assert!(v.is_sensitive());
        assert_eq!(v.to_str().unwrap(), "Bearer test-token-123");
    }

    #[test]
    fn estimate_tokens_bytes_div_4() {
        let s = serde_json::json!("abcd");
        assert_eq!(serde_json::to_string(&s).unwrap().len(), 6);
        assert_eq!(estimate_tokens(&s), 2);

        let obj = serde_json::json!({});
        assert_eq!(serde_json::to_string(&obj).unwrap().len(), 2);
        assert_eq!(estimate_tokens(&obj), 1);
    }

    #[tokio::test]
    async fn context_preflight_rejects_before_request() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let inference = vec![sample_inference("local-1", "http://127.0.0.1:9")];
        let driver = HttpChatDriver::new(secrets, inference);

        let mut inv = sample_invocation("coder-1");
        inv.agent.backend = AgentBackend::LocalModel {
            inference_id: "local-1".to_string(),
            model: "qwen-coder".to_string(),
            temperature: 0.2,
            max_context_tokens: 10,
        };

        let err = driver.run_turn(&inv, &mcp, &hub).await.unwrap_err();
        match err {
            Error::Agent { agent_id, message } => {
                assert_eq!(agent_id, "coder-1");
                assert!(
                    message.starts_with("prompt is about "),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("over max_context_tokens 10"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected Error::Agent, got {other:?}"),
        }
    }

    #[test]
    fn unmappable_tool_left_out_of_request() {
        let mut inv = sample_invocation("coder-1");
        inv.tools = vec![
            McpToolSchema {
                server_id: "fs".to_string(),
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            },
            McpToolSchema {
                server_id: "fs".to_string(),
                name: "read.file".to_string(),
                description: "Read a file with dot".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            },
        ];

        let messages = vec![
            serde_json::json!({"role": "system", "content": inv.system_prompt}),
            serde_json::json!({"role": "user", "content": inv.user_prompt}),
        ];
        let req = build_chat_request(&inv, &messages);
        let names: Vec<&str> = req["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["fs__read_file", "submit_report"]);
    }

    #[tokio::test]
    async fn cli_driver_removes_stale_container_first() {
        let tmp = TempDir::new().unwrap();
        let hub = LiveStreamHub::new(
            tmp.path().join("logs"),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .unwrap();
        let mcp = McpGatekeeper::new();
        let secrets = Arc::new(crate::security::InMemorySecretStore::new());
        let backend = Arc::new(crate::docker::MockContainerBackend::new());

        backend.set_run_script(Box::new(|spec, _stdin| {
            let out_host = find_mount_host(spec, "/orrery/out");
            let report = AgentReport {
                status: ReportStatus::Complete,
                summary: "Clean run.".to_string(),
                feedback: None,
                artifacts: Vec::new(),
                target_stage: None,
                allocations: Vec::new(),
            };
            std::fs::write(
                out_host.join("report.toml"),
                toml::to_string(&report).unwrap(),
            )
            .unwrap();
            (Vec::new(), 0)
        }));

        let driver = CliContainerDriver::new(backend.clone(), secrets, tmp.path().to_path_buf());
        let inv = sample_cli_invocation("cli-coder", WorkspaceMount::ReadOnly, 30, None);
        let expected_name = cli_container_name(&inv.run_id, &inv.agent.agent_id);

        driver.run_turn(&inv, &mcp, &hub).await.unwrap();

        let calls = backend.calls();
        assert!(calls.len() >= 2);
        assert_eq!(
            calls[0],
            crate::docker::MockCall::Remove(expected_name.clone())
        );
        assert!(matches!(
            &calls[1],
            crate::docker::MockCall::RunToCompletion { name, .. } if name == &expected_name
        ));
    }
}
