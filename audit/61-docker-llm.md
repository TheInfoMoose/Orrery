# Audit: Docker and Agent Drivers (Step 61)

## Scope
- Files read: `crates/orrery-core/src/docker.rs:1-1231` (non-test), `crates/orrery-core/src/docker.rs:1232-2490` and `crates/orrery-core/tests/live_docker.rs:1-195` (tests, C11 only); `crates/orrery-core/src/llm.rs:1-1240` (non-test), `crates/orrery-core/src/llm.rs:1241-3527` and `crates/orrery-core/tests/live_llm.rs:1-192` (tests, C11 only)
- Spec ranges: `ORRERY_TECHNICAL_SPECIFICATION.md:391-417` (§5.7), `431-440` (§5.9), `844-921` (§9.6), `956-1005` (§9.7), `1193-1201` (§9.11.3), `1216-1224` (§9.11.5), `1283-1288` (§9.11.12), `1306-1307` (§9.12.2), plus applicable Phase D–F amendment headings in §9.11–§9.13

## Findings
### F61-1: `BollardContainerBackend::run_to_completion` leaks the container if `attach_container` or `start_container` fails, or if cancelled when no `StageLease` is open
- Severity: High
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/docker.rs:1166`
- Spec: §9.6
- Evidence: `self.docker.create_container(Some(create_opts), create_body(spec)).await.map_err(Error::Docker)?; ... let attached = self.docker.attach_container(&spec.name, Some(attach_opts)).await.map_err(Error::Docker)?; ... self.docker.start_container(&spec.name, Some(start_opts)).await.map_err(Error::Docker)?;`
- Impact: `CliContainerDriver::run_turn` (`llm.rs:1167`) sets `auto_remove: false` and injects `EnvValue::Secret` into the container's environment at `create_container` time (`docker.rs:1054`). If `attach_container` or `start_container` returns `Err`, `run_to_completion` returns `Err(Error::Docker)` before `CliContainerDriver::run_turn` reaches `self.backend.remove(&name).await?` at `llm.rs:1201`. Furthermore, when Stage 0 (orchestrator) uses an `AgentCli` agent, `run_orchestrator_turn` does not open a `StageLease`, so an error in `attach`/`start` or a cancellation during `run_to_completion` leaves the created/running `orrery-cli-<prefix>-<agent_id>` container (with its secret env) in Docker, and causes the next retry or resume with the same `run_id` to fail `create_container` with a 409 name conflict.
- Suggested fix: Remove `&spec.name` on any error after `create_container` inside `BollardContainerBackend::run_to_completion` (and before calling `create_container` if a stale container with that name exists), or ensure `CliContainerDriver::run_turn` removes `&name` on all exit/drop paths.

### F61-2: `BollardContainerBackend::run_to_completion` splits attach chunks with `text.lines()` without buffering across chunk boundaries
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/docker.rs:1189`
- Spec: §5.9
- Evidence: `while let Some(item) = output.next().await { let chunk = item.map_err(Error::Docker)?; let text = String::from_utf8_lossy(chunk.as_ref()); for line in text.lines() { on_output(line.to_string()); } }`
- Impact: Docker attach frames can split a single output line (or a multi-byte UTF-8 character, or a secret token) across multiple `LogOutput` chunks. Because `reader` calls `String::from_utf8_lossy` and `text.lines()` per chunk without a line buffer, a line split across two chunks is emitted as two separate `StreamEventKind::Output` events, and a registered secret or `sk-`/`Bearer ` token split across two attach chunks bypasses `LiveStreamHub::emit` redaction (unlike `HttpChatDriver`, which buffers by line in `LineBuffer` specifically to prevent split-secret leaks).
- Suggested fix: Buffer raw bytes in `BollardContainerBackend::run_to_completion` (or in `CliContainerDriver::run_turn` via `LineBuffer`) and emit only on `\n`, `MAX_LINE_BUFFER_BYTES` at a UTF-8 char boundary, and stream EOF.

