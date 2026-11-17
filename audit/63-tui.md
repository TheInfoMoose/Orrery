# Audit: TUI (Step 63)

## Scope
- Files read: `crates/orrery-tui/src/app.rs:1-1157` (non-test), `crates/orrery-tui/src/app.rs:1158-2312` (tests, C11 only); `crates/orrery-tui/src/bridge.rs:1-710` (non-test), `crates/orrery-tui/src/bridge.rs:711-1748` (tests, C11 only); `crates/orrery-tui/src/layout.rs:1-282` (non-test), `crates/orrery-tui/src/layout.rs:283-481` (tests, C11 only); `crates/orrery-tui/src/widgets.rs:1-886` (non-test), `crates/orrery-tui/src/widgets.rs:887-1454` (tests, C11 only); `crates/orrery-tui/src/theme.rs:1-84` (non-test), `crates/orrery-tui/src/theme.rs:85-175` (tests, C11 only); `crates/orrery-tui/src/lib.rs:1-7`; `README.md:73-97`
- Spec ranges: `ORRERY_TECHNICAL_SPECIFICATION.md:493-580` (§6), `1045-1120` (§9.9), `1261-1288` (§9.11.9–§9.11.12), `1321-1323` (§9.12.6), `1332-1341` (§9.13.1)

## Findings
### F63-1: `EngineController::handle(AppAction::Interrupt)` leaves `app.status` stuck at `AwaitingOperator` with `"Stopping…"` when no task is running
- Severity: High
- Confidence: High
- Category: bug
- Location: `crates/orrery-tui/src/bridge.rs:372`
- Spec: §9.11.9
- Evidence: `AppAction::Interrupt => { self.cancel.cancel(); app.status_message = Some("Stopping…".into()); }`
- Impact: When the TUI starts on a project whose checkpoint is `AwaitingOperator`, `load_initial_app` (`bridge.rs:589`) sets `app.status = RunStatus::AwaitingOperator`, `app.awaiting_operator = true`, and `app.status_message = cp.awaiting_reason`, while `controller.task` is `None`. Because `app.is_running()` (`app.rs:195`) and `render_footer_menu` (`widgets.rs:502`) treat `RunStatus::AwaitingOperator` as running, the footer renders `[Stop]` instead of `[Run]`. Pressing `Enter`/`Space` on `[Stop]`, clicking `[Stop]`, or pressing `Ctrl+C` emits `AppAction::Interrupt`, which overwrites `app.status_message` with `"Stopping…"` (erasing `awaiting_reason`), leaves `app.status = RunStatus::AwaitingOperator`, and waits for a task completion (`RunFinished` / `poll_finished`) that never occurs because `self.task` is `None`.
- Suggested fix: In `EngineController::handle(AppAction::Interrupt)`, if `!self.has_running_task()`, transition `app.status` to `RunStatus::Idle`, set `app.awaiting_operator = false`, and clear `app.status_message` instead of setting `"Stopping…"`.

### F63-2: Selecting an agent on an occupied slot in `App::apply_selector_choice` appends to the stage instead of replacing the selected slot
- Severity: High
- Confidence: High
- Category: bug
- Location: `crates/orrery-tui/src/app.rs:631`
- Spec: §6.2
- Evidence: `} else { match self.config.assign_agent(stage, option.clone()) { Ok(()) => { self.refresh_warnings(); vec![AppAction::AssignAgent { stage, agent_id: option }] }`
- Impact: When the operator opens the Agent `Selector` on an already-occupied slot (`slot < stage.agent_ids.len()`) and selects a replacement agent (anything other than `"(remove)"`), `apply_selector_choice` ignores `slot` and calls `self.config.assign_agent(stage, option)`, which pushes `option` onto the end of `stage.agent_ids`. On an orchestrator stage (`MAX_ORCHESTRATOR_AGENTS = 1`) or a stage with 15 assigned agents, choosing a replacement agent on an occupied slot always fails `validate_staging()` with an error; on a partially filled stage, it appends to the next slot and leaves the selected slot's agent unchanged.
- Suggested fix: When `slot < stage.agent_ids.len()` and `option != REMOVE_OPTION`, replace the agent at `stage.agent_ids[slot]` (or remove the existing agent at `slot` before assigning `option`) and persist the updated configuration.

