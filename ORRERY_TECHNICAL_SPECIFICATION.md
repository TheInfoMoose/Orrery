# Orrery — Technical Specification

> **Source of truth.** This spec is authoritative. §9 is the binding interface reference: exact signatures and algorithms the plan steps implement as written. `STEPPED_IMPLEMENTATION_PLAN.md` references its sections by number and must not restate or change any type, field or function defined here. If the plan and this spec disagree, this spec wins; stop and report the conflict.
>
> Product intent and recorded decisions: `Orrery_Initial.md` (see its "Decisions" section).

## 1. Summary

**Orrery** is a Rust CLI + TUI that stages and runs multi-agent LLM workflows. It ingests a stepped implementation plan written in Markdown, and for each plan step drives a workflow of **stages** (columns): a single **Orchestrator** stage, one or more implementation stages, and usually a **Validator** stage. Orrery enforces per-role guardrails, mediates MCP tool access, manages local container/model resources, and checkpoints every transition to disk as validated TOML.

Orrery **only orchestrates**. Version control, builds and similar actions are performed by MCP tools (e.g. Nebulous, a git MCP server) that Orrery calls at configured lifecycle hooks. The core is **domain-neutral**: software development is the first use case, but no role names, tool names or language toolchains are hardcoded in `orrery-core`. They live in role/template data.

### 1.1 Terminology

| Term | Meaning |
|---|---|
| **Plan step** | One section of the implementation plan Markdown, found by the step marker (§5.4). |
| **Stage** | One workflow column: a role plus 1..=15 agents. A workflow has 2..=15 stages. |
| **Round** | One orchestrator turn plus the stage turn it dispatches. |
| **Cycle count** | In Loop mode, the number of plan steps that must pass before stopping. `0` = until out of plan steps. Ignored in Run mode. |
| **Gate** | An ordered list of MCP tools Orrery runs itself (no LLM) to check a plan step, e.g. a test runner. |
| **Operator** | The human running Orrery. |

---

## 2. Core Principles

1. **CLI + TUI parity.** `orrery` or `orrery [PATH]` (no subcommand) launches the TUI for `PATH` (default `.`). `orrery <SUBCOMMAND>` runs headless. Every TUI action has a CLI equivalent (§7). The reverse is not required.
2. **Three agent backends** (§4.3), any role may use any backend:
   - `LocalModel` — an OpenAI-compatible chat endpoint served locally (primary target: a llama.cpp `llama-server` the operator already runs, single-model or router mode). Orrery runs the tool loop.
   - `AgentCli` — a containerized agent CLI (e.g. Claude Code, Gemini CLI) run non-interactively. The CLI runs its own tool loop.
   - `CloudApi` — a remote OpenAI-compatible chat endpoint. Orrery runs the tool loop.
   - The default 3-stage template assigns local models to implementation and a stronger backend to orchestrator and validator.
3. **Resource management.**
   - Agents from different stages never run at the same time.
   - Within a stage, up to `max_concurrent_agents` run in parallel, except **writer stages** (§5.10.3), which run one agent at a time unless the orchestrator assigns disjoint write scopes.
   - Orrery attaches to the operator's existing model servers. Between stages it frees what the next stage doesn't need: unload through the router API, or stop a single-model server Orrery itself started (§9.6, §9.6a). It never creates or configures model servers.
4. **Zero-footprint secrets.** API keys live only in the OS keychain (§5.3). Configs reference them by `(keychain_service, keychain_account)`. In memory they are `SecretValue` (zeroized on drop, redacted on format). Every stream event and log line passes through `SecretRedactor`.
5. **Indirect agent communication via validated reports.** Each agent turn starts from a fresh context. Agents return a structured report (§5.5); Orrery validates it, stamps the fields it already knows, and persists it atomically as TOML in `.orrery/checkpoints/`. Interrupted runs resume from the last checkpoint.
6. **Deterministic pass rule.** A plan step passes only when Orrery's own rule says so (§5.10.2). The orchestrator cannot override a validator denial or a failed gate.
7. **Central MCP gatekeeper.** For `LocalModel` and `CloudApi` agents, Orrery is the only MCP client. Tools shown to an agent are filtered by the role's allowlist, and every call is re-checked before it reaches a transport (§5.8). For `AgentCli` agents, guardrails come from the CLI's own tool configuration plus container isolation and mounts; the MCP gatekeeper does **not** cover a CLI's built-in tools (§5.7.2).
8. **Isolated live stream.** Agent output streams to the TUI and to `.orrery/logs/`. It is never fed back into any agent prompt or exposed through any tool.

---

## 3. Workspace & Dependencies

Resolver 3, edition 2024, `rust-version` = installed stable toolchain at Step 1.

```text
Orrery/
├── Cargo.toml                        # Workspace manifest, [workspace.dependencies], [workspace.lints]
├── Cargo.lock                        # Committed
├── deny.toml                         # cargo-deny policy (§8)
├── Orrery_Initial.md
├── ORRERY_TECHNICAL_SPECIFICATION.md
├── STEPPED_IMPLEMENTATION_PLAN.md
└── crates/
    ├── orrery-core/src/
    │   ├── lib.rs           # Module exports, Error, Result
    │   ├── config.rs        # Project/Role/Agent/MCP models, ConfigStore
    │   ├── security.rs      # SecretValue, SecretStore, SecretRedactor
    │   ├── plan.rs          # Plan step parser
    │   ├── checkpoint.rs    # Reports, WorkflowCheckpoint, CheckpointManager
    │   ├── stream_log.rs    # LiveStreamHub, rotating log
    │   ├── docker.rs        # ContainerBackend, ResourceManager
    │   ├── mcp.rs           # McpTransport, McpGatekeeper
    │   ├── llm.rs           # Prompt builders, AgentDriver implementations
    │   └── engine.rs        # Workflow state machine
    ├── orrery-tui/src/
    │   ├── lib.rs           # Exports, run_tui
    │   ├── theme.rs         # Nebulous palette, banner
    │   ├── layout.rs        # UiLayoutRegions
    │   ├── app.rs           # App state, AppAction, input handling
    │   ├── widgets.rs       # Leaf widget rendering
    │   └── bridge.rs        # Engine ↔ TUI channel wiring
    └── orrery-cli/src/
        ├── main.rs          # Entry point
        ├── args.rs          # clap definitions
        └── commands.rs      # Command handlers
```

### 3.1 Dependencies

Versions and features below were resolved together, built, and checked with `cargo deny` on 2026-11-12 (latest stable releases at that date). Plan steps add each crate with exactly this line; they do not look up versions or feature names. Never edit `Cargo.lock` by hand.

| Crate | `[workspace.dependencies]` line | Purpose / constraints | Added in |
|---|---|---|---|
| `thiserror`, `serde`, `serde_json`, `toml`, `tempfile`, `dirs` | already present | Errors, configs, fixtures, XDG paths. `deny_unknown_fields` on every on-disk struct | Steps 1–3 |
| `secrecy` | `secrecy = "0.10.3"` | `SecretString` | Step 4 |
| `zeroize` | `zeroize = "1.9.1"` | Secret memory hygiene | Step 4 |
| `keyring-core` | `keyring-core = "1.0.1"` | Credential-store API (`Entry`, `CredentialStore`, `Error`) | Step 4 |
| `dbus-secret-service-keyring-store` | `dbus-secret-service-keyring-store = { version = "1.0.1", features = ["crypto-rust"] }` | Secret Service via libdbus. Build host needs `libdbus-1-dev` + `pkg-config`. **Not** the zbus store: zbus 5 pulls `zvariant`/`zbus_names`, flagged unmaintained (RUSTSEC-2026-0370/0371/0372) | Step 4 |
| `linux-keyutils-keyring-store` | `linux-keyutils-keyring-store = "1.0.0"` | Kernel keyutils fallback | Step 4 |
| `sha2` | `sha2 = "0.11.1"` | FIPS 180-4 SHA-256. Hex encoding is hand-written (no `hex` crate: last release 2021) | Step 6 |
| `chrono` | `chrono = { version = "0.4.45", default-features = false, features = ["clock", "std", "serde"] }` | Timestamps | Step 7 |
| `uuid` | `uuid = { version = "1.29.2", features = ["v4", "serde"] }` | Run IDs | Step 8 |
| `tokio` | `tokio = { version = "1.54.1", features = ["sync"] }` — features grow per step (§3.2) | Runtime | Step 9 |
| `async-trait` | `async-trait = "0.1.92"` | `dyn`-compatible async traits (native `async fn` in traits is not `dyn`-compatible) | Step 10 |
| `reqwest` | `reqwest = { version = "0.13.5", default-features = false, features = ["rustls", "json", "stream"] }` | HTTP. `rustls` uses aws-lc-rs. No `openssl-sys` | Step 11 |
| `rmcp` | `rmcp = { version = "3.9.0", default-features = false, features = ["client", "transport-child-process", "transport-streamable-http-client-reqwest", "reqwest"] }` | MCP client | Step 11 |
| `futures-util` | `futures-util = "0.3.34"` | `StreamExt`, `join_all` | Step 15 |
| `bollard` | `bollard = "0.21.1"` | Docker Engine API (default features `http`, `pipe`) | Step 15 |
| `tokio-util` | `tokio-util = "0.7.20"` | `CancellationToken` (no features needed) | Step 20 |
| `ratatui` | `ratatui = "0.30.2"` | TUI | Step 27 |
| `crossterm` | `crossterm = { version = "0.29.0", features = ["event-stream"] }` | Terminal I/O, `EventStream` | Step 27 |
| `clap` | `clap = { version = "4.6.7", features = ["derive"] }` | CLI | Step 33 |
| `rpassword` | `rpassword = "7.5.5"` | No-echo secret prompt for `orrery secret set` | Step 35 |

There is no `keyring` facade crate: `keyring` 4.x's `v1::Entry` hard-codes the zbus store and has no keyutils fallback, and 3.x is an older release (not permitted).

### 3.2 `tokio` features by step

The workspace line's `features` list is replaced (not appended) at these steps:

| Step | `features = [...]` |
|---|---|
| 9 | `["sync"]` |
| 10 | `["sync", "macros", "rt"]` |
| 11 | `["sync", "macros", "rt", "process", "io-util", "time"]` |
| 33 | `["sync", "macros", "rt", "rt-multi-thread", "process", "io-util", "time", "signal"]` |

---

## 4. On-Disk Layout & Schemas

```text
~/.config/orrery/                 # Global catalog (optional). Project files override by id.
├── roles/<role_id>.toml + .md
├── agents/<agent_id>.toml
└── mcp/servers.toml

<project_root>/.orrery/
├── project.toml
├── roles/<role_id>.toml + .md
├── agents/<agent_id>.toml
├── mcp/servers.toml
├── checkpoints/
│   ├── workflow_state.toml                       # WorkflowCheckpoint
│   └── step_001/round_02_stage_01_<agent_id>.toml # StampedReport
├── agent_io/<run_id>/<agent_id>/                 # AgentCli mounts: in/ (ro), out/ (rw), mask/ (empty, ro over /workspace/.orrery). Deleted after the stage.
└── logs/live_stream.log, .log.1, .log.2
```

All on-disk writes are atomic (write temp file in the same directory, `fsync`, rename).

### 4.1 `project.toml` — `ProjectConfig`

```toml
project_name = "my-service"
plan_path = "STEPPED_IMPLEMENTATION_PLAN.md"
step_marker = "## Step "          # Line prefix that starts a plan step (§5.4)
execution_mode = "Run"            # "Run" | "Loop"
cycle_count = 0                   # Loop only: plan steps to pass. 0 = until out of steps
max_concurrent_agents = 2         # Upper bound for parallel agents within one stage
max_step_retries = 3              # Validator/gate failures per plan step before escalating to the operator
validation_mode = "Auto"          # "Auto" | "Agents" | "GateAndOperator" (§5.10.2)
gate_tools = []                   # Ordered MCP tools run by Orrery, e.g. ["cargo:test"]

[[stages]]
role_id = "orchestrator"
agent_ids = ["cloud-orchestrator"]

[[stages]]
role_id = "coder"
agent_ids = ["local-coder"]

[[stages]]
role_id = "validator"
agent_ids = ["cloud-validator"]

[hooks]                           # Optional. Each names an MCP tool (§5.10.4)
on_step_start = { tool = "git:create_branch", on_failure = "Halt" }
on_step_pass  = { tool = "git:commit", on_failure = "Halt" }
on_step_retry = { tool = "git:restore", on_failure = "Warn" }
on_halt       = { tool = "git:stash", on_failure = "Warn" }
```

Stage index = position in `stages` (0-based). There is no separate `column_index` field.

**`ProjectConfig::validate(&self, roles, agents) -> Result<()>` invariants:**
- `stages.len()` in `2..=15`. `ProjectConfig::default_template` produces 3 stages.
- **Exactly one** stage has an orchestrator role, and it is stage 0. It has exactly 1 agent when running (0 allowed while staging).
- Every other stage has `0..=15` agents while staging and `1..=15` when running (`validate_for_run`).
- Every referenced `role_id` / `agent_id` exists.
- `step_marker` is non-empty and contains at least one non-whitespace character.
- `plan_path` resolves inside `project_root` (no `..`, no symlink escape).
- `validation_mode` resolution (`ProjectConfig::effective_validation_mode`): `Auto` → `Agents` if any stage's role has `is_validator = true`, otherwise `GateAndOperator`.
- `GateAndOperator` with empty `gate_tools` is allowed (operator-only validation) but `validate` returns a warning list that the CLI/TUI shows.
- `max_concurrent_agents >= 1`.

### 4.2 Roles — `roles/<role_id>.toml` + `.md`