### F61-3: `CliContainerDriver::run_turn` ignores `LiveStreamHub::emit` errors on container output lines
- Severity: Low
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/llm.rs:1170`
- Spec: §9.7
- Evidence: `let on_output = |line: String| { let _ = stream.emit(StreamEvent { timestamp: Utc::now(), step_number: Some(inv.step_number), stage_index: Some(inv.stage_index), agent_id: Some(inv.agent.agent_id.clone()), kind: StreamEventKind::Output, text: line }); };`
- Impact: Because `ContainerBackend::run_to_completion` takes `on_output: &(dyn Fn(String) + Send + Sync)` returning `()`, `CliContainerDriver::run_turn` discards `stream.emit(..)` errors with `let _ =`. If the `SecretRedactor` lock is poisoned or the `LiveStreamHub` writer thread has stopped (`Error::Security`), `HttpChatDriver` aborts the turn (`llm.rs:693`), whereas `CliContainerDriver` silently drops the error and continues running.
- Suggested fix: Capture the first `Err` from `stream.emit` in a `Mutex<Option<Error>>` inside `on_output` and check it after `run_to_completion` returns.

### F61-4: `agent_io_dir` and `CliContainerDriver::run_turn` join `agent_id` onto `.orrery/agent_io/<run_id>/` without path-safety validation
- Severity: Medium
- Confidence: High
- Category: path-safety
- Location: `crates/orrery-core/src/docker.rs:622`
- Spec: §9.6
- Evidence: `pub fn agent_io_dir(project_root: &Path, run_id: &uuid::Uuid, agent_id: &str) -> PathBuf { project_root.join(".orrery").join("agent_io").join(run_id.to_string()).join(agent_id) }`
- Impact: As noted in F59-4 and F60-1, `AgentConfig::validate` does not enforce `validate_config_file_id` when loading `.orrery/agents/*.toml` from disk, and `agent_io_dir` is a public helper that directly joins `agent_id`. If `agent_id` contains `..` or `/` (e.g. `../../outside`), `CliContainerDriver::run_turn` (`llm.rs:1100-1123`) creates `in/`, `out/`, and calls `std::fs::remove_dir_all(&mask_dir)` and bind-mounts host directories outside `.orrery/agent_io/<run_id>/`.
- Suggested fix: Enforce `validate_config_file_id` in `AgentConfig::validate` (F60-1) and assert or validate in `CliContainerDriver::run_turn` that `agent_id` is a single normal path component.

### F61-5: `TurnState::finish_response` stops processing remaining tool calls in an assistant batch when `submit_report` succeeds first
- Severity: Low
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/llm.rs:880`
- Spec: §9.7
- Evidence: `for call in calls { if call.name == SUBMIT_REPORT_TOOL { ... match parsed { Ok(report) => return Ok(Some(report)), ... } } ... }`
- Impact: When a model returns multiple parallel tool calls in a single response where `submit_report` appears at a lower index than an MCP tool call (e.g. index 0 is `submit_report` and index 1 is a file-write tool), `finish_response` returns `Ok(Some(report))` immediately upon validating `submit_report` at index 0, silently skipping all subsequent tool calls in the same response batch without executing them or checking `max_tool_calls`.
- Suggested fix: Either execute all non-`submit_report` tool calls in the batch before handling `submit_report`, or reject a batch that mixes `submit_report` with subsequent tool calls as an invalid report so the model is re-prompted.

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §5.7 / §9.6 | `ContainerBackend` trait (`start`, `stop`, `remove`, `inspect`, `run_to_completion`), `BollardContainerBackend::connect` (`connect_with_local_defaults`), `MockContainerBackend`, `create_body` (no GPU/ports, `EnvValue::Secret` exposed only here) | Partial | `crates/orrery-core/src/docker.rs:67`, `crates/orrery-core/src/docker.rs:431`, `crates/orrery-core/src/docker.rs:1037`, `crates/orrery-core/src/docker.rs:1048`, `crates/orrery-core/src/docker.rs:1099` (see F61-1, F61-2) |
| §9.6 | `cli_container_name` (`"orrery-cli-" + first 8 chars of run_id.simple() + "-" + agent_id`) and `agent_io_dir` (`<root>/.orrery/agent_io/<run_id>/<agent_id>`) | Partial | `crates/orrery-core/src/docker.rs:616`, `crates/orrery-core/src/docker.rs:622` (see F61-4) |
| §5.7.1 / §9.6 | AgentCli mounts (`mounts_for`): `LocalModel`/`CloudApi` → `[]`; `AgentCli` → `/workspace` (`ro`/`rw`/omitted for `None`), `/workspace/.orrery` bound `ro` from empty `mask/` when workspace mounted, `/orrery/in` (`ro`), `/orrery/out` (`rw`) | Implemented | `crates/orrery-core/src/docker.rs:672`, `crates/orrery-core/src/llm.rs:1111` |
| §5.7 / §9.6 / §9.7 | AgentCli memory/time limits and cleanup on every exit path (success, error, timeout, cancel): `memory_limit_mb * 1024 * 1024`, `tokio::time::timeout(timeout_secs, ...)`, `backend.remove(&name)` on timeout and after run, `end_stage` removes CLI containers and `agent_io/<run_id>` | Partial | `crates/orrery-core/src/docker.rs:920`, `crates/orrery-core/src/docker.rs:1077`, `crates/orrery-core/src/llm.rs:1180`, `crates/orrery-core/src/llm.rs:1193`, `crates/orrery-core/src/llm.rs:1201` (see F61-1) |
| §5.7.2 / §9.7 / §9.11.12 | `secret_env` injection: fetched from `SecretStore` before container run as `EnvValue::Secret`, exposed only in `create_body`; `AGENT_CLI_GUARDRAIL_NOTE` in TUI Settings and CLI `agent show` | Implemented | `crates/orrery-core/src/llm.rs:1151`, `crates/orrery-core/src/docker.rs:1054` |
| §5.7 / §5.10.3 / §9.6 | `ResourceManager` & `effective_concurrency`: `max_concurrent_agents`, writer overlap/ReadWrite CLI rules, `StageLease` / `SlotPermit`, `LeaseOpenGuard`, inference start/stop, `unload_after_stage`, `startup_timeout_secs`, `External` / `Container` / `Compose` lifecycles (`DockerComposeCli`, `DockerInferenceControl`, `unload_request`), `shutdown` | Implemented | `crates/orrery-core/src/docker.rs:81`, `crates/orrery-core/src/docker.rs:190`, `crates/orrery-core/src/docker.rs:399`, `crates/orrery-core/src/docker.rs:630`, `crates/orrery-core/src/docker.rs:742` (note: engine missing call to `shutdown` was reported in F59-1) |
| §5.9 / §9.7 | Prompt builders (`build_system_prompt`, `build_user_prompt`, `build_orchestrator_prompt`), `AgentInvocation`, `StageSummary`, `AgentDriver`, `MockAgentDriver` | Implemented | `crates/orrery-core/src/llm.rs:30`, `crates/orrery-core/src/llm.rs:52`, `crates/orrery-core/src/llm.rs:104`, `crates/orrery-core/src/llm.rs:138`, `crates/orrery-core/src/llm.rs:187`, `crates/orrery-core/src/llm.rs:197` |
| §5.9 / §9.7 / §9.11.3 / §9.11.5 | `HttpChatDriver`: `endpoint_for`, `build_chat_request` (filters unmappable tool names, appends `submit_report`), `estimate_tokens` context preflight (`bytes.div_ceil(4) > max_context_tokens`), `idle_timeout` on `send()` and each `byte_stream.next()`, `SseParser`, `LineBuffer` (per-line & 4096-byte UTF-8 boundary emission), `ToolCallAccumulator`, `orrery_call_{n}` fallback IDs, tool-call loop & `max_tool_calls`, single reprompt on invalid/missing `submit_report` | Partial | `crates/orrery-core/src/llm.rs:307`, `crates/orrery-core/src/llm.rs:330`, `crates/orrery-core/src/llm.rs:337`, `crates/orrery-core/src/llm.rs:462`, `crates/orrery-core/src/llm.rs:545`, `crates/orrery-core/src/llm.rs:577`, `crates/orrery-core/src/llm.rs:635`, `crates/orrery-core/src/llm.rs:781`, `crates/orrery-core/src/llm.rs:974` (see F61-5) |
| §5.9 / §9.7 | `CliContainerDriver`: deletes stale `report.toml`, writes `prompt.md` via `atomic_write`, runs container with stdin + mounts, streams output, reads and validates `report.toml` (errors name `/orrery/out/report.toml`, never host path), re-prompts once on missing/invalid report | Partial | `crates/orrery-core/src/llm.rs:1052`, `crates/orrery-core/src/llm.rs:1073` (see F61-1, F61-2, F61-3, F61-4) |
| §9.11.3 / §9.12.2 | Bearer header (`auth_header_value`): fetched only for `CloudApi`, formatted in `zeroize::Zeroizing<String>`, validated as HTTP `HeaderValue` (`Error::Validation` on invalid bytes), marked `set_sensitive(true)` | Implemented | `crates/orrery-core/src/llm.rs:611` |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F61-2, F61-5 | Checked all types, signatures, prompt formats, lifecycle orderings, and error variants in `docker.rs` and `llm.rs` against §5.7, §5.9, §9.6, §9.7, §9.11.3, §9.11.5, §9.11.12, and §9.12.2. |
| C2 | findings: F61-3 | Checked all `let _ =` sites in non-test code (`llm.rs:773`, `1170`, `1193`, `1223`; none in `docker.rs`). `llm.rs:773` discards `bool` after `?;`; `1193` is best-effort container removal on timeout before returning `Error::Agent`; `1223` is best-effort `remove_file` on invalid report before retry; `1170` ignores `stream.emit(..)` `Result` in `CliContainerDriver`'s `on_output` closure. |
| C3 | none | Checked all `.ok()`, `unwrap_or*`, `if let Ok`, and `Err(_)` sites in non-test code (`docker.rs:253`, `545`, `1032`, `1078`, `1202`; `llm.rs:63`, `333`, `355`). `docker.rs:253` polls `/health` until 200 or deadline; `1202` ignores `BrokenPipe`/`ConnectionReset` on container stdin per §9.6; `llm.rs:355` filters unmappable tool names per §9.11.5. |
| C4 | none | Checked all `match` expressions in non-test code of `docker.rs` and `llm.rs`. Every `match` on crate enums (`EnvValue`, `MountSpec`, `ContainerState`, `ComposeOp`, `InferenceLifecycle`, `InferenceMode`, `AgentBackend`, `SseItem`) is exhaustive without `_ =>` catch-all arms. |
| C5 | none | Zero occurrences of `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, or `#[allow(..)]` in non-test code of `docker.rs` (lines 1–1231) and `llm.rs` (lines 1–1240). |
| C6 | none | Zero numeric `as` casts in non-test code of `docker.rs` and `llm.rs`. `create_body` (`docker.rs:1077`) uses `i64::try_from(spec.memory_limit_mb).unwrap_or(i64::MAX).saturating_mul(1024 * 1024)`; `estimate_tokens` (`llm.rs:332`) uses `div_ceil(4)` with `usize::MAX` fallback; `LineBuffer` uses `floor_char_boundary`. |
| C7 | findings: F61-1 | Checked lock scopes, semaphore permits, `tokio::join!`, and `tokio::time::timeout`. No `std::sync::Mutex` guard is held across `.await` in `ResourceManager`, `MockContainerBackend`, `MockInferenceControl`, or `MockAgentDriver`. `LeaseOpenGuard` resets `lease_open` if `begin_stage` fails or is cancelled mid-flight, and `StageLease::drop` resets `open`. Container cleanup on `attach`/`start`/cancel without a stage lease is reported in F61-1. |
| C8 | none | `CliContainerDriver::run_turn` writes `prompt.md` via `config::atomic_write` (`llm.rs:1149`) and deletes any pre-existing `report.toml` before each container run (`llm.rs:1139`). |
| C9 | findings: F61-4 | `DockerComposeCli` rejects relative compose paths with `ensure_absolute_compose_file` (`docker.rs:130`). `agent_io_dir` (`docker.rs:622`) and `CliContainerDriver::run_turn` (`llm.rs:1100`) join `agent_id` onto `.orrery/agent_io/<run_id>/` without checking that `agent_id` is a single safe path segment. |
| C10 | findings: F61-1, F61-2 | `EnvValue::Secret` is exposed only in `create_body` (`docker.rs:1054`); `DockerComposeCli` redacts stderr via `SecretRedactor` (`docker.rs:155`); `auth_header_value` wraps `"Bearer {key}"` in `zeroize::Zeroizing` and marks `HeaderValue` sensitive (`llm.rs:622-628`); `CliContainerDriver` missing-report errors use `CLI_REPORT_PATH` instead of the host path (`llm.rs:1216`). Unbuffered chunk line-splitting in `BollardContainerBackend::run_to_completion` (F61-2) and unremoved failed/cancelled CLI containers (F61-1) are reported above. |
| C11 | none | Reviewed all unit tests in `docker.rs:1232-2490` (25 tests), `llm.rs:1241-3527` (36 tests), and live tests in `tests/live_docker.rs:1-195` (3 tests) and `tests/live_llm.rs:1-192` (2 tests). No `#[ignore]` or assertion-free tests. |
| C12 | none | Zero `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, `#[allow(dead_code)]`, or commented-out code blocks in `docker.rs` or `llm.rs`. |