### F63-3: `run_tui` never calls `EngineController::drain_channels` and `start_run` silently drops lagged `EngineEvent`s
- Severity: Medium
- Confidence: High
- Category: concurrency
- Location: `crates/orrery-tui/src/bridge.rs:299`
- Spec: §9.11.9
- Evidence: `res = eng_sub.recv(), if events_open => { match res { Ok(ev) => { let _ = eng_tx.send(ev).await; } ... Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} } }`
- Impact: `EngineController::drain_channels` (`bridge.rs:136`) has no non-test callers: `run_tui` (`bridge.rs:651-685`) pops only one event from `controller.next_event()` per iteration and immediately executes a full `terminal.draw`, violating §9.11.9 ("the render loop drains them every iteration"). In addition, `start_run`'s `biased` `select!` (`bridge.rs:261`) polls `stream_sub.recv()` before `eng_sub.recv()`, and on broadcast lag (`RecvError::Lagged(_)` at `bridge.rs:299` and `TryRecvError::Lagged(_)` at `bridge.rs:485`), `EngineEvent`s (`StepStatusChanged`, `PhaseChanged`, `AwaitingOperator`, `RunFinished`) are silently discarded, leaving `app.step_statuses`, `app.run_passed`, and `app.status` out of sync with the engine.
- Suggested fix: Call `controller.drain_channels(&mut app, &regions)` in `run_tui` before drawing each frame, place `eng_sub.recv()` before `stream_sub.recv()` in the `biased` `select!`, and reload checkpoint state or emit a notice if `eng_sub` lags.

### F63-4: `EngineController::handle(Approve | Reject)` while a run is active leaves `app.awaiting_operator` true, allowing duplicate decisions to queue
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-tui/src/bridge.rs:384`
- Spec: §9.11.9
- Evidence: `if self.has_running_task() { if let Some(tx) = &self.operator_tx && let Err(mpsc::error::TrySendError::Full(_)) = tx.try_send(decision) { app.status_message = Some("operator decision dropped: channel full".into()); } }`
- Impact: When a run task is active (`self.has_running_task()`), sending `OperatorDecision` via `tx.try_send(decision)` does not set `app.awaiting_operator = false` or `app.status = RunStatus::Running` (unlike the no-task branch which calls `start_run` at `bridge.rs:405`). While the engine processes the decision (for example, re-running a halted `on_step_pass` hook per §9.11.7 before emitting `PhaseChanged` or `StepStatusChanged`), `[Approve]` and `[Reject]` remain visible and active; pressing `Enter` or clicking again queues extra `OperatorDecision`s in `operator_tx` / `eng_op_tx` (capacity 8) that will auto-approve or auto-reject a subsequent operator pause without operator input.
- Suggested fix: When `tx.try_send(decision)` succeeds in `EngineController::handle`, set `app.awaiting_operator = false`, `app.status = RunStatus::Running`, and `app.status_message = None`.

### F63-5: `load_initial_app` and `EngineController::handle(IngestPlan)` do not reconcile checkpoints and silently discard load errors
- Severity: Medium
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-tui/src/bridge.rs:580`
- Spec: §9.9
- Evidence: `if let Ok(plan_path) = store.resolve_safe_path(&app.config.plan_path) && let Ok(plan) = ImplementationPlan::from_file(&plan_path, &app.config.step_marker) { ... match checkpoints.load_state() { Ok(Some(cp)) if cp.step_statuses.len() == plan.steps.len() => {`
- Impact: `load_initial_app` (`bridge.rs:580-599`) silently discards `Err` from `resolve_safe_path`, `ImplementationPlan::from_file`, and `checkpoints.load_state()` without setting `app.status_message`, and checks only `cp.step_statuses.len() == plan.steps.len()` instead of calling `cp.reconcile(&plan)`. If a step was added to `PLAN.md`, all prior `Passed` or `AwaitingOperator` statuses are hidden as `Pending`; if a different plan with the same step count was written, stale statuses from the old plan are shown. Conversely, `handle(IngestPlan)` (`bridge.rs:360-364`) resets `app.step_statuses` to `Pending` in memory without reconciling disk checkpoints or clearing a stale `app.status = RunStatus::AwaitingOperator` / `app.awaiting_operator = true` (and allows re-ingesting while a run is active).
- Suggested fix: Surface plan/checkpoint load errors in `app.status_message`, reconcile checkpoints with `cp.reconcile(&plan)` in both `load_initial_app` and `handle(IngestPlan)`, and update `app.status` / `app.awaiting_operator` from the reconciled checkpoint.