```toml
role_id = "coder"
display_name = "Implementation"
kind = "Implementer"              # "Orchestrator" | "Implementer" | "Validator"
rules_file = "coder.md"           # Markdown appended to the system prompt
allowed_mcp_tools = ["filesystem:read_file", "filesystem:write_file", "cargo:check"]
```

`RoleConfig` exposes `is_orchestrator()` / `is_validator()` derived from `kind` (one field, no conflicting booleans). A role is a **writer** if any of its allowed tools is listed in a server's `write_tools` (§4.4), or if any of its agents uses `AgentCli` with `workspace_mount = "ReadWrite"`.

### 4.3 Agents — `agents/<agent_id>.toml`

Common fields: `agent_id`, `display_name`, `[backend]`.

```toml
# LocalModel — served by an inference server the operator already runs
agent_id = "local-coder"
display_name = "Local Coder"
[backend]
type = "LocalModel"
inference_id = "coder-14b"         # Refers to [[inference]] in project.toml or global config
model = "Qwen2.5-Coder-14B-Instruct-Q4_K_M"  # Router mode: an id from GET /v1/models. Single mode: informational
temperature = 0.1
max_context_tokens = 32768
```

```toml
# AgentCli — one container per agent per stage, removed afterwards
agent_id = "cli-validator"
display_name = "CLI Validator"
[backend]
type = "AgentCli"
image = "registry.example.com/agents/claude-code@sha256:<digest>"  # Digest or exact tag; never `latest`
command = ["claude", "-p", "--output-format", "stream-json"]       # Prompt is passed on stdin
workspace_mount = "ReadOnly"       # "None" | "ReadOnly" | "ReadWrite"
memory_limit_mb = 4096
timeout_secs = 1800
keychain_service = "orrery.agent.cli-validator"   # Optional; injected as env var below
keychain_account = "testuser"
secret_env = "ANTHROPIC_API_KEY"
```

```toml
# CloudApi
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
```

Inference servers (`[[inference]]` in `project.toml` or global `inference.toml`). These describe servers the operator already runs; see §9.6a for every field.

```toml
# Single-model llama-server in an existing container
[[inference]]
inference_id = "coder-14b"
endpoint_url = "http://127.0.0.1:8080"
mode = "Single"                    # "Single" (default) | "Router"
lifecycle = { type = "Container", name = "llama-coder-14b" }
unload_after_stage = true          # Default true: stop it before a stage that doesn't use it (only if Orrery started it)
```

```toml
# Router-mode llama-server defined in a compose file
[[inference]]
inference_id = "router"
endpoint_url = "http://127.0.0.1:8081"
mode = "Router"
lifecycle = { type = "Compose", file = "docker-models/router/compose.yaml", service = "llama" }
```

```toml
# Already running; Orrery only talks HTTP
[[inference]]
inference_id = "always-on"
endpoint_url = "http://127.0.0.1:8082"
lifecycle = { type = "External" }
```

`AgentConfig::validate_no_embedded_secrets` rejects any field value matching common key shapes (`sk-`, `Bearer `, `AIza`, `ghp_`, `xox`). This is defense in depth, not the primary control.

### 4.4 MCP servers — `mcp/servers.toml`

```toml
[[servers]]
server_id = "filesystem"
transport = { type = "Stdio", command = "mcp-filesystem", args = ["--root", "."] }
write_tools = [{ tool = "write_file", path_args = ["path"] }]   # Marks writer tools and which args are paths

[[servers]]
server_id = "remote"
transport = { type = "StreamableHttp", endpoint_url = "https://mcp.example.com/mcp", keychain_service = "orrery.mcp.remote", keychain_account = "testuser" }
```

---

## 5. Components (`orrery-core`)

### 5.1 `lib.rs` — Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}")] Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("TOML parse error in {path}")] TomlDe { path: PathBuf, #[source] source: toml::de::Error },
    #[error(transparent)] TomlSer(#[from] toml::ser::Error),
    #[error(transparent)] Json(#[from] serde_json::Error),
    #[error("path escapes project: {path}")] PathTraversal { path: PathBuf },
    #[error("invalid {field}: {message}")] Validation { field: String, message: String },
    #[error("security violation: {message}")] Security { message: String },
    #[error("keychain error")] Keychain(#[source] keyring_core::Error),
    #[error("plan parse error at line {line}: {message}")] PlanParse { line: usize, message: String },
    #[error("invalid agent report: {message}")] Report { message: String },
    #[error("docker error")] Docker(#[source] bollard::errors::Error),
    #[error("MCP error on {server_id}")] Mcp { server_id: String, #[source] source: Box<dyn std::error::Error + Send + Sync> },
    #[error("agent {agent_id} failed: {message}")] Agent { agent_id: String, message: String },
    #[error("HTTP error")] Http(#[source] reqwest::Error),
    #[error("plan step {step_number}: {message}")] Engine { step_number: usize, message: String },
    #[error("interrupted")] Interrupted,
}
pub type Result<T> = std::result::Result<T, Error>;
```

Variants that wrap a crate's error are added in the step that adds that crate. No `.unwrap()`/`.expect()` outside tests.

### 5.2 `config.rs`

- Types: `ExecutionMode { Run, Loop }`, `ValidationMode { Auto, Agents, GateAndOperator }`, `StageConfig { role_id, agent_ids }`, `HookEvent { StepStart, StepPass, StepRetry, Halt }`, `HookConfig { tool, on_failure: HookFailure { Halt, Warn } }`, `ProjectConfig`, `RoleKind`, `RoleConfig`, `AgentBackend { LocalModel{..}, AgentCli{..}, CloudApi{..} }`, `AgentConfig`, `InferenceConfig`, `McpTransportConfig { Stdio{..}, StreamableHttp{..} }`, `WriteToolSpec { tool, path_args }`, `McpServerConfig`, `ConfigStore`.
- `ProjectConfig::default_template(project_name) -> Self`
- `ProjectConfig::validate(&self, roles, agents) -> Result<Vec<Warning>>`, `validate_for_run(..)`, `effective_validation_mode(&self, roles) -> ValidationMode`
- `ProjectConfig::set_stage_count(&mut self, count) -> Result<()>` — grows with empty implementer stages; shrinking removes trailing stages; never removes stage 0.
- `ProjectConfig::set_stage_role(&mut self, index, role_id)`, `assign_agent(&mut self, index, agent_id)`, `remove_agent(&mut self, index, agent_id)`, `clear_stage(&mut self, index)` — each re-validates staging invariants.
- `AgentConfig::validate_no_embedded_secrets(&self) -> Result<()>`
- `ConfigStore::new(project_root, xdg_override: Option<PathBuf>) -> Result<Self>`, `init_project(name)`, `resolve_safe_path(rel)`, `load_project`/`save_project`, `load_roles`/`save_role`/`remove_role`, `load_agents`/`save_agent`/`remove_agent`, `load_mcp_servers`/`save_mcp_server`/`remove_mcp_server`, `load_inference`.
- Layering: global catalog loaded first; project entries with the same id replace (not merge) global ones.

### 5.3 `security.rs`

- `SecretValue(secrecy::SecretString)`; `Debug`/`Display` print `[REDACTED]`; `SecretValue::expose(&self) -> &str` is `pub(crate)` and used only by HTTP auth and AgentCli env injection.
- `trait SecretStore: Send + Sync { set, get, delete }` with `OsKeyringStore` and `InMemorySecretStore` (tests).
- **Headless policy:** `OsKeyringStore` tries Secret Service first (`dbus_secret_service_keyring_store::Store::new()`); if that fails (no D-Bus session), it uses kernel keyutils (`linux_keyutils_keyring_store::Store::new()`), which does not survive reboot — the CLI tells the operator to re-run `orrery secret set`. If neither is available, return `Error::Keychain`. There is no file or env-var fallback.
- `SecretRedactor`: `register(&self, &SecretValue)`, `redact(&self, &str) -> String`. Replaces registered secrets (len ≥ 8) and patterns `Bearer \S+`, `sk-[A-Za-z0-9_-]{8,}`, `AIza[0-9A-Za-z_-]{20,}`, `ghp_[A-Za-z0-9]{20,}` with `[REDACTED]`. Pattern matching is hand-written (no regex dependency).

### 5.4 `plan.rs` — Plan Step Parser

- `PlanStep { index: usize, number: usize, title: String, body: String, sha256_hex: String }` — hash over `title + "\n" + body` with line endings normalized to `\n`.
- `ImplementationPlan { source_path: Option<PathBuf>, step_marker: String, preamble: String, steps: Vec<PlanStep> }`
- `ImplementationPlan::parse(markdown, step_marker) -> Result<Self>`:
  - Tracks fenced code blocks (```` ``` ```` and `~~~`, matching fence character and minimum length per CommonMark). Marker lines inside fences are ignored.
  - Outside fences, a line starting with `step_marker` begins a step; `title` is the rest of the line, trimmed.
  - Text before the first marker is `preamble`.
  - Errors: empty marker, zero steps, unclosed fence at EOF (`PlanParse` with line number).
  - If titles begin with a number (`1:`, `1.`, `1 `), numbers must be contiguous starting at 1; otherwise `PlanParse`.
- `ImplementationPlan::from_file(path, step_marker)`, `plan_sha256(&self)`.
- **Acceptance fixture:** parsing `STEPPED_IMPLEMENTATION_PLAN.md` with marker `### STEP ` yields exactly the plan's step count (37, numbered 1..=37). That marker is specific to Orrery's own plan, whose headings follow the Nebulous format; the product default `step_marker` stays `"## Step "` (§4.1).

### 5.5 `checkpoint.rs` — Reports & State

**What the agent submits** (`AgentReport`, `deny_unknown_fields`):
- `status: ReportStatus` — per role kind:
  - Orchestrator: `Dispatch` | `StepPassed` | `Escalate`
  - Implementer: `Complete` | `Blocked`
  - Validator: `Confirmed` | `Denied`
- `summary: String` (non-empty, ≤ 4 KiB)
- `target_stage: Option<usize>` — required for `Dispatch`
- `allocations: Vec<SubtaskAllocation>` — required non-empty for `Dispatch`; `SubtaskAllocation { target_agent_id, instructions, write_scope: Vec<String> }` (`write_scope` = relative path prefixes)
- `feedback: Option<String>` — required for `Denied`, `Blocked`, `Escalate`
- `artifacts: Vec<String>` — paths or refs the agent produced/touched

**What Orrery stamps** (`StampedReport { report: AgentReport, step_number, plan_step_sha256, round, stage_index, role_id, agent_id, recorded_at }`). Agents never supply these.

**How reports arrive:**
- `LocalModel` / `CloudApi`: the agent calls a synthetic `submit_report` function tool whose JSON schema is generated from `AgentReport`. A turn that ends without a valid `submit_report` call is re-prompted once with the validation error, then fails (`Error::Report`).
- `AgentCli`: the agent writes `/orrery/out/report.toml` (path given in its prompt). Same schema, same validation, same single re-prompt.

**Functions:**
- `AgentReport::validate_for_role(&self, role: &RoleConfig, stage_count: usize) -> Result<()>`
- `report_json_schema() -> serde_json::Value` — hand-built JSON Schema for the `submit_report` tool; required fields must match `AgentReport`.
- `WorkflowCheckpoint { run_id, plan_sha256, step_hashes: Vec<String>, step_statuses: Vec<StepStatus>, current_step_index, round, active_stage_index, retry_count, steps_passed_this_run, phase: EnginePhase, hook_results: Vec<HookResult> }`; `StepStatus { Pending, InProgress, Passed, AwaitingOperator, Failed }`.
- `CheckpointManager::new(dir)`, `save_state`, `load_state -> Option<_>`, `save_report(&StampedReport) -> PathBuf`, `load_reports(step_number, round: Option<usize>)`.
- **Plan-edit policy** (`WorkflowCheckpoint::reconcile(&self, plan) -> Result<ReconcileOutcome>`): if any `Passed` or the current step's hash changed → `Error::Engine` (operator must `orrery reset --step N`). Changes to later steps, or appended steps, are accepted and the stored hashes updated.

### 5.6 `stream_log.rs` — Live Stream

- `StreamEventKind { Output, Reasoning, SystemNotice, ToolCall, ReportSaved, GateResult, Error }`; `StreamEvent { timestamp, step_number, stage_index, agent_id, kind, text }`.
- `LiveStreamConfig { ring_capacity: 2_000, max_file_bytes: 10 MiB, max_files: 3 }`.
- `LiveStreamHub::new(log_dir, config, Arc<SecretRedactor>)`; `emit(&self, event)` redacts, pushes to the ring buffer (evicting oldest), appends to `live_stream.log`, rotates `.log → .log.1 → .log.2` (oldest dropped) **before** a write would exceed `max_file_bytes`; `subscribe(&self) -> tokio::sync::broadcast::Receiver<StreamEvent>`; `recent(&self, limit)`.
- Interior mutability (`Mutex` held only for non-async sections); the hub is shared as `Arc<LiveStreamHub>`.
- Invariant: total log bytes ≤ `max_file_bytes * max_files`.
- `Reasoning` events appear only when a backend exposes reasoning tokens.

### 5.7 `docker.rs` — Containers & Resources

- `#[async_trait] trait ContainerBackend: Send + Sync`:
  - `start(&self, name: &str) -> Result<()>` (existing containers only; §9.6)
  - `stop(&self, name: &str) -> Result<()>`
  - `remove(&self, name: &str) -> Result<()>`
  - `inspect(&self, name: &str) -> Result<ContainerState>`
  - `run_to_completion(&self, spec: &ContainerSpec, stdin: Vec<u8>, on_output: &(dyn Fn(String) + Send + Sync)) -> Result<ExitStatus>` (AgentCli)
