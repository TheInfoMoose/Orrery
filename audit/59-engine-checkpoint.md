# Audit: Engine and Checkpoints (Step 59)

## Scope
- Files read: `crates/orrery-core/src/engine.rs:1-2062` (non-test), `crates/orrery-core/src/engine.rs:2063-5972` (tests, C11 only); `crates/orrery-core/src/checkpoint.rs:1-543` (non-test), `crates/orrery-core/src/checkpoint.rs:544-1266` (tests, C11 only)
- Spec ranges: `ORRERY_TECHNICAL_SPECIFICATION.md:26-45` (§2), `356-381` (§5.5), `441-492` (§5.10), `718-778` (§9.3), `1006-1044` (§9.8), `1160-1192` (§9.11.2), `1225-1243` (§9.11.6, §9.11.7), `1309-1311` (§9.12.3), plus applicable Phase D–F amendment headings in §9.11–§9.13

## Findings
### F59-1: `WorkflowEngine::run` never calls `ResourceManager::shutdown()` on exit
- Severity: High
- Confidence: High
- Category: wiring
- Location: `crates/orrery-core/src/engine.rs:405`
- Spec: §2
- Evidence: `state.phase = EnginePhase::Done; state.active_stage_index = None; self.emit_event(EngineEvent::PhaseChanged(EnginePhase::Done)); self.emit_event(EngineEvent::RunFinished(outcome.clone())); return Ok(outcome);`
- Impact: When a workflow run completes (`RunOutcome::Completed`, `RunOutcome::NoPendingSteps`), pauses (`RunOutcome::AwaitingOperator`), is cancelled (`RunOutcome::Interrupted` at lines 1016/1032), or errors out, `self.ctx.resources.shutdown().await` is never called. Router models loaded during the final stage remain loaded in VRAM and single-model inference containers/compose services that Orrery started remain running after the run ends.
- Suggested fix: Call `self.ctx.resources.shutdown().await` before returning from `WorkflowEngine::run` (or via an async cleanup helper on every return path of `run`, including `handle_cancel`).

### F59-2: Resuming from `AwaitingOperator` does not reconstruct `stage_ran_since_gate_failure`
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/engine.rs:421`
- Spec: §5.10
- Evidence: `let mut resumed = StepRunState { step_idx, step_number: step.number, notes: Vec::new(), history, last_stage_run, consecutive_violations: 0, stage_ran_since_gate_failure: true };`
- Impact: If a gate fails on `StepPassed`, the orchestrator immediately escalates (or halts) without dispatching a stage turn, and the operator later resumes with `Approve`, `resumed.stage_ran_since_gate_failure` is hardcoded to `true` (and at line 500 on `InProgress` resume). If the orchestrator reports `StepPassed` again without dispatching any stage turn and the gate fails again, the engine treats it as the first gate failure instead of recording a protocol violation per §5.10.1 ("A `StepPassed` whose gate fails with no stage turn since the previous failed gate is a protocol violation").
- Suggested fix: Derive `stage_ran_since_gate_failure` when reconstructing step state, or persist it in `WorkflowCheckpoint` so resume preserves whether a stage ran since the last gate failure.

### F59-3: `build_engine` silently swallows keychain errors when registering secrets with `SecretRedactor`
- Severity: Medium
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/engine.rs:1964`
- Spec: §2
- Evidence: `if let Some((service, account)) = pair && let Ok(secret) = backends.secrets.get(service, account) { redactor.register(&secret)?; }`
- Impact: Both the agent loop (line 1964) and the MCP server loop (line 1975) use `if let Ok(secret) = backends.secrets.get(service, account)`, ignoring `Err` (such as a locked keychain, D-Bus error, or missing secret for a configured agent/server). If a secret cannot be read at engine build time but becomes readable or is echoed from another source during the run, `SecretRedactor` will not have it registered.
- Suggested fix: For referenced agents and servers, propagate `backends.secrets.get` errors or emit a `SystemNotice` warning when a configured keychain entry cannot be loaded for redaction.

### F59-4: `CheckpointManager::save_report` and `delete_reports` do not validate `agent_id` in report filename
- Severity: Low
- Confidence: High
- Category: path-safety
- Location: `crates/orrery-core/src/checkpoint.rs:447`
- Spec: §9.3
- Evidence: `let file_name = format!("round_{:02}_stage_{:02}_{}.toml", report.round, report.stage_index, report.agent_id); let path = step_dir.join(file_name);`
- Impact: `CheckpointManager::save_report` is a public API that interpolates `report.agent_id` directly into `file_name` without calling `validate_safe_relative_path` or rejecting path separators. Although `AgentConfig::validate` checks `agent_id` against embedded secrets and emptiness, `AgentConfig::validate` does not forbid `/` or `..` in `agent_id` unless saved through `ConfigStore::save_agent`; a caller constructing `StampedReport` directly with `agent_id = "../evil"` would write outside `step_dir`.
- Suggested fix: Validate that `report.agent_id` contains no path separators or `..` (e.g. via `validate_safe_relative_path`) in `CheckpointManager::save_report`.