### F63-6: `App::handle_footer_menu_key` steps `footer_index` onto hidden `Approve` and `Reject` buttons when `!app.approve_visible()`
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-tui/src/app.rs:908`
- Spec: §6.2
- Evidence: `KeyCode::Left => { self.footer_index = self.footer_index.saturating_sub(1); Vec::new() } KeyCode::Right => { if self.footer_index < 6 { self.footer_index += 1; } Vec::new() }`
- Impact: When `app.approve_visible()` is `false`, `render_footer_menu` (`widgets.rs:514-525`) skips `footer_items` indices `1` (`ApproveButton`) and `2` (`RejectButton`) with `continue`. However, `handle_footer_menu_key` still increments and decrements `self.footer_index` through `1` and `2`, causing the selection highlight in the `Actions` footer to vanish completely for two key presses between `[Run]`/`[Stop]` (index 0) and `[Settings]` (index 3).
- Suggested fix: Skip indices `1` and `2` during `Left`/`Right` navigation in `handle_footer_menu_key` when `!self.approve_visible()`, and move `self.footer_index` off `1..=2` whenever `awaiting_operator` becomes `false`.

### F63-7: `App::keep_selection_visible` and `handle_stage_canvas_key` never reset `slot_scroll` when `slot_selected == 0`
- Severity: Low
- Confidence: High
- Category: bug
- Location: `crates/orrery-tui/src/app.rs:977`
- Spec: §6.1
- Evidence: `if self.slot_selected > 0 { let slot_idx = self.slot_selected - 1; let visible_slots = regions.stage_columns[0].slots.len(); if visible_slots > 0 { if slot_idx < self.slot_scroll { self.slot_scroll = slot_idx; } ... } }`
- Impact: After scrolling down agent slots on a multi-agent stage so `self.slot_scroll > 0`, pressing `Up` to select the stage header (`slot_selected = 0`, `app.rs:793`), clicking a stage header (`app.rs:1053`), or switching to a stage with `unlocked_slots == 0` leaves `self.slot_scroll > 0` because `keep_selection_visible` only adjusts `slot_scroll` when `self.slot_selected > 0`. Because `slot_scroll` applies to all visible stage columns (`layout.rs:167`), slot 0 (including Stage 0's sole orchestrator slot) remains scrolled off the top of the canvas with only `[locked]` rows visible.
- Suggested fix: Reset `self.slot_scroll = 0` when `self.slot_selected == 0` in `handle_stage_canvas_key(KeyCode::Up)` and `keep_selection_visible`.

### F63-8: Early I/O error returns in `run_tui` bypass `controller.shutdown(&mut app).await`, detaching a running engine task
- Severity: Medium
- Confidence: High
- Category: concurrency
- Location: `crates/orrery-tui/src/bridge.rs:640`
- Spec: §9.11.9
- Evidence: `terminal.draw(|frame| render(frame, &app, &regions)).map_err(|source| Error::Io { path: PathBuf::from("<terminal>"), source })?;`
- Impact: Inside `run_tui`'s loop, `terminal.draw(...)` (`bridge.rs:640`), `event_stream.next()` (`bridge.rs:662`), and `guard.set_mouse_capture(*enable)` (`bridge.rs:689`) return `Err` early via `?` / `return Err(...)`, skipping `controller.shutdown(&mut app).await` at `bridge.rs:706`. If a terminal I/O error occurs while a workflow is running, `EngineController` is dropped without cancelling `self.cancel` or awaiting `self.task`, detaching the engine task so `on_halt` and checkpoint persistence are skipped on process exit.
- Suggested fix: Capture any loop error into an `Option<Error>`, `break` out of the event loop, and always execute `controller.shutdown(&mut app).await` before returning the error.

### F63-9: `EngineController` sends unredacted task errors to `stream_tx` and `app.status_message`, and skips the stream error notice on build failures
- Severity: Low
- Confidence: High
- Category: secrets
- Location: `crates/orrery-tui/src/bridge.rs:318`
- Spec: §9.11.9
- Evidence: `if let Err(ref err) = res { let _ = stream_tx.try_send(StreamEvent { kind: StreamEventKind::Error, text: err.to_string(), ..StreamEvent::notice("") }); }`
- Impact: When `engine.run` returns `Err(err)`, `bridge.rs:318` sends `err.to_string()` directly over `stream_tx` (bypassing `LiveStreamHub::emit` and its `SecretRedactor`), and `apply_task_result` (`bridge.rs:154`) sets `app.status_message = Some(err.to_string())` unredacted. In addition, `(make_backends)()?` (`bridge.rs:240`) and `build_engine(&store, backends).await?` (`bridge.rs:241`) return via `?` before line 318, so a backend or engine build failure omits the §9.11.9 live-stream error notice and waits until the next 250 ms tick for `poll_finished`. Moreover, `app.status_message` is never cleared (`= None`) anywhere in non-test code once set.
- Suggested fix: Route task errors through `LiveStreamHub::emit` when available (or redact before `stream_tx.try_send` and `status_message`), catch `make_backends` / `build_engine` errors before returning so the stream error notice is emitted, and clear `app.status_message` when starting a run or completing a valid action.

### F63-10: `EngineController::handle` contains `_ => unreachable!()` in non-test code
- Severity: Low
- Confidence: High
- Category: leftover
- Location: `crates/orrery-tui/src/bridge.rs:382`
- Spec: none
- Evidence: `let decision = match action { AppAction::Approve => OperatorDecision::Approve, AppAction::Reject(feedback) => OperatorDecision::Reject { feedback: feedback.clone() }, _ => unreachable!(), };`
- Impact: Inside the `AppAction::Approve | AppAction::Reject(_)` arm of `EngineController::handle`, `action` is matched a second time with a catch-all `_ => unreachable!()` arm in non-test code (flagged by checks C4 and C5).
- Suggested fix: Construct `decision` in the outer match or split the pattern without a nested `_ => unreachable!()` arm.

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §6.1 / §9.9 | `UiLayoutRegions::compute` (80×24 min, `too_small`, 11/3 header at height 40, `top_menu`, `main_workspace` 25%/Fill(1), `live_stream`, `footer_menu`, stage columns min width 18, `stage_scroll`, `slot_scroll`) and `region_at` | Implemented | `crates/orrery-tui/src/layout.rs:38`, `crates/orrery-tui/src/layout.rs:193`, `crates/orrery-tui/src/layout.rs:245` |
| §6.2 / §6.3 | Theme palette (`BG_BLACK`, `PANEL_BG`, `AMBER_*`), `BANNER`, `COMPACT_HEADER`, `banner_line` 4-rule coloring, block/style helpers, header widget and status badge | Implemented | `crates/orrery-tui/src/theme.rs:5`, `crates/orrery-tui/src/theme.rs:13`, `crates/orrery-tui/src/theme.rs:27`, `crates/orrery-tui/src/widgets.rs:101` |
| §6.2 / §9.11.11 | Top menu widget (`[Ingest]`, `[Mode: RUN\|LOOP]`, `[Cycles: N]`, `[Passed: X/Y]` with `run_passed/cycle_count` in Loop mode when `cycle_count > 0`, `[Stages: - N +]`) | Implemented | `crates/orrery-tui/src/widgets.rs:151` |
| §6.2 / §9.9 | Steps panel (`[ ]`, `[>]`, `[✓]`, `[?]`, `[!]` badges, scrolling via `step_scroll`), Stage canvas (`unlocked_slots`, `LM`/`CLI`/`API` tags, `[locked]` in `AMBER_DARK`), Live stream (ring cap 2,000, scroll/follow, alert styling) | Partial | `crates/orrery-tui/src/app.rs:202`, `crates/orrery-tui/src/widgets.rs:287`, `crates/orrery-tui/src/widgets.rs:346`, `crates/orrery-tui/src/widgets.rs:466` (see F63-2, F63-7) |
| §6.2 / §9.11.10 | Footer menu (`[Run]`/`[Stop]`, `[Approve]`/`[Reject]` visible whenever `awaiting_operator`, `[Settings]`, `[Help]`, `[Mouse: ON\|OFF]`, `[Quit]`) | Partial | `crates/orrery-tui/src/app.rs:224`, `crates/orrery-tui/src/widgets.rs:497` (see F63-6) |
| §9.9 / §9.11.9 / §9.11.10 / §9.12.6 | `AppAction::Run` — handled at `bridge.rs:366`; when no run is active, calls `start_run` (`bridge.rs:224`) which resets `run_passed = 0`, sets `status = Running`, and spawns the engine task | Implemented | `crates/orrery-tui/src/app.rs:102`, `crates/orrery-tui/src/bridge.rs:224`, `crates/orrery-tui/src/bridge.rs:366` |
| §9.9 / §9.11.9 | `AppAction::Interrupt` — handled at `bridge.rs:372`; cancels `self.cancel` and sets `status_message = Some("Stopping…")` even when no run is active | Partial | `crates/orrery-tui/src/app.rs:103`, `crates/orrery-tui/src/bridge.rs:372` (see F63-1) |
| §9.9 / §9.11.10 / §9.12.6 | `AppAction::Approve` and `AppAction::Reject(String)` — handled at `bridge.rs:376`; when no run is active, loads checkpoint, verifies `AwaitingOperator`, saves `operator_decision`, and calls `start_run` (or sets `"no step awaiting operator"`) | Partial | `crates/orrery-tui/src/app.rs:104`, `crates/orrery-tui/src/bridge.rs:376`, `crates/orrery-tui/src/bridge.rs:414` (see F63-4, F63-10) |
| §9.9 / §9.11.11 | `AppAction::IngestPlan(PathBuf)` — handled at `bridge.rs:339`; whether or not a run is active, resolves path, parses plan (restoring `step_marker` on error), saves `project.toml`, updates `app.plan` and `app.step_statuses` | Partial | `crates/orrery-tui/src/app.rs:106`, `crates/orrery-tui/src/bridge.rs:339` (see F63-5) |
| §9.9 | `AppAction::SetMode`, `SetCycles`, `SetStageCount`, `SetStageRole`, `AssignAgent`, `UnassignAgent` — handled at `bridge.rs:331-338`; when no run is active (and during a run), saves `app.config` via `store.save_project` | Implemented | `crates/orrery-tui/src/app.rs:107`, `crates/orrery-tui/src/bridge.rs:331` |
| §9.9 | `AppAction::SetMouseCapture(bool)` and `AppAction::Quit` — no-op in `EngineController::handle` (`bridge.rs:408`); handled in `run_tui` (`guard.set_mouse_capture` at `bridge.rs:688`, loop break + `shutdown` at `bridge.rs:647`, `701`, `706`) when no run is active or running | Implemented | `crates/orrery-tui/src/app.rs:113`, `crates/orrery-tui/src/bridge.rs:408`, `crates/orrery-tui/src/bridge.rs:688`, `crates/orrery-tui/src/bridge.rs:701` |
| §6.1 / §6.4 / §9.9 | `UiRegion` keyboard paths (top menu): `IngestButton` (`Focus::TopMenu` index 0, `Enter`/`Space`), `ModeToggle` (index 1, `Enter`/`Space`), `CyclesControl` (index 2, `Enter`/`Space` or `+`/`-`), `PassedCounter` (index 3, read-only), `StagesMinus` (index 4, `Enter`/`Space`/`-`), `StagesPlus` (index 5, `Enter`/`Space`/`+`) | Implemented | `crates/orrery-tui/src/layout.rs:10`, `crates/orrery-tui/src/app.rs:682` |
| §6.1 / §6.4 / §9.9 | `UiRegion` keyboard paths (workspace & stream): `StepsList` (`Focus::StepsPanel`, `Up`/`Down`), `StageHeader(stage)` (`Focus::StageCanvas`, `Left`/`Right` to `stage`, `Up` to `slot_selected = 0`, `Enter`/`Space`), `AgentSlot { stage, slot }` (`Focus::StageCanvas`, `Left`/`Right`, `Up`/`Down` to `slot + 1`, `Enter`/`Space`/`Delete`), `LiveStream` (`Focus::LiveStream`, `Up`/`Down`/`PageUp`/`PageDown`/`End`) | Implemented | `crates/orrery-tui/src/layout.rs:16`, `crates/orrery-tui/src/app.rs:749`, `crates/orrery-tui/src/app.rs:771`, `crates/orrery-tui/src/app.rs:879` |
| §6.1 / §6.4 / §9.9 | `UiRegion` keyboard paths (footer menu): `RunStopButton` (`Focus::FooterMenu` index 0 `Enter`/`Space`, or `Ctrl+C` when running), `ApproveButton` (index 1 `Enter`/`Space`), `RejectButton` (index 2 `Enter`/`Space`), `SettingsButton` (index 3 `Enter`/`Space`), `HelpButton` (index 4 `Enter`/`Space` or `?`), `MouseToggle` (index 5 `Enter`/`Space` or `m`), `QuitButton` (index 6 `Enter`/`Space` or `q`) | Implemented | `crates/orrery-tui/src/layout.rs:20`, `crates/orrery-tui/src/app.rs:538`, `crates/orrery-tui/src/app.rs:906` |
| §6.4 / §9.13.1 | README TUI keys table (`README.md:77-96`): all 16 table rows and the `Ingest` `Tab`/`BackTab` note match `app.rs` (`handle_key_event`, `handle_top_menu_key`, `handle_steps_panel_key`, `handle_stage_canvas_key`, `handle_live_stream_key`, `handle_footer_menu_key`, `handle_mouse_event`) | Implemented | `README.md:77`, `crates/orrery-tui/src/app.rs:332`, `crates/orrery-tui/src/app.rs:990` |
| §6.2 / §9.9 / §9.11.11 / §9.11.12 | Modals (`Ingest`, `CyclesEntry`, `Settings`, `Help`, `QuitConfirm`, `RejectFeedback`, `Selector`): open paths, close paths (`Enter`/`y`/`n`/`q`), and `Esc` always closes every modal (`app.rs:399`, `430`, `452`, `472`, `499`, `509`, `526`) | Implemented | `crates/orrery-tui/src/app.rs:80`, `crates/orrery-tui/src/app.rs:356`, `crates/orrery-tui/src/widgets.rs:559` |
| §6.5 / §9.11.9 | Render path non-blocking: `render` (`widgets.rs:20`) and `step` (`bridge.rs:50`) are synchronous; `EngineController::handle` (`bridge.rs:329`) spawns the engine task without awaiting `make_backends`, `build_engine`, or `engine.run`; `poll_finished` (`bridge.rs:166`) only awaits a handle when `is_finished()` is true; `shutdown` (`bridge.rs:183`) runs after loop exit with a 30s timeout | Partial | `crates/orrery-tui/src/bridge.rs:166`, `crates/orrery-tui/src/bridge.rs:183`, `crates/orrery-tui/src/bridge.rs:224`, `crates/orrery-tui/src/bridge.rs:635` (see F63-3, F63-8, F63-9) |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F63-1, F63-2, F63-3, F63-4, F63-5, F63-6, F63-7 | Checked all types, constants, signatures, key/mouse bindings, modals, layout rules, and bridge behaviors in `app.rs`, `bridge.rs`, `layout.rs`, `widgets.rs`, and `theme.rs` against §6, §9.9, §9.11.9–§9.11.12, §9.12.6, §9.13.1, and `README.md:73-97`. |
| C2 | none | Checked all `let _ =` sites in non-test code (`bridge.rs:138`, `141`, `211`, `280`, `294`, `318`, `422`, `471`, `479`, `483`, `511`, `540`, `541`). Channel sends in the engine task / shutdown and best-effort terminal restoration on drop/error are intentional per §9.11.9 and §9.12.6. |
| C3 | findings: F63-3, F63-5 | Checked all `.ok()`, `unwrap_or*`, `if let Ok`, and discarded `Err` arms in non-test code. `bridge.rs:299` and `485` ignore `RecvError::Lagged(_)` on `eng_sub` (F63-3); `bridge.rs:580-596` `load_initial_app` silently ignores plan and checkpoint load errors (F63-5). `app.rs:423` `parse().unwrap_or(0)` is safe because `CyclesEntry` accepts only ≤3 ASCII digits. |
| C4 | findings: F63-10 | Checked all `match` arms on crate enums in non-test code. `Focus`, `RunStatus`, `SelectorKind`, `Modal`, and `BridgeInput` matches are exhaustive; `bridge.rs:382` has a nested `_ => unreachable!()` on `AppAction` (F63-10). `widgets.rs:201`, `537` and `app.rs:1149` use `_ =>` to filter subsets of `UiRegion`. |
| C5 | findings: F63-10 | Checked all five files (non-test ranges) for `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, and `#[allow(..)]`. Found `unreachable!()` at `bridge.rs:382` (F63-10); zero occurrences in `app.rs:1-1157`, `layout.rs:1-282`, `widgets.rs:1-886`, and `theme.rs:1-84`. |
| C6 | none | Checked all numeric conversions and arithmetic in non-test code. No `as` casts exist in `crates/orrery-tui/src/`; conversions use `u16::try_from(...).unwrap_or(...)` or `usize::from(...)`, and scroll/index arithmetic uses `saturating_add`, `saturating_sub`, `min`, `max`, and `clamp`. |
| C7 | findings: F63-3, F63-8 | No lock is held across `.await`. Checked `tokio::select!` loops in `bridge.rs:130`, `190`, `261`, `651`, channel capacities, and task lifecycles. Uncalled `drain_channels` / lagged `eng_sub` (F63-3) and early error returns in `run_tui` that skip `controller.shutdown` and detach `self.task` (F63-8) are reported above. |
| C8 | none | `orrery-tui` performs file writes only by delegating to `ConfigStore::save_project` (`bridge.rs:337`, `359`) and `CheckpointManager::save_state` (`bridge.rs:404`), which use write-to-temp-and-rename in `orrery-core`. |
| C9 | none | Plan ingestion in `bridge.rs:340` and initial plan loading in `bridge.rs:580` pass `path` through `ConfigStore::resolve_safe_path` before reading. |
| C10 | findings: F63-9 | `widgets.rs:743-789` Settings modal displays only non-secret agent fields (`inference_id`, `endpoint_url`, `model`, `max_context_tokens`, `image`, `workspace_mount`, `memory_limit_mb`, `timeout_secs`), omitting keychain fields and `secret_env`. Unredacted task error strings sent via `stream_tx.try_send` and `app.status_message` are reported in F63-9. |
| C11 | none | Reviewed all unit tests in `app.rs:1158-2312` (26 tests), `bridge.rs:711-1748` (14 tests), `layout.rs:283-481` (10 tests), `widgets.rs:887-1454` (13 tests), and `theme.rs:85-175` (7 tests). Zero `#[ignore]` or assertion-free tests. |
| C12 | none | Zero `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, `#[allow(dead_code)]`, or commented-out code blocks in any of the five files. |