- `ContainerSpec { name, image, cmd, env: Vec<(String, EnvValue)>, mounts: Vec<MountSpec>, memory_limit_mb, auto_remove: bool }` (AgentCli containers only); `EnvValue::{Plain(String), Secret(SecretValue)}`.
- `BollardContainerBackend::connect()` — `Docker::connect_with_local_defaults()`, which honors `DOCKER_HOST` (rootless Docker, Podman). No socket path in any config file. No GPU configuration (§9.6).
- `MockContainerBackend` records every call for tests.
- `ResourceManager<B: ContainerBackend>`:
  - `new(backend: Arc<B>, inference: Vec<InferenceConfig>, max_concurrent_agents)`
  - `begin_stage(&self, agents: &[AgentConfig], writer: bool, allocations: &[SubtaskAllocation]) -> Result<StageLease>` — fails if a lease is already open (no cross-stage overlap). Ensures each needed inference container is running. Computes effective concurrency (§5.10.3).
  - `StageLease::acquire_slot(&self) -> SlotPermit` (semaphore).
  - `end_stage(&self, lease) -> Result<()>` — removes AgentCli containers and deletes their `agent_io` dirs. Model release happens at the next `begin_stage` (§9.6).
  - `shutdown(&self)` — unloads held router models and stops only the inference servers Orrery started.
- Pure helpers (unit-tested on their own): `effective_concurrency(max_concurrent_agents, writer: bool, agents: &[AgentConfig], allocations: &[SubtaskAllocation]) -> usize` (§5.10.3); `mounts_for(agent: &AgentConfig, project_root, agent_io_dir) -> Vec<MountSpec>` (§5.7.1).