### F59-5: `WorkflowCheckpoint::reset_from` does not delete stale `StampedReport` files for reset steps
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/checkpoint.rs:396`
- Spec: §9.3
- Evidence: `let start_index = step_number - 1; for status in &mut self.step_statuses[start_index..] { *status = StepStatus::Pending; } self.current_step_index = start_index;`
- Impact: When the operator runs `orrery reset --step N`, `WorkflowCheckpoint::reset_from(N)` marks steps `>= N` as `Pending` and resets `round = 0`, `retry_count = 0`, and `phase = Idle`, but `CheckpointManager` never removes `.orrery/checkpoints/step_{:03}/` for steps `>= N`. On the next run of step `N`, `run_orchestrator_turn` (line 1637) and `run_stage_turn` (line 1721) call `checkpoints.load_reports(step.number, None)?`, feeding all stale `StampedReport`s from the pre-reset run into `# Prior reports` of the fresh run's prompts, and `reconstruct_step_history` (line 1454) includes them if the new run pauses or interrupts.
- Suggested fix: Delete step report directories for steps `>= step_number` on reset, or clear existing step reports when a `Pending` step transitions to `InProgress` at `engine.rs:511`.

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §2 / §5.10.1 | Engine state machine phases (`Idle`, `OrchestratorTurn`, `StageTurn`, `Gate`, `AwaitingOperator`, `Hook`, `Done`) and transitions | Implemented | `crates/orrery-core/src/checkpoint.rs:231`, `crates/orrery-core/src/engine.rs:346` |
| §5.10.1 / §9.8 | Dispatch rules (`check_dispatch`: forward to `next` or retry to earlier non-orchestrator, non-validator stage) | Implemented | `crates/orrery-core/src/engine.rs:118` |
| §5.10.1 / §9.8 | Retries, `retry_count` increment on every `DispatchKind::Retry` and `PassCheck` reject, `retry_count > max_step_retries` → `AwaitingOperator` | Implemented | `crates/orrery-core/src/engine.rs:677`, `crates/orrery-core/src/engine.rs:1133` |
| §5.10.1 / §9.8 | Protocol violation retried once with note; second consecutive violation → `AwaitingOperator`; repeated failed gate without stage turn is violation | Partial | `crates/orrery-core/src/engine.rs:861`, `crates/orrery-core/src/engine.rs:1039` (see F59-2) |
| §5.10.1 / §9.8 | Escalation (`ReportStatus::Escalate`) transitions to `AwaitingOperator` with `feedback` reason | Implemented | `crates/orrery-core/src/engine.rs:778` |
| §4.1 / §5.10.2 / §9.8 | Three validation modes (`Auto`, `Agents`, `GateAndOperator`), `agents_pass_rule`, and gate execution (`__orrery_gate`) | Implemented | `crates/orrery-core/src/engine.rs:168`, `crates/orrery-core/src/engine.rs:803`, `crates/orrery-core/src/engine.rs:1347` |
| §9.8 / §9.11.7 | Operator decisions (`Approve` / `Reject` for `PassCheck`, `HaltOrEscalation`, and `StepPassHook` via channel or checkpoint) | Implemented | `crates/orrery-core/src/engine.rs:423`, `crates/orrery-core/src/engine.rs:1068`, `crates/orrery-core/src/engine.rs:1116`, `crates/orrery-core/src/engine.rs:1175` |
| §5.10.4 / §9.8 / §9.11.6 | Lifecycle hooks (`on_step_start`, `on_step_pass`, `on_step_retry`, `on_halt` — note: config/spec uses `on_step_start` and `on_step_retry`), `on_failure` (`Halt` / `Warn`), redaction, 4 KiB truncation, 200-entry cap | Implemented | `crates/orrery-core/src/engine.rs:1248`, `crates/orrery-core/src/checkpoint.rs:247`, `crates/orrery-core/src/checkpoint.rs:409` |
| §5.10.3 / §9.8 | Stage concurrency via `ResourceManager::begin_stage` / `SlotPermit` and validator broadcast | Implemented | `crates/orrery-core/src/engine.rs:1695`, `crates/orrery-core/src/engine.rs:1748` |
| §5.10.5 / §9.8 | `Run` and `Loop` modes, `cycle_count` (`0` = all pending, `> 0` = stop at `cycles`), `steps_passed_this_run` reset rule | Implemented | `crates/orrery-core/src/engine.rs:381`, `crates/orrery-core/src/engine.rs:1215` |
| §5.10.6 / §9.8 | Interrupt and cancel: race with `CancellationToken`, close stage lease, preserve `AwaitingOperator` without re-running `on_halt`, else `InProgress` + `on_halt` + `Interrupted` | Implemented | `crates/orrery-core/src/engine.rs:308`, `crates/orrery-core/src/engine.rs:993` |
| §5.10.6 / §9.8 | Resume: `InProgress` deletes current round reports and restarts round from orchestrator; `AwaitingOperator` applies or awaits decision; `reconcile` on start | Partial | `crates/orrery-core/src/engine.rs:352`, `crates/orrery-core/src/engine.rs:411`, `crates/orrery-core/src/engine.rs:484` (see F59-2, F59-5) |
| §9.8 / §9.11.6 / §9.12.3 | `Backends::real`, `referenced_mcp_servers`, `build_engine` (secrets redaction, stream attach, referenced MCP connect, unused notice, hidden tool notice) | Partial | `crates/orrery-core/src/engine.rs:1862`, `crates/orrery-core/src/engine.rs:1901`, `crates/orrery-core/src/engine.rs:1933` (see F59-1, F59-3) |
| §5.5 / §9.3 | `AgentReport::validate_for_role`, `report_json_schema`, `WorkflowCheckpoint` (`new`, `reconcile`, `reset_from`, `push_hook_result`) | Partial | `crates/orrery-core/src/checkpoint.rs:50`, `crates/orrery-core/src/checkpoint.rs:156`, `crates/orrery-core/src/checkpoint.rs:292` (see F59-5) |
| §4 / §5.5 / §9.3 | `CheckpointManager` (`save_state`, `load_state` via `atomic_write`, `save_report` to `step_{:03}/round_{:02}_stage_{:02}_{agent_id}.toml`, `load_reports`, `delete_reports`) | Implemented | `crates/orrery-core/src/checkpoint.rs:422` (see F59-4) |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F59-1, F59-2, F59-5 | Checked all types, signatures, ordering, messages, and state transitions in `engine.rs` and `checkpoint.rs` against §2, §5.5, §5.10, §9.3, §9.8, §9.11.2, §9.11.6, §9.11.7, and §9.12.3. |
| C2 | none | Checked all `let _ =` sites in non-test code (`engine.rs:502`, `530`, `1036`, `1244`; none in `checkpoint.rs`). `502`/`530` discard `AwaitResolution` after `?;` where only side-effects matter; `1036` ignores `broadcast::Sender::send` with no receivers; `1244` ignores `HookOutcome` in `run_halt_hook` after `?;`. |
| C3 | findings: F59-3 | Checked `.ok()`, `unwrap_or*`, `if let Ok`, and `Err(_)` in non-test code. `engine.rs:1964` and `1975` use `if let Ok(secret) = backends.secrets.get(..)` which silently skips registering secrets if the keychain call fails. Other `unwrap_or*` uses (`engine.rs:347-348`, `442`, `782`, `899`, `1635`, `1686`, `1699`) have valid fallbacks. |
| C4 | none | Checked all `match` expressions in non-test code. The only open catch-all on a crate enum is `other =>` on `orch_report.status` (`engine.rs:969`), which intentionally catches non-orchestrator `ReportStatus` variants as protocol violations. |
| C5 | none | Zero occurrences of `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, or `#[allow(..)]` in non-test code of `engine.rs` (lines 1–2062) and `checkpoint.rs` (lines 1–543). |
| C6 | none | Checked all arithmetic and index operations in non-test code. No `as` casts in either file; `saturating_sub` is used at `checkpoint.rs:354`, `362`, and `engine.rs:489`; `truncate_utf8` checks `len() <= max_bytes` and `end > 0`. |
| C7 | none | Checked lock scopes, `tokio::select!` (`race_cancel!`), channels, and stage concurrency. `current_lease` lock is never held across `.await`; `operator_rx` lock is held only while awaiting `rx.recv().await` in single-consumer `enter_awaiting_operator`; `join_all` in `run_stage_turn` is dropped before `handle_cancel` unwraps `current_lease`. |
| C8 | none | `CheckpointManager::save_state` (`checkpoint.rs:430`) and `save_report` (`checkpoint.rs:453`) both delegate to `config::atomic_write`, which writes a unique temp file in the same directory, flushes, calls `sync_all()`, renames, and syncs the parent directory. |
| C9 | findings: F59-4 | `build_engine` resolves `project.plan_path` via `store.resolve_safe_path` (`engine.rs:1945`), and `AgentReport::validate_for_role` checks every `write_scope` entry with `validate_safe_relative_path` (`checkpoint.rs:119`). `CheckpointManager::save_report` (`checkpoint.rs:447`) formats `report.agent_id` directly into a filename without validating path safety. |
| C10 | none | `build_engine` registers agent and HTTP MCP secrets with `SecretRedactor` before creating `LiveStreamHub`; `run_hook` redacts hook output via `self.ctx.stream.redactor().redact(&raw_output)?` (`engine.rs:1309`) before storing in `WorkflowCheckpoint.hook_results`; all `LiveStreamHub::emit` calls redact automatically. |
| C11 | none | Reviewed all tests in `engine.rs:2063-5972` (40 tests) and `checkpoint.rs:544-1266` (18 tests). No `#[ignore]` or assertion-free tests; unit tests cover all spec'd methods, edge cases, and Phase D/E amendments. |
| C12 | none | Zero `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, `#[allow(dead_code)]`, or commented-out code blocks in `engine.rs` or `checkpoint.rs`. |