#### 5.7.1 Mounts
- Inference servers: Orrery adds no mounts (they are the operator's own).
- AgentCli containers: project at `/workspace` per `workspace_mount`; `agent_io/<run>/<agent>/in` at `/orrery/in` (ro); `.../out` at `/orrery/out` (rw). Never mount `.orrery/` itself: when the project is mounted, an empty `agent_io/<run>/<agent>/mask` is bound read-only over `/workspace/.orrery`, so no agent can read or edit `.orrery/` (config, checkpoints, role prompts).

#### 5.7.2 AgentCli guardrails & secrets
- The MCP gatekeeper does not see a CLI's built-in tools. Guardrails are: `workspace_mount`, container isolation, memory/time limits, and whatever tool config the image ships. Document this in the TUI agent details and `orrery agent show`.
- Secrets are injected as an env var at container create time and the container is removed when the stage ends. Residual risk: anyone with Docker socket access can read container env while it runs (that access is already root-equivalent). Second residual risk: a `ReadWrite` mount can write `.git/hooks`, which the host runs at the next git command. State both in `orrery agent add-cli` output.

### 5.8 `mcp.rs` — MCP Gatekeeper

- `#[async_trait] trait McpTransport: Send + Sync { async fn list_tools(&self) -> Result<Vec<McpToolSchema>>; async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value>; }`
- `RmcpTransport` — wraps an `rmcp` client (stdio child process or Streamable HTTP); performs the MCP `initialize` handshake on connect; HTTP auth header from `SecretStore`.
- `MockMcpTransport` — programmable registry for tests.
- `McpToolSchema { server_id, name, description, input_schema }`.
- `McpGatekeeper`:
  - `register(&mut self, server_id, Arc<dyn McpTransport>, write_tools: Vec<WriteToolSpec>)`, `refresh_catalog(&mut self)`
  - `is_tool_allowed(role, server_id, tool) -> bool` — patterns `server:tool`, `server:*`. (`*:*` is not supported.)
  - `tools_for_role(&self, role) -> Vec<McpToolSchema>`
  - `execute_for_role(&self, role, scope: &WriteScope, server_id, tool, args) -> Result<Value>` — rejects disallowed tools with `Error::Security` before any transport call; for write tools, rejects any `path_args` value outside `scope` (normalized, no `..`).
- **Tool-name mapping** (`llm.rs`): function names exposed to models are `<server_id>__<tool>`, restricted to `[A-Za-z0-9_-]{1,64}`; ids that can't map are rejected at config validation.

### 5.9 `llm.rs` — Agent Drivers

- `AgentInvocation { step_number, stage_index, round, role: RoleConfig, agent: AgentConfig, system_prompt: String, user_prompt: String, tools: Vec<McpToolSchema>, write_scope: WriteScope }`
- `build_system_prompt(role: &RoleConfig, backend: &AgentBackend) -> String` — role rules + report instructions (function tool or `report.toml` path).
- `build_user_prompt(step: &PlanStep, allocation: Option<&SubtaskAllocation>, prior: &[StampedReport]) -> String` — only step text, the agent's allocation, and summaries/feedback of prior **validated** reports for this plan step. Never live-stream content.
- `#[async_trait] trait AgentDriver: Send + Sync { async fn run_turn(&self, inv: &AgentInvocation, mcp: &McpGatekeeper, stream: &LiveStreamHub) -> Result<AgentReport>; }`
- `HttpChatDriver` (`LocalModel` → `InferenceConfig::chat_url()`; `CloudApi` → `endpoint_url` with `Authorization: Bearer` from `SecretStore`): streams output to the hub, runs the tool-call loop through the gatekeeper (max 50 tool calls per turn, configurable), ends on `submit_report`.
- `CliContainerDriver`: writes the prompt to `/orrery/in/prompt.md`, runs the container via `ContainerBackend::run_to_completion`, streams stdout lines to the hub, reads and validates `/orrery/out/report.toml`. Enforces `timeout_secs`.
- `MockAgentDriver`: scripted reports for engine tests.

### 5.10 `engine.rs` — Workflow State Machine

#### 5.10.1 Flow per plan step

```mermaid
stateDiagram-v2
    [*] --> LoadAndReconcile
    LoadAndReconcile --> StepStart: next Pending step
    StepStart --> OrchestratorTurn: on_step_start hook
    OrchestratorTurn --> StageTurn: Dispatch(target_stage)
    StageTurn --> OrchestratorTurn: stage reports saved
    OrchestratorTurn --> PassCheck: StepPassed
    PassCheck --> StepPassed: pass rule holds
    PassCheck --> OrchestratorTurn: rule fails (protocol violation, retried once)
    OrchestratorTurn --> AwaitingOperator: Escalate / retries exhausted
    StepPassed --> [*]: Run mode, or cycle_count reached, or no steps left
    StepPassed --> StepStart: Loop mode
    AwaitingOperator --> OrchestratorTurn: operator retry
    AwaitingOperator --> [*]: operator abort
```

- A round = one orchestrator turn + the dispatched stage turn. After every stage turn, control returns to the orchestrator.
- `Dispatch.target_stage` must be either the next stage after the last one run in this plan step, or an earlier non-orchestrator, non-validator stage (a **retry**). Anything else is a protocol violation.
- Every dispatch back to an earlier stage counts as a retry (amended 2026-11-15: counting only retries after a `Denied` report or a failed gate let a step loop forever). A `StepPassed` whose gate fails with no stage turn since the previous failed gate is a protocol violation, so every path through a step is bounded. `retry_count > max_step_retries` → `AwaitingOperator`. The `on_step_retry` hook runs before each retry.
- `Blocked` from an implementer is passed to the orchestrator; the orchestrator decides (re-dispatch, or `Escalate`).

#### 5.10.2 Pass rule (enforced by Orrery, not the orchestrator)
- **`Agents` mode:** the most recent validator stage turn in this plan step has `Confirmed` from **every** validator agent, no implementer stage ran after it, and (if `gate_tools` is non-empty) the gate passed after the last implementer turn.
- **`GateAndOperator` mode:** all `gate_tools` succeed (run by Orrery after the last implementer turn, results streamed as `GateResult`), then the operator approves (TUI prompt, or `orrery approve` / `orrery reject --feedback`). A rejection counts as a retry and its feedback goes to the orchestrator.
- Only when the rule holds and the orchestrator reports `StepPassed` does the step become `Passed`. Then `on_step_pass` runs.

#### 5.10.3 Concurrency within a stage
- Non-writer stage: up to `max_concurrent_agents` agents in parallel.
- Writer stage: 1 at a time, unless every allocation has a non-empty `write_scope`, the scopes are pairwise disjoint (no scope is a prefix of another), and no agent in the stage is `AgentCli` with `ReadWrite` mount. Then up to `max_concurrent_agents`.
- Validators receive the same instructions (broadcast) when the orchestrator allocates to one validator but the stage has several; every validator must report.

#### 5.10.4 Lifecycle hooks
- On each event, Orrery calls the configured MCP tool via the gatekeeper with a reserved system role whose allowlist is exactly the configured hook tools. Arguments: `{ "orrery": { "project_name", "run_id", "step_number", "step_title", "plan_step_sha256", "event" } }`.
- The tool's text result (redacted, truncated to 4 KiB) is stored in `WorkflowCheckpoint.hook_results`.
- `on_failure = "Halt"` → `AwaitingOperator`; `"Warn"` → stream a `SystemNotice` and continue.

#### 5.10.5 Modes
- **Run:** one plan step to `Passed`, save checkpoint, stop.
- **Loop:** repeat until `steps_passed_this_run == cycle_count` (when `cycle_count > 0`) or no `Pending` steps remain.

#### 5.10.6 Interrupt & resume
- `WorkflowEngine::run(&self, cancel: CancellationToken) -> Result<RunOutcome>`. Cancel (Ctrl+C) finishes no further tool calls, ends the stage lease, saves the checkpoint with the step `InProgress` and the last completed round, runs `on_halt`, returns `Interrupted`.
- Resume restarts the interrupted round from its orchestrator turn.
- `WorkflowEngine` exposes an event channel (`EngineEvent`: phase changes, step status, awaiting-operator prompts) for the TUI bridge.

---

## 6. TUI (`orrery-tui`)

### 6.1 Layout (`layout.rs`, `UiLayoutRegions::compute(area: Rect, stage_count, stage_scroll, slot_scroll)`)

Minimum terminal 80×24. Below that, render only a centered "Terminal too small (need 80×24)" notice.

Vertical root split:
1. `header` — `Length(11)` with the full banner (§6.3) when height ≥ 40, else `Length(3)` with the compact header.
2. `top_menu` — `Length(3)`.
3. `main_workspace` — `Fill(3)`.
4. `live_stream` — `Fill(2)`.
5. `footer_menu` — `Length(3)`.

At 80×24: 3 + 3 + 9 + 6 + 3 = 24. No unused rows.

`main_workspace` horizontal: `steps_panel` `Percentage(25)`, `stage_canvas` `Fill(1)`.
- `stage_canvas`: each visible stage column has min width 18; columns that don't fit scroll horizontally (`stage_scroll`). Inside a column: role header (3 rows), then agent slots (1 row each), scrolling vertically (`slot_scroll`) when slots exceed the height.
- `region_at(col, row) -> Option<UiRegion>` hit-tests every interactive cell.

### 6.2 Widgets
- **Header:** banner or compact header; project path; status badge `IDLE | RUNNING | AWAITING OPERATOR | PASSED | ERROR`.
- **Top menu:** `[Ingest]`, `[Mode: RUN|LOOP]`, `[Cycles: N]` (dimmed in Run mode), `[Passed: X/Y]` (read-only), `[Stages: - N +]`.
- **Steps panel:** plan steps with badges `[ ]` pending, `[>]` in progress, `[✓]` passed, `[?]` awaiting operator, `[!]` failed.
- **Stage canvas:** header = role selector. Slots locked until a role is chosen. Orchestrator: 1 slot. Other roles: choosing an agent in slot k unlocks slot k+1, up to 15. Each slot shows the backend kind tag (`LM`, `CLI`, `API`).
- **Live stream:** redacted events from `LiveStreamHub::subscribe`, newest at the bottom, scrollable; auto-follows unless the user scrolled up.
- **Footer:** `[Run]`/`[Stop]`, `[Approve]`/`[Reject]` (only in `AWAITING OPERATOR` + `GateAndOperator`), `[Settings]`, `[Help]`, `[Mouse: ON|OFF]`, `[Quit]`.
- **Modals:** Ingest path entry; Settings (read-only lists of roles, agents, MCP servers, hooks — editing is via CLI in v1); Help (key list); Quit confirm (only while RUNNING); Reject feedback entry.

### 6.3 Theme & Banner (`theme.rs`)

Palette copied verbatim from Nebulous (`Nebulous/crates/nebulous_cli/src/tui/theme.rs`):

| Constant | Hex | RGB | Use |
|---|---|---|---|
| `BG_BLACK` | `#000000` | (0, 0, 0) | Every cell's background |
| `PANEL_BG` | `#0C0800` | (12, 8, 0) | Modals, focused panel background |
| `AMBER_PRIMARY` | `#FFB000` | (255, 176, 0) | Primary text, active borders, banner letters |
| `AMBER_BRIGHT` | `#FFD866` | (255, 216, 102) | Focus highlight, running badge, banner sun/planets |
| `AMBER_DIM` | `#996300` | (153, 99, 0) | Inactive borders, secondary labels, banner orbit |
| `AMBER_DARK` | `#5C3A00` | (92, 58, 0) | Dividers, locked slots, banner background stars |
| `AMBER_ALERT` | `#FF5500` | (255, 85, 0) | Denials, errors, guardrail alerts |

Full banner (9 rows × 72 cols, ASCII only). Store as `pub const BANNER: [&str; 9]` of raw string literals — the art contains backslashes. Paste exactly; do not retype or "fix" it.

```text
            .       o            .                *           .
        _.-~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~-._
     .-~      ___   ____   ____   _____  ____  __   __            ~-.
   .~        / _ \ |  _ \ |  _ \ | ____||  _ \ \ \ / /    .-.        ~.
  @         | | | || |_) || |_) ||  _|  | |_) | \ V /    ( * )         )
   '.       | |_| ||  _ < |  _ < | |___ |  _ <   | |      '-'        .'
     '-.     \___/ |_| \_\|_| \_\|_____||_| \_\  |_|              .-'
        '-.____________________________________________________.-O
     .         *             .        o              .
```

Banner coloring (rows and columns 1-based), first matching rule wins: rows 1 and 9 → `AMBER_DARK`. Rows 3–7, columns 13–54 (the `ORRERY` letter block) → `AMBER_PRIMARY`. Rows 2–8, characters `@ O * ( )` (sun and planets) → `AMBER_BRIGHT`. Everything else in rows 2–8 (orbit, including the orbit's `~` and `_` on rows 2 and 8) → `AMBER_DIM`.

Theme API: the constants above; `BANNER`; `COMPACT_HEADER: &str`; `banner_line(row: usize) -> ratatui::text::Line<'static>` (applies the coloring rules); style helpers `base_block()`, `focused_block()`, `alert_block()`, `muted()`, `highlight()`.

Compact header content (1 row, inside the bordered 3-row header; status badge right-aligned):

```text
 -(*)--o---@----O-   O R R E R Y  //  WORKFLOW ORCHESTRATOR        [IDLE]
```

### 6.4 Input
`App::handle_key_event(&mut self, KeyEvent) -> Vec<AppAction>` and `App::handle_mouse_event(&mut self, MouseEvent, &UiLayoutRegions) -> Vec<AppAction>`. They change only `App` state and return actions; they never render or await.

- **No invented modifier chords.** Standard keys keep standard meanings.
- `Ctrl+C`: interrupt the running workflow (cancel token). When idle, it does nothing (does not quit).
- `Tab`/`BackTab`: cycle focus `TopMenu → StepsPanel → StageCanvas → LiveStream → FooterMenu`.
- Arrows: move within a panel; scroll when the panel is a list.
- `Enter`/`Space`: activate / open selector / confirm.
- `+`/`-`: adjust the focused numeric control.
- `Esc`: close modal or cancel text entry.
- `m`: toggle mouse capture. With capture off, the terminal's native text selection works (copying from the live stream).
- `q`: quit when not editing text; asks for confirmation while RUNNING.
- `?`: Help modal.
- Mouse (when captured): left-click focuses + activates via `region_at`; wheel scrolls the panel under the cursor.

### 6.5 Engine bridge (`bridge.rs`)
- `run_tui(project_root: PathBuf) -> Result<()>` owns the terminal (raw mode, alternate screen, restores on drop and on panic).
- The loop body is `fn step(app: &mut App, input: BridgeInput) -> Effects` (pure, testable without a terminal); `run_tui` only does terminal I/O around it.
- The engine runs on a Tokio task. The TUI receives `EngineEvent`s and `StreamEvent`s through channels and sends `AppAction`s (`Run`, `Stop`, `Approve`, `Reject(feedback)`, config edits) back. The render loop never blocks on engine work.

---

## 7. CLI (`orrery-cli`)

All commands accept `--project <PATH>` (default `.`). Output is plain text; add `--json` where noted for scripting.

| Command | Behavior |
|---|---|
| `orrery [PATH]` | Launch TUI. |
| `orrery init [PATH] [--name <NAME>]` | Create `.orrery/` with the 3-stage template, default roles, empty dirs. |
| `orrery ingest <MD_FILE> [--marker <STR>]` | Parse/validate plan, set `plan_path`/`step_marker`, print step list. |
| `orrery workflow show [--json]` | Stages, roles, agents, validation mode, warnings. |
| `orrery workflow stages <COUNT>` | Set stage count (2..=15). |
| `orrery workflow set-role <STAGE> <ROLE_ID>` | Assign role to a stage. |
| `orrery workflow assign <STAGE> <AGENT_ID>` | Add agent to a stage. |
| `orrery workflow unassign <STAGE> <AGENT_ID>` | Remove agent from a stage. |
| `orrery workflow clear <STAGE>` | Remove all agents from a stage. |
| `orrery workflow mode <run\|loop> [--cycles <N>]` | Set execution mode / cycle count. |
| `orrery agent add-local-model --id --name --inference <ID> --model <M> [--temperature] [--context]` | Register `LocalModel`. |
| `orrery agent add-cli --id --name --image <IMG@DIGEST> --command <ARG>... [--mount none\|ro\|rw] [--memory-mb] [--timeout] [--secret-env <VAR> --keychain-service --keychain-account]` | Register `AgentCli`; prints the §5.7.2 residual-risk note. |
| `orrery agent add-cloud --id --name --endpoint <URL> --model <M> --keychain-service --keychain-account` | Register `CloudApi`. |
| `orrery agent list [--json]` / `show <ID>` / `remove <ID>` | Inspect/remove agents. |
| `orrery inference add --id --endpoint <URL> [--mode single\|router] [--container <NAME> \| --compose <FILE> [--service <S>]] [--no-unload] [--startup-timeout <SECS>]` / `list` / `remove <ID>` | Register an existing model server. No `--container`/`--compose` → `External`. `--container` and `--compose` conflict. `--compose` is relative to `.orrery/` and must resolve (`resolve_compose_file`, Project) before saving; absolute paths go in the machine-scope `inference.toml` by hand. |
| `orrery secret set --service <S> --account <A>` | Read secret from a no-echo prompt (or stdin when not a TTY) into the OS keychain. |
| `orrery secret delete --service <S> --account <A>` | Remove it. |
| `orrery role add --id --name --kind <orchestrator\|implementer\|validator> --rules-file <MD> [--allow-tool <SERVER:TOOL>]...` / `list` / `show <ID>` / `remove <ID>` | Manage roles. |
| `orrery mcp add-stdio --id --command <CMD> [--arg <A>]... [--write-tool <TOOL>:<ARG>[,<ARG>]]...` | Register stdio MCP server. Connects first (handshake + `list_tools`) and saves only on success; prints the tool count. Project scope: `--command` is a bare name on `PATH` and no `--arg` is an absolute path. |
| `orrery mcp add-http --id --endpoint <URL> [--keychain-service --keychain-account] [--write-tool ...]` | Register Streamable HTTP MCP server. Connects first and saves only on success, as `add-stdio`. |
| `orrery mcp list` / `tools <ID>` / `remove <ID>` | Inspect/remove; `tools` connects and lists tools. |
| `orrery gate set <SERVER:TOOL>...` / `clear` | Set/clear `gate_tools`. |
| `orrery hook set <start\|pass\|retry\|halt> <SERVER:TOOL> [--on-failure halt\|warn]` / `clear <EVENT>` | Configure hooks. |
| `orrery run [--loop] [--cycles <N>] [--step <N>]` | Run headless; streams the live stream to stdout. Ctrl+C interrupts cleanly. |
| `orrery approve` / `orrery reject --feedback <TEXT>` | Operator decision for an `AwaitingOperator` step. |
| `orrery resume` | Continue an interrupted or escalated run. |
| `orrery reset --step <N>` | Mark step N and later steps `Pending` (needed after editing a passed step). |
| `orrery status [--json]` | Plan steps, statuses, phase, retries. |
| `orrery logs [--tail <N>] [--follow]` | Redacted live stream log. |

Exit codes: `0` success, `1` error, `2` usage error (clap), `3` awaiting operator, `130` interrupted.

---

## 8. Quality Gates

Every plan step ends with all of these passing:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
cargo llvm-cov --workspace --locked --fail-under-lines 80
```

- `deny.toml`: advisories = deny (including `unmaintained` and `yanked`); bans `openssl-sys` and `openssl`; licenses allow-list (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0 — MPL-2.0 approved 2026-11-12 for `option-ext` via `dirs`; extend only with operator approval); sources = crates.io only; `[graph] targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]` (Orrery is Linux-only; without this, cargo-deny also checks Android-only `webpki-root-certs`, licensed CDLA-Permissive-2.0).
- If the sandbox blocks network access, run `cargo deny --offline check` and say so in the step report.
- Build host packages (operator installs once): `libdbus-1-dev`, `pkg-config`.
- `[workspace.lints]`: `rust.unsafe_code = "forbid"`, `clippy.unwrap_used = "deny"`, `clippy.expect_used = "deny"`. Tests may unwrap through `clippy.toml` (`allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`), not through `#[allow]` attributes.
- No `#[ignore]`, no empty/assertion-free tests, no `#[allow(...)]` in non-test code.
- Tests cover behaviors and invariants. Trivial tests (e.g. asserting an error's `Display` string) don't count toward intent; coverage floor is the measured gate.
- **Live-backend tests** (real Docker, a real local model server, real MCP servers) live in `crates/orrery-core/tests/live_*.rs` with `required-features = ["live-backends"]`. They aren't compiled in the default gate, and the operator runs them with `cargo test --workspace --locked --features live-backends`. `live_keyring` uses the real OS keychain (service `orrery.test`, account `testuser`) and needs no env var. Other inputs come from env vars: `ORRERY_LIVE_MCP_CMD` (stdio MCP server command), `ORRERY_LIVE_IMAGE` (small test image), `ORRERY_LIVE_CONTAINER` (an existing container Orrery may start and stop; never removed), `ORRERY_LIVE_LLM_URL` + `ORRERY_LIVE_LLM_MODEL` (OpenAI-compatible endpoint). A missing var fails the test with a clear message; it never silently passes.
- Fixtures use reserved placeholders only: `example.com`/`.test` domains, `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`, `testuser`. No absolute host paths in code, tests or docs.
- NIST alignment: SP 800-218 (SSDF) practices via these gates and `cargo deny`; FIPS 180-4 SHA-256 for integrity; SP 800-63B-aligned secret handling (§5.3).

---

## 9. Interface Reference (binding)

Exact public signatures for everything §5–§7 leave open. Plan steps implement these as written. Where a line here differs from earlier text, it is marked **(overrides §x)** and this section wins. Private helpers, field order, and derive lists beyond those shown are free choices; they never need a halt.

### 9.0 Conventions (all crates)
- Derives on plain data types: `Debug, Clone, PartialEq` (+ `Eq` when every field allows it) and `Serialize, Deserialize` when the type is written to disk or sent as JSON. Types holding a `SecretValue`, a trait object, a channel or a lock derive only what compiles; none of them derive `Clone` unless shown.
- A poisoned `Mutex`/`RwLock` maps to `Error::Security { message: "<type name> lock poisoned".into() }`.
- Tests live in `#[cfg(test)] mod tests` at the bottom of the file they test, except `tests/live_*.rs` and `crates/orrery-cli/tests/cli_smoke.rs`.
- Async tests use `#[tokio::test]`.
- Reuse, don't re-implement: `config::atomic_write`, `config::validate_safe_relative_path`, `config::format_mapped_tool_name`, `config::validate_concrete_tool_ref`.
- Reserved role ids built by the engine (never loaded from disk): `"__orrery_hooks"`, `"__orrery_gate"`.

### 9.1 `security.rs`
```rust
pub const REDACTED: &str = "[REDACTED]";
pub struct SecretValue(secrecy::SecretString);          // no Clone, no Serialize
impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self;
    pub(crate) fn expose(&self) -> &str;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
impl fmt::Debug for SecretValue   // writes REDACTED
impl fmt::Display for SecretValue // writes REDACTED

pub trait SecretStore: Send + Sync {
    fn set(&self, service: &str, account: &str, secret: &SecretValue) -> Result<()>;
    fn get(&self, service: &str, account: &str) -> Result<SecretValue>; // missing → Error::Keychain(keyring_core::Error::NoEntry)
    fn delete(&self, service: &str, account: &str) -> Result<()>;       // missing → Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringBackend { SecretService, Keyutils }
pub struct OsKeyringStore { store: Arc<keyring_core::CredentialStore>, backend: KeyringBackend }
impl OsKeyringStore {
    pub fn new() -> Result<Self>;          // Secret Service store, else keyutils store, else Error::Keychain(<keyutils error>)
    pub fn backend(&self) -> KeyringBackend;
}
// SecretStore for OsKeyringStore: entry = self.store.build(service, account, None)?;
// set → entry.set_password(secret.expose()); get → entry.get_password() → SecretValue::new;
// delete → entry.delete_credential(), NoEntry → Ok(()). All keyring_core errors → Error::Keychain.

#[derive(Default)]
pub struct InMemorySecretStore { entries: Mutex<HashMap<(String, String), SecretValue>> }

#[derive(Default)]
pub struct SecretRedactor { secrets: RwLock<Vec<SecretValue>> }
impl SecretRedactor {
    pub fn new() -> Self;
    pub fn register(&self, secret: &SecretValue) -> Result<()>;  // stores SecretValue::new(secret.expose()); len < 8 is ignored (Ok)
    pub fn redact(&self, text: &str) -> Result<String>;
}
```
`redact` algorithm (fixed order): (1) replace every registered secret, longest first, with `REDACTED`; (2) apply each pattern with one private helper `redact_prefixed(text, prefix, min_len, allowed: fn(char) -> bool) -> String`, which finds `prefix`, then takes the longest run of `allowed` chars after it; when the run is `>= min_len`, prefix + run becomes `REDACTED`, otherwise the text is left as is. Patterns: (`"Bearer "`, 1, not whitespace), (`"sk-"`, 8, ASCII alphanumeric or `_` `-`), (`"AIza"`, 20, ASCII alphanumeric or `_` `-`), (`"ghp_"`, 20, ASCII alphanumeric). (**`redact` returns `Result` because of the lock; overrides §5.3.**)

### 9.2 `plan.rs`
```rust
pub struct PlanStep { pub index: usize, pub number: usize, pub title: String, pub body: String, pub sha256_hex: String }
pub struct ImplementationPlan { pub source_path: Option<PathBuf>, pub step_marker: String, pub preamble: String, pub steps: Vec<PlanStep> }
impl ImplementationPlan {
    pub fn parse(markdown: &str, step_marker: &str) -> Result<Self>;
    pub fn from_file(path: &Path, step_marker: &str) -> Result<Self>;   // read error → Error::Io { path, source }
    pub fn plan_sha256(&self) -> String;                               // SHA-256 of the step hashes joined with "\n"
    pub fn step(&self, number: usize) -> Option<&PlanStep>;
}
pub fn sha256_hex(bytes: &[u8]) -> String;   // lowercase hex, hand-written: bytes.iter().map(|b| format!("{b:02x}")).collect()
```
- Normalize first: replace `"\r\n"` with `"\n"`. Line numbers in errors are 1-based.
- Fence open: a line whose first non-space char is at indent ≤ 3 and starts a run of ≥ 3 `` ` `` or ≥ 3 `~`. Fence close: same char, run length ≥ the opening run, only whitespace after it.
- `title`: the marker line minus `step_marker`, trimmed. `body`: the lines after the marker line up to (not including) the next marker line or EOF, joined with `"\n"`, then trailing `'\n'` trimmed. `preamble`: same rule for the text before the first marker.
- `number`: if `title` starts with ASCII digits followed by `:`, `.`, a space or end of string, that number; otherwise `index + 1`. If **any** title starts with digits, all must, and they must be `1, 2, 3, …` in order; else `PlanParse` at the offending marker line.
- `sha256_hex` = `sha256_hex((title + "\n" + body).as_bytes())`.

### 9.3 `checkpoint.rs`
```rust
#[serde(deny_unknown_fields)]
pub struct AgentReport {
    pub status: ReportStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub target_stage: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]   pub allocations: Vec<SubtaskAllocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub feedback: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]   pub artifacts: Vec<String>,
}
pub enum ReportStatus { Dispatch, StepPassed, Escalate, Complete, Blocked, Confirmed, Denied }
#[serde(deny_unknown_fields)]
pub struct SubtaskAllocation { pub target_agent_id: String, pub instructions: String, #[serde(default)] pub write_scope: Vec<String> }
pub const MAX_SUMMARY_BYTES: usize = 4096;
impl AgentReport { pub fn validate_for_role(&self, role: &RoleConfig, stage_count: usize) -> Result<()>; }
pub fn report_json_schema() -> serde_json::Value;
#[serde(deny_unknown_fields)]
pub struct StampedReport { pub report: AgentReport, pub step_number: usize, pub plan_step_sha256: String, pub round: usize,
                           pub stage_index: usize, pub role_id: String, pub agent_id: String, pub recorded_at: chrono::DateTime<chrono::Utc> }
```
`validate_for_role` rules, checked in this order, every failure `Error::Report { message }`:
1. `status` allowed for `role.kind` (§5.5 table).
2. `summary.trim()` non-empty and `summary.len() <= MAX_SUMMARY_BYTES`.
3. `Dispatch`: `target_stage == Some(t)` with `1 <= t < stage_count`; `allocations` non-empty; every allocation has non-empty `target_agent_id` and `instructions`; every `write_scope` entry passes `validate_safe_relative_path`.
4. Not `Dispatch`: `target_stage` is `None` and `allocations` is empty.
5. `Denied`, `Blocked`, `Escalate`: `feedback` is `Some` with non-empty trimmed text.

`report_json_schema()`: `{"type":"object","additionalProperties":false,"required":["status","summary"],"properties":{...}}` with one property per `AgentReport` field; `status` has `"enum"` of the 7 variant names; `allocations.items` is an object schema with `required: ["target_agent_id","instructions"]` and `additionalProperties: false`.

```rust
pub enum StepStatus { Pending, InProgress, Passed, AwaitingOperator, Failed }
pub enum EnginePhase { Idle, OrchestratorTurn, StageTurn, Gate, AwaitingOperator, Hook, Done }
pub enum OperatorDecision { Approve, Reject { feedback: String } }
pub struct HookResult { pub event: HookEvent, pub step_number: usize, pub tool: String, pub success: bool, pub output: String, pub recorded_at: chrono::DateTime<chrono::Utc> }
#[serde(deny_unknown_fields)]
pub struct WorkflowCheckpoint {
    pub run_id: uuid::Uuid, pub plan_sha256: String, pub step_hashes: Vec<String>, pub step_statuses: Vec<StepStatus>,
    pub current_step_index: usize, pub round: usize, pub active_stage_index: Option<usize>, pub retry_count: usize,
    pub steps_passed_this_run: usize, pub phase: EnginePhase, pub hook_results: Vec<HookResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub awaiting_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub operator_decision: Option<OperatorDecision>,
}   // last two fields (overrides §5.5): headless approve/reject is stored here and applied by `resume`
pub enum ReconcileOutcome { Unchanged, Updated { changed_steps: Vec<usize>, appended: usize, removed: usize } }
impl WorkflowCheckpoint {
    pub fn new(plan: &ImplementationPlan) -> Self;   // all Pending, run_id = Uuid::new_v4(), round 0, phase Idle
    pub fn reconcile(&mut self, plan: &ImplementationPlan) -> Result<ReconcileOutcome>; // &mut (overrides §5.5)
    pub fn reset_from(&mut self, step_number: usize) -> Result<()>; // steps >= N → Pending; current_step_index = N-1; round 0; retry 0; phase Idle; decision/reason None
}
pub struct CheckpointManager { dir: PathBuf }   // dir = <project>/.orrery/checkpoints
impl CheckpointManager {
    pub fn new(dir: PathBuf) -> Self;
    pub fn save_state(&self, state: &WorkflowCheckpoint) -> Result<()>;       // workflow_state.toml via atomic_write
    pub fn load_state(&self) -> Result<Option<WorkflowCheckpoint>>;           // missing file → Ok(None)
    pub fn save_report(&self, report: &StampedReport) -> Result<PathBuf>;     // step_{:03}/round_{:02}_stage_{:02}_{agent_id}.toml
    pub fn load_reports(&self, step_number: usize, round: Option<usize>) -> Result<Vec<StampedReport>>; // sorted by (round, stage_index, agent_id)
    pub fn delete_reports(&self, step_number: usize, round: usize) -> Result<()>;
}
```
`reconcile`: for each index `i` present in both: hash differs and status is `Passed`, or `i == current_step_index` and status is `InProgress`/`AwaitingOperator` → `Error::Engine { step_number: i + 1, message: "step edited; run `orrery reset --step N`" }`. Otherwise record the new hash. New steps are appended as `Pending`. Steps removed from the end are dropped if they are `Pending`; removing a non-`Pending` step is an error the same way. `plan_sha256` is updated.

### 9.4 `stream_log.rs`
```rust
pub enum StreamEventKind { Output, Reasoning, SystemNotice, ToolCall, ReportSaved, GateResult, Error }
pub struct StreamEvent { pub timestamp: DateTime<Utc>, pub step_number: Option<usize>, pub stage_index: Option<usize>,
                         pub agent_id: Option<String>, pub kind: StreamEventKind, pub text: String }
impl StreamEvent { pub fn notice(text: impl Into<String>) -> Self; } // SystemNotice, all Options None, timestamp now
pub struct LiveStreamConfig { pub ring_capacity: usize, pub max_file_bytes: u64, pub max_files: usize } // Default: 2_000, 10 * 1024 * 1024, 3
pub const LOG_FILE: &str = "live_stream.log";
pub struct LiveStreamHub { /* log_dir, config, redactor, Mutex<(VecDeque<StreamEvent>, current file size)>, broadcast::Sender<StreamEvent> */ }
impl LiveStreamHub {
    pub fn new(log_dir: PathBuf, config: LiveStreamConfig, redactor: Arc<SecretRedactor>) -> Result<Self>; // creates log_dir; current size = existing file length
    pub fn emit(&self, event: StreamEvent) -> Result<()>;
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<StreamEvent>; // channel capacity = ring_capacity
    pub fn recent(&self, limit: usize) -> Result<Vec<StreamEvent>>;          // oldest first
    pub fn redactor(&self) -> &Arc<SecretRedactor>;
}
pub fn tail_log(log_dir: &Path, limit: usize) -> Result<Vec<String>>; // reads .log.2, .log.1, .log in that order; last `limit` lines
```
- File format: one `serde_json` line per event plus `'\n'`.
- `emit` order: redact `text` → ring push (pop front when full) → file write → broadcast (`send` error with no receivers is ignored).
- Rotation, before writing a line of `n` bytes when `size + n > max_file_bytes`: delete `.log.{max_files-1}` if present; rename `.log.{k}` → `.log.{k+1}` for `k = max_files-2 ..= 1`; rename `.log` → `.log.1`; size = 0.
- A single line longer than `max_file_bytes`: shorten `text` (char boundary) until it fits; if empty text still doesn't fit, skip the file write only.

### 9.5 `mcp.rs`
```rust
pub struct McpToolSchema { pub server_id: String, pub name: String, pub description: String, pub input_schema: serde_json::Value }
#[async_trait] pub trait McpTransport: Send + Sync { /* §5.8, unchanged */ }
#[async_trait] pub trait McpConnector: Send + Sync {
    async fn connect(&self, server: &McpServerConfig) -> Result<Arc<dyn McpTransport>>;
}
pub struct WriteScope { prefixes: Vec<String> }
impl WriteScope {
    pub fn new(prefixes: Vec<String>) -> Result<Self>;  // each must pass validate_safe_relative_path
    pub fn anywhere() -> Self;                           // empty prefix list = whole project
    pub fn contains(&self, path: &str) -> bool;
}
pub fn scopes_overlap(a: &str, b: &str) -> bool;
pub fn tool_result_is_error(result: &serde_json::Value) -> bool;  // result["isError"] == true
pub fn tool_result_text(result: &serde_json::Value) -> String;    // "\n"-joined result["content"][i]["text"] strings
pub struct McpGatekeeper { /* transports: HashMap<String, Arc<dyn McpTransport>>, write_tools: HashMap<String, Vec<WriteToolSpec>>, catalog: Vec<McpToolSchema> */ }
impl McpGatekeeper {
    pub fn new() -> Self;
    pub fn register(&mut self, server_id: impl Into<String>, transport: Arc<dyn McpTransport>, write_tools: Vec<WriteToolSpec>);
    pub async fn refresh_catalog(&mut self) -> Result<()>;
    pub fn is_tool_allowed(role: &RoleConfig, server_id: &str, tool: &str) -> bool; // associated fn
    pub fn is_write_tool(&self, server_id: &str, tool: &str) -> bool;
    pub fn tools_for_role(&self, role: &RoleConfig) -> Vec<McpToolSchema>;
    pub async fn execute_for_role(&self, role: &RoleConfig, scope: &WriteScope, server_id: &str, tool: &str, args: serde_json::Value) -> Result<serde_json::Value>;
}
pub struct MockMcpTransport { /* server_id, tools, results, calls: Mutex<Vec<(String, Value)>> */ }
impl MockMcpTransport {
    pub fn new(server_id: impl Into<String>) -> Self;
    pub fn with_tool(self, name: &str, result: serde_json::Value) -> Self;   // description "mock", input_schema {"type":"object"}
    pub fn with_failing_tool(self, name: &str, message: &str) -> Self;       // call → Error::Mcp
    pub fn calls(&self) -> Vec<(String, serde_json::Value)>;
}
pub struct RmcpTransport { /* server_id, rmcp::service::RunningService<rmcp::RoleClient, ()> */ }
impl RmcpTransport { pub async fn connect(server: &McpServerConfig, secrets: &dyn SecretStore) -> Result<Self>; pub async fn connect_in(server: &McpServerConfig, secrets: &dyn SecretStore, workdir: &Path) -> Result<Self>; }   // connect_in added (Step 26)
pub struct RmcpConnector { pub secrets: Arc<dyn SecretStore>, pub project_root: PathBuf, pub search_path: Option<std::ffi::OsString> }   // project_root, search_path added (Step 26); Stdio → resolve_stdio_command then RmcpTransport::connect_in(.., &project_root)
pub struct MockMcpConnector { pub transports: HashMap<String, Arc<dyn McpTransport>> } // unknown id → Error::Mcp
```
- Path normalization for `contains`/`scopes_overlap`: split on `/`, drop empty and `.` segments; any `..` segment or a leading `/` → not contained / treated as overlapping everything. `contains`: empty prefix list → true; else some prefix's segments are a leading run of the path's segments. Overlap: either segment list is a leading run of the other (`src` vs `src/lib.rs` overlap; `src` vs `src2` don't).
- `execute_for_role` order: (0) `args` not a JSON object → `Error::Security` (so no shape skips the path checks); (1) not allowed → `Error::Security`; (2) server not registered → `Error::Security`; (3) write tool: each `path_args` key present in `args` must be a JSON string with `scope.contains(..)`, else `Error::Security`; (4) `transport.call_tool(tool, args)`.
- `RmcpTransport::connect(server, secrets)` = `connect_in(server, secrets, Path::new("."))`; `connect_in(.., workdir: &Path)`: `Stdio` → `rmcp::transport::TokioChildProcess::new(tokio::process::Command::new(command).args(args).current_dir(workdir))` then `rmcp::ServiceExt::serve((), transport).await`. `StreamableHttp` → if a keychain pair is set, `secrets.get(..)?` **first** (before any network); `StreamableHttpClientTransportConfig::with_uri(endpoint_url)` + `.auth_header(secret.expose())` (token without `Bearer `); `StreamableHttpClientTransport::from_config(cfg)`; then `serve`. `list_tools` → `peer().list_all_tools()`; `call_tool` → `CallToolRequestParams::new(name.to_owned()).with_arguments(obj)` (non-object args → `Error::Validation`), result via `serde_json::to_value`. Every rmcp error → `Error::Mcp { server_id, source: Box::new(e) }`. `resolve_stdio_command(command: &str, project_root: &Path, search_path: Option<&OsStr>) -> Result<PathBuf>` (`mcp.rs`, Step 26): No '/' or '\\' → first absolute search_path entry holding a regular file (unix: any exec bit) named `command`; relative/empty entries skipped; none → Error::Validation { field: "transport.command", message: "'{command}' not found on PATH" }. Absolute → canonicalize (missing → Error::Io), regular file, unix exec bit, else Error::Validation. Relative with a separator → Error::Validation. Result inside canonical project_root and not under <root>/.orrery → Error::Security { "stdio command is inside the agent-writable workspace" }

### 9.6 `docker.rs`
```rust
pub enum EnvValue { Plain(String), Secret(SecretValue) }
pub enum MountSpec { Bind { host: PathBuf, container: String, read_only: bool } }   // AgentCli only; no volumes
pub enum ContainerState { Missing, Created, Running, Exited { code: i64 } }
pub struct ExitStatus { pub code: i64 }                           // own type, not std's
pub struct ContainerSpec { pub name: String, pub image: String, pub cmd: Vec<String>, pub env: Vec<(String, EnvValue)>,
    pub mounts: Vec<MountSpec>, pub memory_limit_mb: u64, pub auto_remove: bool }   // no port, no GPU (overrides §5.7)
#[async_trait] pub trait ContainerBackend: Send + Sync {   // (overrides §5.7: `ensure_running(spec)` → `start(name)`)
    async fn start(&self, name: &str) -> Result<()>;   // existing container only; Missing → Error::Validation { field: "container", message: "container '{name}' not found; Orrery never creates inference containers" }
    async fn stop(&self, name: &str) -> Result<()>;
    async fn remove(&self, name: &str) -> Result<()>;
    async fn inspect(&self, name: &str) -> Result<ContainerState>;
    async fn run_to_completion(&self, spec: &ContainerSpec, stdin: Vec<u8>, on_output: &(dyn Fn(String) + Send + Sync)) -> Result<ExitStatus>;  // AgentCli: creates, attaches, starts; writes stdin concurrently with draining output (BrokenPipe on stdin is not an error); waits; no exit status → Error::Validation
}
#[async_trait] pub trait ComposeRunner: Send + Sync {
    async fn up(&self, file: &Path, service: Option<&str>) -> Result<()>;        // docker compose -f <file> up -d [service]
    async fn down(&self, file: &Path, service: Option<&str>) -> Result<()>;      // no service: docker compose -f <file> down; service: docker compose -f <file> stop <service>
    async fn is_running(&self, file: &Path, service: Option<&str>) -> Result<bool>; // docker compose -f <file> ps --status running -q [service] → non-empty stdout
}
pub struct DockerComposeCli;   // runs `docker compose …` via tokio::process::Command; a relative `file` → Error::Validation { field: "compose", message: "compose file must be resolved by ConfigStore::load_inference" } before any process starts; non-zero exit → Error::Validation { field: "compose", message: <stderr, redacted> }
#[async_trait] pub trait InferenceControl: Send + Sync {
    async fn is_running(&self, inference: &InferenceConfig) -> Result<bool>;   // External → true
    async fn start(&self, inference: &InferenceConfig) -> Result<()>;          // Container → backend.start; Compose → up; External → Ok
    async fn stop(&self, inference: &InferenceConfig) -> Result<()>;           // Container → backend.stop; Compose → down; External → Ok
    async fn wait_ready(&self, inference: &InferenceConfig) -> Result<()>;     // GET {endpoint_url}/health (2 s per-request timeout) every 1 s until 200; > startup_timeout_secs → Error::Agent { agent_id: inference_id, message: "not ready" }
    async fn unload(&self, inference: &InferenceConfig, model: &str) -> Result<()>; // Router: POST {endpoint_url}/models/unload {"model": m}; Single: Ok (no-op)
}
pub struct DockerInferenceControl { pub containers: Arc<dyn ContainerBackend>, pub compose: Arc<dyn ComposeRunner>, client: reqwest::Client }
impl DockerInferenceControl { pub fn new(containers: Arc<dyn ContainerBackend>, compose: Arc<dyn ComposeRunner>) -> Self; }
pub enum InferenceCall { IsRunning(String), Start(String), Stop(String), WaitReady(String), Unload(String, String) }   // inference_id[, model]
pub struct MockInferenceControl { /* running: Mutex<HashSet<String>>, calls: Mutex<Vec<InferenceCall>> */ }
impl MockInferenceControl { pub fn new() -> Self; pub fn set_running(&self, inference_id: &str, running: bool); pub fn calls(&self) -> Vec<InferenceCall>; }
pub fn unload_request(inference: &InferenceConfig, model: &str) -> Option<(String, serde_json::Value)>; // Router → Some(("{endpoint}/models/unload", {"model": m})); Single → None
pub enum MockCall { Start(String), Stop(String), Remove(String), Inspect(String), RunToCompletion { name: String, stdin_len: usize, secret_env: Vec<String>, plain_env: Vec<String> } }
pub type MockRunScript = Box<dyn Fn(&ContainerSpec, &[u8]) -> (Vec<String>, i64) + Send + Sync>;
pub struct MockContainerBackend { /* calls, states, script, delay */ }
impl MockContainerBackend {
    pub fn new() -> Self;
    pub fn add_container(&self, name: &str, state: ContainerState);   // pre-existing container (start of an unknown name fails)
    pub fn calls(&self) -> Vec<MockCall>;
    pub fn set_run_script(&self, script: MockRunScript);   // default: no output, code 0
    pub fn set_run_delay(&self, delay: std::time::Duration);
}
pub fn cli_container_name(run_id: &uuid::Uuid, agent_id: &str) -> String; // "orrery-cli-" + first 8 chars of run_id.simple() + "-" + agent_id
pub fn agent_io_dir(project_root: &Path, run_id: &uuid::Uuid, agent_id: &str) -> PathBuf; // <root>/.orrery/agent_io/<run_id>/<agent_id>
pub fn effective_concurrency(max_concurrent_agents: usize, writer: bool, agents: &[AgentConfig], allocations: &[SubtaskAllocation]) -> usize;
pub fn mounts_for(agent: &AgentConfig, project_root: &Path, agent_io_dir: &Path) -> Vec<MountSpec>;

pub struct ResourceManager { /* backend, inference_control, inference, max, project_root, lease_open: Arc<AtomicBool>, started: Mutex<HashSet<String>>, held: Mutex<Vec<(String, String)>> */ }
impl ResourceManager {   // not generic (overrides §5.7 `ResourceManager<B>`)
    pub fn new(backend: Arc<dyn ContainerBackend>, inference_control: Arc<dyn InferenceControl>, inference: Vec<InferenceConfig>, max_concurrent_agents: usize, project_root: PathBuf) -> Self;
    pub async fn begin_stage(&self, run_id: uuid::Uuid, agents: &[AgentConfig], writer: bool, allocations: &[SubtaskAllocation]) -> Result<StageLease>;
    pub async fn end_stage(&self, lease: StageLease) -> Result<()>;
    pub async fn shutdown(&self) -> Result<()>;
}
pub struct StageLease { /* run_id, semaphore: Arc<Semaphore>, concurrency, cli_agents: Vec<String>, open: Arc<AtomicBool> */ }
impl StageLease { pub async fn acquire_slot(&self) -> Result<SlotPermit>; pub fn concurrency(&self) -> usize; }
impl Drop for StageLease { /* open.store(false) — a dropped lease never blocks the next stage */ }
pub struct SlotPermit { _permit: tokio::sync::OwnedSemaphorePermit }   // named `_permit` so no dead_code allow is needed
pub struct BollardContainerBackend { docker: bollard::Docker }
impl BollardContainerBackend {
    pub fn connect() -> Result<Self>;  // Docker::connect_with_local_defaults() (honors DOCKER_HOST); no socket argument (overrides §5.7)
}
pub fn create_body(spec: &ContainerSpec) -> bollard::models::ContainerCreateBody; // AgentCli containers only; the only place EnvValue::Secret is exposed
```
- `effective_concurrency`: `cap = max(1, min(max_concurrent_agents, agents.len()))`. Non-writer → `cap`. Writer → `cap` only if allocations is non-empty, every allocation has a non-empty `write_scope`, no two scope entries across different allocations overlap (`scopes_overlap`), and no agent is `AgentCli` with `ReadWrite`; else `1`.
- `mounts_for`: `LocalModel`/`CloudApi` → `[]`. `AgentCli` → project `Bind` at `/workspace` (`ReadOnly` → `read_only: true`, `ReadWrite` → `false`, `None` → no project mount), then, only when the project is mounted, `Bind { agent_io_dir/mask, "/workspace/.orrery", true }`, plus `Bind { agent_io_dir/in, "/orrery/in", true }` and `Bind { agent_io_dir/out, "/orrery/out", false }`.
- `begin_stage`, in order:
  1. `lease_open.swap(true)` was `true` → `Error::Engine { step_number: 0, message: "stage lease already open" }`.
  2. `needed` = distinct `(inference_id, model)` of the stage's `LocalModel` agents; unknown `inference_id` → `Error::Validation`.
  3. **Release what the previous stage held and this one doesn't need** (only for servers with `unload_after_stage`): `Router` → `unload(inference, model)` for each held model not in `needed`; `Single` → if the server isn't in `needed` and is in `started`, `stop` it and remove it from `started`.
  4. For each server in `needed`: `is_running` false → `start`, add to `started`; then `wait_ready`.
  5. `held = needed`.
- `end_stage`: `remove(cli_container_name(..))` for each CLI agent (backends return `Ok` for a missing container; every other error propagates, since a surviving container still holds secret env); `remove_dir_all(<root>/.orrery/agent_io/<run_id>)` if it exists; `open = false`. Models stay loaded until the next `begin_stage` decides (step 3), so back-to-back stages on the same model don't reload it.
- `shutdown`: `unload` every held `Router` model (when `unload_after_stage`), then `stop` every server in `started`. Servers Orrery found already running are never stopped.
- `create_body`: `image`, `cmd`, `env` as `"K=V"`, `host_config.binds` (`host:container[:ro]`), `memory = mb * 1024 * 1024`, `auto_remove`, `open_stdin`/`attach_stdin`/`stdin_once: true`. No GPU or port settings: Orrery never configures GPUs (inference servers are the operator's own; AgentCli containers don't need one).

### 9.6a Inference servers (`config.rs`)
Orrery attaches to model servers the operator already runs (any GPU stack, e.g. ROCm, is configured in the operator's own container or compose file). It never creates, configures or deletes an inference server. Path scope: a project-scope entry (`.orrery/inference.toml`, `project.toml`) may hold only a relative `Compose.file`, resolved under `<root>/.orrery/`; the machine-scope `<global_dir>/inference.toml` may also hold an absolute one (relative resolves under `global_dir`). This keeps host paths out of anything committable.
```rust
#[serde(deny_unknown_fields)]
pub struct InferenceConfig {     // replaces the Step 2 shape (overrides §4.3's original fields)
    pub inference_id: String,
    pub endpoint_url: String,                     // base URL, e.g. "http://127.0.0.1:8080"; http or https
    #[serde(default)] pub mode: InferenceMode,
    #[serde(default)] pub lifecycle: InferenceLifecycle,
    #[serde(default = "default_true")] pub unload_after_stage: bool,
    #[serde(default = "default_startup_timeout")] pub startup_timeout_secs: u64,   // 300
}
#[derive(Default)] pub enum InferenceMode { #[default] Single, Router }
#[derive(Default)] #[serde(tag = "type", deny_unknown_fields)]
pub enum InferenceLifecycle { #[default] External, Container { name: String }, Compose { file: PathBuf, #[serde(default)] service: Option<String> } }
impl InferenceConfig {
    pub fn chat_url(&self) -> String;     // endpoint_url without trailing '/' + "/v1/chat/completions"
    pub fn validate(&self) -> Result<()>; // id non-empty; endpoint starts with http:// or https://; no embedded secret (check_no_embedded_secret); Container name non-empty; Compose file non-empty; startup_timeout_secs >= 1. Also (Step 14): `pub enum ConfigScope { Project, Machine }`; `pub fn resolve_compose_file(file: &Path, scope: ConfigScope, project_root: &Path, global_dir: Option<&Path>) -> Result<PathBuf>`: Project + absolute → Error::Validation { field: "lifecycle.file", message: "project config must use a path relative to .orrery/" }; Project + relative → resolve_safe_within_dir(<root>/.orrery, file); Machine + relative → global_dir.join(file) (no global_dir → Error::Validation); then canonicalize (missing → Error::Io), must be a regular file (else Error::Validation); canonical path inside canonical project_root and not under <root>/.orrery → Error::Security { "compose file is inside the agent-writable workspace" }. `ConfigStore::load_inference` replaces every Compose `file` with this result (in memory only; global entries as Machine, project entries as Project). Project-scope MCP entries (`.orrery/mcp/servers.toml`): a Stdio `command` containing '/' or '\\' or an absolute `args` entry → Error::Validation (checked on load and save)
}
```

| `mode` | Server | Unload between stages | `LocalModel.model` |
|---|---|---|---|
| `Single` (default) | One model per `llama-server` (`-m <file>`). | Stop the server (container or compose service) if Orrery started it. | Any string; the server ignores it. |
| `Router` | `llama-server --models-dir <dir>` (no `-m`); loads a model on the first request naming it. | `POST {endpoint_url}/models/unload` body `{"model": m}`. | An id from the server's `GET /v1/models`. |

| `lifecycle` | Orrery may |
|---|---|
| `External` (default) | Only send HTTP requests. Never start or stop anything. |
| `Container { name }` | Start that existing container if it isn't running; stop it later only if Orrery started it. |
| `Compose { file, service }` | `docker compose -f <file> up -d [service]` if not running; `down` (or `stop <service>`) later only if Orrery started it. |

Readiness: `GET {endpoint_url}/health` returns 503 while a model loads and 200 when ready. Source: `ggml-org/llama.cpp` `tools/server/README.md` (checked 2026-11-12, release v0.9.0). The server sends `reasoning_content` deltas, which §9.7's `SseParser` already reads.

### 9.7 `llm.rs`
```rust
pub struct AgentInvocation { pub run_id: uuid::Uuid, pub step_number: usize, pub stage_index: usize, pub stage_count: usize, pub round: usize,
    pub role: RoleConfig, pub agent: AgentConfig, pub system_prompt: String, pub user_prompt: String,
    pub tools: Vec<McpToolSchema>, pub write_scope: WriteScope }      // run_id, stage_count added (overrides §5.9)
pub struct StageSummary { pub index: usize, pub role_id: String, pub kind: RoleKind, pub agent_ids: Vec<String> }
pub const SUBMIT_REPORT_TOOL: &str = "submit_report";
pub const CLI_REPORT_PATH: &str = "/orrery/out/report.toml";
pub fn build_system_prompt(role: &RoleConfig, rules_markdown: &str, backend: &AgentBackend) -> String;   // rules param added (overrides §5.9)
pub fn build_user_prompt(step: &PlanStep, allocation: Option<&SubtaskAllocation>, prior: &[StampedReport], notes: &[String]) -> String;
pub fn build_orchestrator_prompt(step: &PlanStep, stages: &[StageSummary], prior: &[StampedReport], notes: &[String]) -> String;
pub fn parse_mapped_tool_name(name: &str) -> Result<(String, String)>; // split at first "__", then format_mapped_tool_name must round-trip
#[async_trait] pub trait AgentDriver: Send + Sync { /* §5.9, unchanged */ }
pub struct MockAgentDriver { /* scripts: Mutex<HashMap<String, VecDeque<Result<AgentReport>>>>, invocations, delay, in_flight, max_in_flight */ }
impl MockAgentDriver {
    pub fn new() -> Self;
    pub fn push(&self, agent_id: &str, report: Result<AgentReport>);
    pub fn set_delay(&self, delay: std::time::Duration);
    pub fn invocations(&self) -> Vec<AgentInvocation>;   // AgentInvocation derives Clone
    pub fn max_in_flight(&self) -> usize;
}   // empty script → Error::Agent { agent_id, message: "mock script exhausted" }
```
- System prompt text: `"# Role: {display_name}\n\n{rules_markdown}\n\n# Reporting\n"` + for `LocalModel`/`CloudApi`: `"End your turn by calling the `submit_report` tool exactly once."`; for `AgentCli`: `"End your turn by writing your report as TOML to /orrery/out/report.toml."` + the `report_json_schema()` pretty-printed in a fenced `json` block.
- User prompt sections, in order, each omitted when empty: `"# Plan step {number}: {title}\n\n{body}"`; `"# Your assignment\n{instructions}\nWrite scope: {comma list or \"whole project\"}"`; `"# Prior reports\n"` + one line per report `"- round {r}, stage {s}, {agent_id}: {status:?}: {summary}"` + `"  feedback: {f}"` when present; `"# Notes from Orrery\n"` + one `"- {note}"` per note. Orchestrator prompt adds `"# Stages\n"` + one line per stage `"- {index}: {role_id} ({kind:?}) agents: {ids}"` after the plan step section.

```rust
pub struct HttpChatDriver { client: reqwest::Client, secrets: Arc<dyn SecretStore>, inference: Vec<InferenceConfig>, max_tool_calls: usize }
impl HttpChatDriver { pub fn new(secrets: Arc<dyn SecretStore>, inference: Vec<InferenceConfig>) -> Self; pub fn with_max_tool_calls(self, n: usize) -> Self; } // default 50
pub fn endpoint_for(agent: &AgentConfig, inference: &[InferenceConfig]) -> Result<String>;
pub fn build_chat_request(inv: &AgentInvocation, messages: &[serde_json::Value]) -> serde_json::Value;
pub struct ToolCallDelta { pub index: usize, pub id: Option<String>, pub name: Option<String>, pub arguments: String }
pub struct ChatChunk { pub content: Option<String>, pub reasoning: Option<String>, pub tool_calls: Vec<ToolCallDelta>, pub finish_reason: Option<String> }
#[derive(Default)] pub struct SseParser { buf: Vec<u8> }   // bytes: decode only complete lines, so a UTF-8 char split across chunks survives
impl SseParser { pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseItem>>; }
pub enum SseItem { Chunk(ChatChunk), Done }
pub struct AssembledToolCall { pub id: String, pub name: String, pub arguments: String }
#[derive(Default)] pub struct ToolCallAccumulator { calls: BTreeMap<usize, AssembledToolCall> }
impl ToolCallAccumulator { pub fn apply(&mut self, delta: &ToolCallDelta); pub fn finish(self) -> Vec<AssembledToolCall>; }
```
- `endpoint_for`: `LocalModel` → `chat_url()` of the matching `inference_id` (missing → `Error::Validation`); `CloudApi` → `endpoint_url`; `AgentCli` → `Error::Validation`.
- `build_chat_request`: `{"model", "temperature", "stream": true, "messages", "tools"}`; `tools` = each `inv.tools` entry as `{"type":"function","function":{"name": format_mapped_tool_name(server_id, name), "description", "parameters": input_schema}}` plus the `submit_report` tool with `parameters: report_json_schema()`.
- `SseParser`: split the byte buffer on `b'\n'`, keep a trailing partial line in `buf`, decode each complete line as UTF-8; ignore blank lines and lines not starting with `"data:"`; `"data: [DONE]"` → `Done`; else parse JSON `choices[0].delta.{content, reasoning_content | reasoning, tool_calls[]}` and `choices[0].finish_reason`. Bad JSON → `Error::Json`.
- Turn loop (`run_turn`): messages = `[system, user]`. Repeat: POST (Bearer from `secrets.get` only for `CloudApi`, fetched right before the request), stream body through `SseParser`, buffer `Output`/`Reasoning` deltas per kind and emit one event per complete line (flush the remainder at end of response, and when a buffer passes 4096 bytes at a char boundary) — redaction runs per event, so per-delta events would let a secret split across deltas through; accumulate tool calls; an assembled call with an empty `id` gets `"orrery_call_{n}"` (n counts calls across the turn), used in both the assistant message and its `tool` reply. On end of response: if there are tool calls, append the assistant message with them; then for each call: `submit_report` → parse `AgentReport` from `arguments` + `validate_for_role`, success → return it; failure → (first time) append a `tool` message with the error and continue, (second time) `Error::Report`. Other names → `parse_mapped_tool_name` + `mcp.execute_for_role(&inv.role, &inv.write_scope, ..)`; append a `tool` message with `tool_result_text` or `"error: {e}"`; emit a `ToolCall` event `"{server}:{tool} ok|denied: {e}"`. Tool calls counted across the turn; `> max_tool_calls` → `Error::Agent`. No tool calls at all → append user message `"You must call submit_report."` once, second time `Error::Report`. HTTP non-2xx → `Error::Agent { message: "HTTP {status}" }`; transport errors → `Error::Http`.

```rust
pub struct CliContainerDriver { backend: Arc<dyn ContainerBackend>, secrets: Arc<dyn SecretStore>, project_root: PathBuf }
impl CliContainerDriver { pub fn new(backend: Arc<dyn ContainerBackend>, secrets: Arc<dyn SecretStore>, project_root: PathBuf) -> Self; }
```
- `run_turn`: `dir = agent_io_dir(root, run_id, agent_id)`; create `dir/in`, `dir/out` and, if the agent mounts the project, an empty `dir/mask`; prompt = system + `"\n\n"` + user; write `dir/in/prompt.md`; secret env (if `secret_env` + keychain pair set) = `EnvValue::Secret(secrets.get(..)?)` fetched before the run; spec = `{ name: cli_container_name, image, cmd: command, env, mounts: mounts_for(..), memory_limit_mb, auto_remove: false }`; `tokio::time::timeout(timeout_secs, backend.run_to_completion(&spec, prompt bytes, &|line| emit Output))`. Timeout → `backend.remove(name)` then `Error::Agent { message: "timed out after {n}s" }`. Before each run, delete any existing `dir/out/report.toml` (a stale report is never read). Then read `dir/out/report.toml` (errors name `CLI_REPORT_PATH`, never the host path, since they reach the retry prompt and the stream) → `toml::from_str` → `validate_for_role`. Missing/invalid → delete the file, re-run once with prompt + `"\n\n# Previous report was invalid\n{error}"`; second failure → `Error::Report`. After each run: `backend.remove(name)`.

### 9.8 `engine.rs`
```rust
pub struct DriverSet { pub chat: Arc<dyn AgentDriver>, pub cli: Arc<dyn AgentDriver> }  // LocalModel/CloudApi → chat, AgentCli → cli
pub struct EngineContext {
    pub project_root: PathBuf, pub project: ProjectConfig, pub roles: Vec<RoleConfig>, pub role_rules: HashMap<String, String>,
    pub agents: Vec<AgentConfig>, pub servers: Vec<McpServerConfig>, pub plan: ImplementationPlan,
    pub checkpoints: CheckpointManager, pub stream: Arc<LiveStreamHub>, pub gatekeeper: Arc<McpGatekeeper>,
    pub resources: Arc<ResourceManager>, pub drivers: DriverSet,
}
#[derive(Debug, Clone, Default)]
pub struct RunOptions { pub mode: Option<ExecutionMode>, pub cycles: Option<usize>, pub start_step: Option<usize>, pub wait_for_operator: bool }
pub enum RunOutcome { Completed { steps_passed: usize }, NoPendingSteps, AwaitingOperator { step_number: usize, reason: String }, Interrupted }
pub enum EngineEvent { PhaseChanged(EnginePhase), StepStatusChanged { step_number: usize, status: StepStatus },
    RoundStarted { step_number: usize, round: usize }, AwaitingOperator { step_number: usize, reason: String }, RunFinished(RunOutcome) }
pub enum DispatchKind { Forward, Retry }
pub struct StageTurnRecord { pub stage_index: usize, pub kind: RoleKind, pub expected_agents: Vec<String>, pub statuses: Vec<(String, ReportStatus)> }
pub fn check_dispatch(target: usize, last_stage_run: Option<usize>, kinds: &[RoleKind]) -> Result<DispatchKind>;
pub fn agents_pass_rule(history: &[StageTurnRecord], gate_ok: bool) -> std::result::Result<(), String>;
pub struct WorkflowEngine { /* ctx, events: broadcast::Sender<EngineEvent>, operator_tx/rx: mpsc (capacity 8), current_lease: tokio::sync::Mutex<Option<StageLease>> */ }
impl WorkflowEngine {
    pub fn new(ctx: EngineContext) -> Result<Self>;  // project.validate_for_run(..) must pass
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EngineEvent>;  // capacity 256
    pub fn operator_sender(&self) -> tokio::sync::mpsc::Sender<OperatorDecision>;
    pub async fn run(&self, opts: RunOptions, cancel: CancellationToken) -> Result<RunOutcome>;  // opts added (overrides §5.10.6)
}
pub struct Backends { pub secrets: Arc<dyn SecretStore>, pub containers: Arc<dyn ContainerBackend>, pub inference_control: Arc<dyn InferenceControl>,
                      pub connector: Arc<dyn McpConnector>, pub drivers: DriverSet }
impl Backends { pub fn real(project_root: &Path) -> Result<Self>; }   // OsKeyringStore, BollardContainerBackend::connect(), DockerInferenceControl::new(containers, Arc::new(DockerComposeCli)), RmcpConnector { secrets, project_root, search_path: std::env::var_os("PATH") }, Http + Cli drivers
pub async fn build_engine(store: &ConfigStore, backends: Backends) -> Result<WorkflowEngine>;
```
- `check_dispatch`: `next = last_stage_run.map_or(1, |l| l + 1)`. `target == next && target < kinds.len()` → `Forward`. `1 <= target < next` and `kinds[target]` is not `Validator` → `Retry`. Anything else → `Error::Engine { step_number: 0, message }` (caller fills the step number).
- `agents_pass_rule`: find the last record with `kind == Validator` (none → `Err`); every `expected_agents` entry has `Confirmed` in it; no `Implementer` record after it; `gate_ok`. `Err` text names the first failed condition.
- Gates run at pass-check time (when the orchestrator reports `StepPassed`), in both modes — that is "after the last implementer turn" because a passing step has no implementer turn after it. A gate run calls each `gate_tools` entry in order through `execute_for_role` with role `"__orrery_gate"` (allowlist = `gate_tools`), `WriteScope::anywhere()`, args = the §5.10.4 `orrery` object; success = `Ok` and not `tool_result_is_error`; stop at the first failure; emit one `GateResult` event per tool.
- Hooks use role `"__orrery_hooks"` (allowlist = the configured hook tools), `WriteScope::anywhere()`, the same args object with `event` = `"step_start" | "step_pass" | "step_retry" | "halt"`.
- Operator decisions: awaiting after a pass check (`GateAndOperator`) → `Approve` passes the step, `Reject` counts a retry and adds the feedback as a note. Awaiting after `Escalate`, exhausted retries, a protocol violation repeated twice, or a `Halt` hook → `Approve` resets `retry_count` to 0 and restarts the round's orchestrator turn; `Reject` does the same and adds the feedback as a note. There is no "abort" decision; the operator stops the run (§5.10.1's abort edge = stop).
- `wait_for_operator = true` (TUI): await the operator channel. `false` (CLI): save the checkpoint with `AwaitingOperator`, `awaiting_reason`, and return `RunOutcome::AwaitingOperator`. A later `run` that finds `operator_decision` set applies it, clears it, and continues.
- `steps_passed_this_run` resets to 0 when `run` starts with no `InProgress`/`AwaitingOperator` step; otherwise it is kept.
- Cancel: every await inside `run` is raced with `cancel.cancelled()` via `tokio::select!`. On cancel: take `current_lease` and `end_stage` it (the stage-turn future is dropped first, so no clone of the lease remains); a step already `AwaitingOperator` keeps that status, phase and `awaiting_reason`, is saved, and returns `RunOutcome::Interrupted` without re-running `on_halt` (amended 2026-11-15); otherwise set the step `InProgress`, `phase = OrchestratorTurn`, keep `round`; save; run `on_halt`; return `RunOutcome::Interrupted`. On resume of an `InProgress` step: `delete_reports(step, round)` then restart that round from its orchestrator turn.

### 9.9 `orrery-tui`
```rust
// layout.rs
pub const MIN_WIDTH: u16 = 80; pub const MIN_HEIGHT: u16 = 24; pub const BANNER_MIN_HEIGHT: u16 = 40; pub const MIN_STAGE_COL_WIDTH: u16 = 18;
pub enum UiRegion { IngestButton, ModeToggle, CyclesControl, PassedCounter, StagesMinus, StagesPlus, StepsList,
    StageHeader(usize), AgentSlot { stage: usize, slot: usize }, LiveStream,
    RunStopButton, ApproveButton, RejectButton, SettingsButton, HelpButton, MouseToggle, QuitButton }
pub struct StageColumn { pub index: usize, pub area: Rect, pub header: Rect, pub slots: Vec<(usize, Rect)> }
pub struct UiLayoutRegions { pub too_small: bool, pub header: Rect, pub top_menu: Rect, pub main_workspace: Rect, pub steps_panel: Rect,
    pub stage_canvas: Rect, pub live_stream: Rect, pub footer_menu: Rect, pub top_items: Vec<(UiRegion, Rect)>,
    pub footer_items: Vec<(UiRegion, Rect)>, pub stage_columns: Vec<StageColumn> }
impl UiLayoutRegions {
    pub fn compute(area: Rect, stage_count: usize, stage_scroll: usize, slot_scroll: usize) -> Self;
    pub fn region_at(&self, col: u16, row: u16) -> Option<UiRegion>;
}
```
- `too_small` when `width < 80 || height < 24`; then every other rect is `Rect::default()` and the item lists are empty.
- Menu items: the inner row of each bordered menu (`x + 1`, `y + 1`, height 1). Items placed left to right with 1 column gap, each rect as wide as its fixed label. Top labels (widths fixed to the widest variant; they total 77 columns with gaps, inside the 78-column inner row at 80×24 — amended 2026-11-15, "[Ingest Plan]" did not fit): `"[Ingest]"`, `"[Mode: LOOP]"`, `"[Cycles: 999]"`, `"[Passed: 999/999]"`, `"[Stages:"` (not a region), `"[-]"` → `StagesMinus`, `" 15 "` (not a region), `"[+]"` → `StagesPlus`, `"]"` (not a region). Footer labels: `"[Stop]"` (`RunStopButton`, also used for `[Run]`), `"[Approve]"`, `"[Reject]"`, `"[Settings]"`, `"[Help]"`, `"[Mouse: OFF]"`, `"[Quit]"`. `ApproveButton`/`RejectButton` rects always exist in `footer_items` and `region_at` always returns them. Visibility is not a layout concern: widgets draw them only when `App::approve_visible()`, and the app ignores activations of them otherwise.
- Stage canvas inner area = `stage_canvas` minus its border. `visible = min(stage_count - stage_scroll, max(1, inner.width / 18))`; columns split the width evenly, the last takes the remainder. Column header = top 3 rows; slots = following rows, one per slot index `slot_scroll..15` until the column height runs out.

```rust
// app.rs
pub enum Focus { TopMenu, StepsPanel, StageCanvas, LiveStream, FooterMenu }
pub enum RunStatus { Idle, Running, AwaitingOperator, Passed, Error }
pub enum SelectorKind { Role, Agent }
pub struct Selector { pub stage: usize, pub slot: usize, pub kind: SelectorKind, pub options: Vec<String>, pub index: usize }
pub enum Modal { None, Ingest { input: String }, Settings, Help, QuitConfirm, RejectFeedback { input: String }, Selector(Selector) }
pub enum AppAction { Run, Interrupt, Approve, Reject(String), IngestPlan(PathBuf), SetMode(ExecutionMode), SetCycles(usize),
    SetStageCount(usize), SetStageRole { stage: usize, role_id: String }, AssignAgent { stage: usize, agent_id: String },
    UnassignAgent { stage: usize, agent_id: String }, SetMouseCapture(bool), Quit }
pub struct App { pub project_root: PathBuf, pub config: ProjectConfig, pub roles: Vec<RoleConfig>, pub agents: Vec<AgentConfig>, pub mcp_servers: Vec<McpServerConfig> /* new() → empty */,
    pub plan: Option<ImplementationPlan>, pub step_statuses: Vec<StepStatus>, pub focus: Focus, pub modal: Modal,
    pub top_index: usize, pub footer_index: usize, pub step_selected: usize, pub stage_selected: usize, pub slot_selected: usize,
    pub step_scroll: usize, pub stage_scroll: usize, pub slot_scroll: usize, pub stream: VecDeque<StreamEvent>, pub stream_scroll: usize /* 0 = follow */,
    pub mouse_capture: bool /* default true */, pub status: RunStatus, pub awaiting_operator: bool, pub status_message: Option<String>, pub should_quit: bool }
// App derives Debug, Clone, PartialEq (tests compare before/after snapshots).
impl App {
    pub fn new(project_root: PathBuf, config: ProjectConfig, roles: Vec<RoleConfig>, agents: Vec<AgentConfig>) -> Self;
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Vec<AppAction>;
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent, regions: &UiLayoutRegions) -> Vec<AppAction>;
    pub fn apply_engine_event(&mut self, event: &EngineEvent);
    pub fn push_stream_event(&mut self, event: StreamEvent);   // cap 2_000, pop front
    pub fn unlocked_slots(&self, stage: usize) -> usize;       // no role → 0; orchestrator → 1; else min(15, agents + 1)
    pub fn approve_visible(&self) -> bool;                     // awaiting_operator && effective mode == GateAndOperator
}
```
Key table (`handle_key_event`), first matching row wins:
1. `Ctrl+C` → running: `[Interrupt]`; idle: `[]`. Any other key with `CONTROL` or `ALT` → `[]`, no state change.
2. Text modal (`Ingest`, `RejectFeedback`): printable char appends; `Backspace` pops; `Enter` → `[IngestPlan(path)]` / `[Reject(text)]` and close; `Esc` closes.
3. `Selector`: `Up`/`Down` move; `Enter` → `[SetStageRole]` or `[AssignAgent]` (option `"(remove)"` → `[UnassignAgent]`) and close; `Esc` closes.
4. `QuitConfirm`: `Enter` or `y` → `[Interrupt, Quit]`; `Esc` or `n` closes.
5. `Help`/`Settings`: `Esc`, `Enter` or `q` closes.
6. No modal: `Tab`/`BackTab` cycle focus; `q` → running: open `QuitConfirm`, else `[Quit]` + `should_quit`; `?` → `Help`; `m` → toggle `mouse_capture` and `[SetMouseCapture(new)]`.
7. Focus `TopMenu`: `Left`/`Right` move `top_index` over the 6 regions; `Enter`/`Space`: Ingest → `Ingest` modal; Mode → `[SetMode(toggled)]`; others nothing. `+`/`-` on Cycles → `[SetCycles(n±1)]` (min 0, Run mode: nothing); on Stages −/+ → `[SetStageCount(n±1)]` clamped `2..=15` (no action if unchanged).
8. Focus `StepsPanel`: `Up`/`Down` move selection.
9. Focus `StageCanvas`: `Left`/`Right` stage (adjust `stage_scroll` to keep it visible); `Up`/`Down` slot (`0` = header, up to `unlocked_slots`); `Enter`/`Space` on header → role `Selector`; on an unlocked slot → agent `Selector` (adds `"(remove)"` when the slot holds an agent); `Delete` on an occupied slot → `[UnassignAgent]`.
10. Focus `LiveStream`: `Up`/`Down`/`PageUp`/`PageDown` scroll; `End` → follow (`stream_scroll = 0`).
11. Focus `FooterMenu`: `Left`/`Right` move; `Enter`/`Space`: Run/Stop → `[Run]` when idle, `[Interrupt]` when running; Approve/Reject only when `approve_visible()`; Settings/Help open modals; Mouse → as `m`; Quit → as `q`.
Config edits (`SetStageCount`, `SetStageRole`, `AssignAgent`, `UnassignAgent`, `SetMode`, `SetCycles`) are first applied to `self.config` through the §5.2 methods; on `Err` set `status_message` and return `[]`.
Mouse: ignored when `!mouse_capture`. Left-down → `region_at`; set focus/selection to that region and act as `Enter`. Scroll up/down over `StepsList`, `LiveStream` or a stage column → same as `Up`/`Down` there.

```rust
// widgets.rs
pub fn render(frame: &mut ratatui::Frame, app: &App, regions: &UiLayoutRegions);
// bridge.rs
pub enum BridgeInput { Key(KeyEvent), Mouse(MouseEvent), Resize(u16, u16), Engine(EngineEvent), Stream(StreamEvent), Tick }
pub struct Effects { pub actions: Vec<AppAction>, pub redraw: bool, pub quit: bool }
pub fn step(app: &mut App, regions: &UiLayoutRegions, input: BridgeInput) -> Effects;   // regions param added (overrides §6.5)
pub struct EngineController { /* store, backends factory, cancel, operator tx, task: Option<JoinHandle<Result<RunOutcome>>> */ }
impl EngineController {
    pub fn new(store: ConfigStore, make_backends: Box<dyn Fn() -> Result<Backends> + Send + Sync>) -> Self;
    pub async fn handle(&mut self, action: &AppAction, app: &mut App) -> Result<()>;  // persists config edits via ConfigStore; Run spawns the engine; Interrupt cancels
}
pub async fn run_tui(project_root: PathBuf) -> Result<()>;   // async (overrides §6.5); uses Backends::real
```

### 9.10 `orrery-cli`
- `args.rs`: `#[derive(Parser)] #[command(name = "orrery", args_conflicts_with_subcommands = true)] pub struct Cli { #[arg(long, global = true, default_value = ".")] pub project: PathBuf, #[command(subcommand)] pub command: Option<Command>, pub path: Option<PathBuf> }` and `impl Cli { pub fn into_command(self) -> Command }` (no subcommand → `Command::Tui { path: self.path.unwrap_or(".".into()) }`). `Command` has one variant per §7 row; nested groups use their own `Subcommand` enums (`WorkflowCmd`, `AgentCmd`, `InferenceCmd`, `SecretCmd`, `RoleCmd`, `McpCmd`, `GateCmd`, `HookCmd`). Flag names are exactly §7's. Stage count uses `value_parser = clap::value_parser!(u8).range(2..=15)`.
- `commands.rs`:
```rust
pub struct CliContext { pub out: Box<dyn Write>, pub err: Box<dyn Write>, pub secrets: Arc<dyn SecretStore>,
    pub read_secret: Box<dyn FnMut() -> Result<String>>, pub make_backends: Box<dyn Fn(&Path) -> Result<Backends> + Send + Sync> }
pub async fn dispatch(command: Command, project: &Path, ctx: &mut CliContext) -> Result<u8>;  // returns the exit code
pub fn exit_code_for(err: &Error) -> u8;   // 130 for Interrupted, else 1
```
- `main.rs`: `#[tokio::main] async fn main() -> std::process::ExitCode`; parse with `Cli::parse()` (clap exits 2 itself); build the real `CliContext` (`OsKeyringStore`, `read_secret` = `rpassword::prompt_password("Secret: ")` when stdin `is_terminal()`, else one line from stdin with the trailing newline trimmed); print errors to stderr; return the code.
- JSON shapes: `workflow show --json` → `{"validation_mode": str, "stages": [{"index", "role_id", "agent_ids"}], "warnings": [str]}`. `status --json` → `{"run_id", "phase", "current_step", "round", "retry_count", "awaiting_reason", "steps": [{"number", "title", "status"}]}`. `agent list --json` → array of `AgentConfig`. `role list --json`, `mcp list --json`, `inference list --json` → arrays of their config types.
- `run` prints each live-stream event as `"[{kind:?}] {agent_id or -}: {text}"` and maps `RunOutcome` to exit codes: `Completed`/`NoPendingSteps` → 0, `AwaitingOperator` → 3, `Interrupted` → 130. Ctrl+C: `tokio::signal::ctrl_c()` cancels the token.
- `approve` / `reject --feedback` set `operator_decision` on a checkpoint whose current step is `AwaitingOperator` (else exit 1, "no step awaiting operator") and save it. `resume` = `run` with no overrides.
