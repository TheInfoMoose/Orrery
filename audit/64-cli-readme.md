# Audit: CLI and README (Step 64)

## Scope
- Files read: `crates/orrery-cli/src/args.rs:1-443` (non-test), `crates/orrery-cli/src/args.rs:444-899` (tests, C11 only); `crates/orrery-cli/src/commands.rs:1-953` (non-test), `crates/orrery-cli/src/commands.rs:954-3331` (tests, C11 only); `crates/orrery-cli/src/main.rs:1-73` (non-test); `crates/orrery-cli/tests/cli_smoke.rs:1-77` (C11 only); `README.md:1-162`; `crates/orrery-core/src/config.rs:1-2293` (README field names and defaults); `crates/orrery-core/tests/live_keyring.rs:1-30`, `live_mcp.rs:1-41`, `live_docker.rs:1-195`, `live_llm.rs:1-192`, `live_engine.rs:1-103` (README env var names)
- Spec ranges: `ORRERY_TECHNICAL_SPECIFICATION.md:581-644` (§7, §8), `1121-1134` (§9.10), `1244-1260` (§9.11.8), `1289-1296` (§9.11.13, §9.11.14), `1300-1328` (§9.12), `1332-1341` (§9.13.1)

## Findings
### F64-1: `dispatch_status` does not call `WorkflowCheckpoint::reconcile`, reporting stale or misaligned statuses after plan edits
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-cli/src/commands.rs:850`
- Spec: §7
- Evidence: `let step_statuses: Vec<StepStatus> = (0..plan.steps.len()).map(|i| { cp.step_statuses.get(i).copied().unwrap_or(StepStatus::Pending) }).collect();`
- Impact: When a checkpoint exists on disk (`Some(cp)`), `dispatch_status` (`commands.rs:845-866`) maps `cp.step_statuses` onto `plan.steps` by raw positional index `i` without calling `cp.reconcile(&plan)` or checking `cp.plan_sha256` / `cp.step_hashes`. If the operator edits a `Passed` or `AwaitingOperator` step, inserts or deletes steps in the plan, or ingests a different plan file with `orrery ingest`, `orrery status` (and `orrery status --json`) reports the old checkpoint's statuses, `phase`, and `current_step` mapped onto the new plan steps instead of surfacing the `reconcile` hash-mismatch error (`step edited; run orrery reset --step <N>`) or reconciling appended/removed steps.
- Suggested fix: Call `cp.reconcile(&plan)?` on the loaded `WorkflowCheckpoint` in `dispatch_status` before computing `current_step` and `step_statuses`.

### F64-2: `AgentCmd::AddCli::command` (`--command <ARG>...`) rejects any container command argument starting with `-`
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-cli/src/args.rs:203`
- Spec: §7
- Evidence: `#[arg(long, required = true, num_args = 1..)] command: Vec<String>,`
- Impact: Unlike `McpCmd::AddStdio::args` (`#[arg(long = "arg", allow_hyphen_values = true)]`, `args.rs:355`), `AgentCmd::AddCli::command` uses `num_args = 1..` without `allow_hyphen_values = true`. When an operator registers an `AgentCli` agent whose command includes flags—such as the spec §4.3 example `["claude", "-p", "--output-format", "stream-json"]` or `sh -c "..."`—clap stops parsing `--command` at the first hyphenated token (`-p` or `-c`), treats it as an unknown flag on `orrery agent add-cli`, and exits with code 2 (`unexpected argument '-p' found`).
- Suggested fix: Enable hyphenated command tokens (for example, repeatable `--command <ARG>` with `allow_hyphen_values = true`, or `allow_hyphen_values = true` on `--command` when parsed per-argument) and add a parsing test in `args.rs` with hyphenated container arguments.

### F64-3: `dispatch_run` subscribes to the live stream after `build_engine` returns and silently ignores lagged broadcast events
- Severity: Medium
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-cli/src/commands.rs:735`
- Spec: §9.10
- Evidence: `let engine = build_engine(&store, backends).await?; let mut stream_rx = engine.subscribe_stream();`
- Impact: `build_engine` emits `StreamEvent::notice` messages during initialization—including the §9.11.6 unused-MCP-server notices (`crates/orrery-core/src/engine.rs:2004`) and the §9.12.3 hidden-MCP-tool notices (`crates/orrery-core/src/engine.rs:2028`)—before returning `WorkflowEngine`. Because `dispatch_run` calls `engine.subscribe_stream()` after `build_engine` finishes and never reads buffered events from `LiveStreamHub::recent`, those startup notices are never printed to stdout by `orrery run` or `orrery resume`. In addition, `commands.rs:756` (`Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}`) silently discards lagged stream events without printing a drop notice.
- Suggested fix: Flush any events already buffered in `LiveStreamHub::recent` to stdout before entering the `select!` loop (or subscribe before `build_engine` emits notices), and print a notice line when `stream_rx.recv()` returns `RecvError::Lagged(n)`.

### F64-4: `dispatch_secret` probes a new `OsKeyringStore` directly with `if let Ok(..)` instead of querying `ctx.secrets`
- Severity: Low
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-cli/src/commands.rs:705`
- Spec: §9.10
- Evidence: `if let Ok(os_store) = OsKeyringStore::new() && os_store.backend() == KeyringBackend::Keyutils {`
- Impact: After storing a secret via `ctx.secrets.set(&service, &account, &secret)?`, `dispatch_secret` constructs a second `OsKeyringStore::new()` directly and discards errors with `if let Ok(..)` (check C3). This bypasses the injected `ctx.secrets: Arc<dyn SecretStore>` in `CliContext`, probing the host's D-Bus and kernel `keyutils` keyrings even during unit tests that use `InMemorySecretStore` and printing the kernel `keyutils` reboot note whenever the host lacks D-Bus regardless of which store `ctx.secrets` holds.
- Suggested fix: Add an optional method on `SecretStore` (e.g., `fn keyring_backend(&self) -> Option<KeyringBackend> { None }`) implemented by `OsKeyringStore`, and check `ctx.secrets.keyring_backend() == Some(KeyringBackend::Keyutils)`.

### F64-5: `read_secret_from_stdin` and `read_secret_input` leave unzeroized secret copies on the heap, and `main` prints unredacted errors
- Severity: Low
- Confidence: High
- Category: secrets
- Location: `crates/orrery-cli/src/main.rs:25`
- Spec: §5.3
- Evidence: `fn read_secret_from_stdin() -> orrery_core::Result<String> { let mut line = String::new(); std::io::stdin().read_line(&mut line).map_err(|source| Error::Io { path: PathBuf::from("<stdin>"), source })?; Ok(line.trim_end_matches(['\r', '\n']).to_string()) }`
- Impact: Both `read_secret_from_stdin` (`main.rs:25`) and `read_secret_input` (`main.rs:34`) call `.trim_end_matches(['\r', '\n']).to_string()`, which allocates a second `String` and drops the original `line` / `value` `String` without zeroizing the plaintext secret in heap memory. In addition, when `dispatch` returns `Err(err)`, `main.rs:69` prints `render_error(&err)` directly to `ctx.err` without redacting token patterns via `SecretRedactor`.
- Suggested fix: Truncate `line` / `value` in place (`let len = line.trim_end_matches(['\r', '\n']).len(); line.truncate(len);`) inside a `zeroize::Zeroizing<String>` before wrapping in `SecretValue`, and pass rendered CLI errors through `SecretRedactor`.

### F64-6: `README.md` global catalog layout omits `~/.config/orrery/inference.toml`
- Severity: Low
- Confidence: High
- Category: doc-drift
- Location: `README.md:101`
- Spec: §7
- Evidence: `~/.config/orrery/                 # Global catalog (optional). Project files override by id. ├── roles/<role_id>.toml + .md ├── agents/<agent_id>.toml └── mcp/servers.toml`
- Impact: `ConfigStore::global_dir` (`crates/orrery-core/src/config.rs:1634`) and `ConfigStore::load_inference` (`crates/orrery-core/src/config.rs:1950`) load `~/.config/orrery/inference.toml` from the global catalog (and spec §7 line 601 states that "absolute paths go in the machine-scope `inference.toml` by hand"), and `ConfigStore::load_inference` / `save_inference` (`crates/orrery-core/src/config.rs:1965`, `2018`) also read/write `<project_root>/.orrery/inference.toml`, but the `## Project layout` section in `README.md:100-124` omits `inference.toml`.
- Suggested fix: Add `inference.toml` to the `~/.config/orrery/` tree and bullet list in `README.md`.

### F64-7: `dispatch_logs` `follow: true`, `dispatch_run` `--step` / exit 130, and `dispatch_reset` without a checkpoint lack unit tests in `commands.rs`
- Severity: Low
- Confidence: High
- Category: test-gap
- Location: `crates/orrery-cli/src/commands.rs:933`
- Spec: §9.11.8
- Evidence: `if follow { let mut follower = LogFollower::new(&log_dir)?; let ctrl_c = tokio::signal::ctrl_c(); tokio::pin!(ctrl_c);`
- Impact: In `commands.rs` unit tests, `logs_tail_shows_redacted_lines` (`commands.rs:3149`) only exercises `follow: false`, leaving the `if follow` polling loop (`commands.rs:933-949`) untested at the CLI command layer (where `_ = &mut ctrl_c => return Ok(130)` at `commands.rs:939` also treats a `ctrl_c()` `Err` as exit code 130). Similarly, `Command::Run` with `step: Some(n)` (`commands.rs:130`), `dispatch_run` mapping `RunOutcome::Interrupted => Ok(130)` (`commands.rs:776`), and `dispatch_reset` when no checkpoint file exists on disk (`commands.rs:814-819`) have no unit tests in `commands.rs`.
- Suggested fix: Add unit tests in `commands.rs` covering `Command::Run` with `step: Some(2)`, `RunOutcome::Interrupted` returning `Ok(130)`, `Command::Reset` when no checkpoint exists on disk, and `dispatch_logs` with `follow: true`.

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §7 / §9.10 | Global `--project <PATH>` (default `"."`), `Cli::validate` (`path` + subcommand conflict → `ErrorKind::ArgumentConflict`, exit 2), `Cli::into_command` | Implemented | `crates/orrery-cli/src/args.rs:10`, `crates/orrery-cli/src/args.rs:19`, `crates/orrery-cli/src/args.rs:29` |
| §7 row 1 | `orrery [PATH]` — `Command::Tui { path }` launches TUI via `orrery_tui::run_tui` | Implemented | `crates/orrery-cli/src/args.rs:38`, `crates/orrery-cli/src/commands.rs:63` |
| §7 row 2 | `orrery init [PATH] [--name <NAME>]` — `Command::Init { path, name }` creates `.orrery/` with 3-stage template, default roles, empty dirs | Implemented | `crates/orrery-cli/src/args.rs:42`, `crates/orrery-cli/src/commands.rs:72` |
| §7 row 3 | `orrery ingest <MD_FILE> [--marker <STR>]` — `Command::Ingest { md_file, marker }` resolves path, parses plan, saves `plan_path`/`step_marker`, prints step titles | Implemented | `crates/orrery-cli/src/args.rs:47`, `crates/orrery-cli/src/commands.rs:96` |
| §7 row 4 / §9.10 | `orrery workflow show [--json]` — `WorkflowCmd::Show { json }` prints stages, roles, agents, effective validation mode, warnings (`{"validation_mode", "stages": [{"index", "role_id", "agent_ids"}], "warnings"}`) | Implemented | `crates/orrery-cli/src/args.rs:131`, `crates/orrery-cli/src/commands.rs:155` |
| §7 row 5 / §9.10 | `orrery workflow stages <COUNT>` — `WorkflowCmd::Stages { count }` with `value_parser!(u8).range(2..=15)` | Implemented | `crates/orrery-cli/src/args.rs:135`, `crates/orrery-cli/src/commands.rs:205` |
| §7 row 6 | `orrery workflow set-role <STAGE> <ROLE_ID>` — `WorkflowCmd::SetRole { stage, role_id }` | Implemented | `crates/orrery-cli/src/args.rs:139`, `crates/orrery-cli/src/commands.rs:210` |
| §7 row 7 | `orrery workflow assign <STAGE> <AGENT_ID>` — `WorkflowCmd::Assign { stage, agent_id }` | Implemented | `crates/orrery-cli/src/args.rs:143`, `crates/orrery-cli/src/commands.rs:215` |
| §7 row 8 | `orrery workflow unassign <STAGE> <AGENT_ID>` — `WorkflowCmd::Unassign { stage, agent_id }` | Implemented | `crates/orrery-cli/src/args.rs:147`, `crates/orrery-cli/src/commands.rs:222` |
| §7 row 9 | `orrery workflow clear <STAGE>` — `WorkflowCmd::Clear { stage }` | Implemented | `crates/orrery-cli/src/args.rs:151`, `crates/orrery-cli/src/commands.rs:231` |
| §7 row 10 | `orrery workflow mode <run\|loop> [--cycles <N>]` — `WorkflowCmd::Mode { mode, cycles }` | Implemented | `crates/orrery-cli/src/args.rs:154`, `crates/orrery-cli/src/commands.rs:236` |
| §7 row 11 | `orrery agent add-local-model --id --name --inference <ID> --model <M> [--temperature] [--context]` — `AgentCmd::AddLocalModel` (defaults: `temperature = 0.1`, `context = 32768`) | Implemented | `crates/orrery-cli/src/args.rs:182`, `crates/orrery-cli/src/commands.rs:318` |
| §7 row 12 / §5.7.2 | `orrery agent add-cli --id --name --image <IMG@DIGEST> --command <ARG>... [--mount none\|ro\|rw] [--memory-mb] [--timeout] [--secret-env <VAR> --keychain-service --keychain-account]` — `AgentCmd::AddCli`, prints both §5.7.2 residual-risk notes | Partial | `crates/orrery-cli/src/args.rs:196`, `crates/orrery-cli/src/commands.rs:340` (see F64-2) |
| §7 row 13 | `orrery agent add-cloud --id --name --endpoint <URL> --model <M> --keychain-service --keychain-account` — `AgentCmd::AddCloud` | Implemented | `crates/orrery-cli/src/args.rs:218`, `crates/orrery-cli/src/commands.rs:378` |
| §7 row 14 / §9.10 / §9.11.8 | `orrery agent list [--json]` / `show <ID>` / `remove <ID>` — `AgentCmd::{List, Show, Remove}`; `show` prints `AGENT_CLI_GUARDRAIL_NOTE` for `AgentCli` | Implemented | `crates/orrery-cli/src/args.rs:232`, `crates/orrery-cli/src/commands.rs:402`, `crates/orrery-cli/src/commands.rs:420`, `crates/orrery-cli/src/commands.rs:437` |
| §7 row 15 / §9.10 | `orrery inference add --id --endpoint <URL> [--mode single\|router] [--container <NAME> \| --compose <FILE> [--service <S>]] [--no-unload] [--startup-timeout <SECS>]` / `list [--json]` / `remove <ID>` — `InferenceCmd::{Add, List, Remove}` | Implemented | `crates/orrery-cli/src/args.rs:261`, `crates/orrery-cli/src/commands.rs:445` |
| §7 rows 16–17 / §9.10 | `orrery secret set --service <S> --account <A>` and `orrery secret delete --service <S> --account <A>` — `SecretCmd::{Set, Delete}` with TTY no-echo prompt (`rpassword::prompt_password("Secret: ")`) or stdin line | Partial | `crates/orrery-cli/src/args.rs:290`, `crates/orrery-cli/src/commands.rs:689`, `crates/orrery-cli/src/main.rs:17` (see F64-4, F64-5) |
| §7 row 18 / §9.10 | `orrery role add --id --name --kind <orchestrator\|implementer\|validator> --rules-file <MD> [--allow-tool <SERVER:TOOL>]...` / `list [--json]` / `show <ID>` / `remove <ID>` — `RoleCmd::{Add, List, Show, Remove}` | Implemented | `crates/orrery-cli/src/args.rs:323`, `crates/orrery-cli/src/commands.rs:505` |
| §7 rows 19–21 / §9.10 | `orrery mcp add-stdio --id --command <CMD> [--arg <A>]... [--write-tool <TOOL>:<ARG>[,<ARG>]]...`, `add-http --id --endpoint <URL> [--keychain-service --keychain-account] [--write-tool ...]`, `list [--json]`, `tools <ID>`, `remove <ID>` — `McpCmd::{AddStdio, AddHttp, List, Tools, Remove}`, connects and lists tools before saving | Implemented | `crates/orrery-cli/src/args.rs:349`, `crates/orrery-cli/src/commands.rs:596`, `crates/orrery-cli/src/commands.rs:612` |
| §7 row 22 | `orrery gate set <SERVER:TOOL>...` / `clear` — `GateCmd::{Set, Clear}` | Implemented | `crates/orrery-cli/src/args.rs:385`, `crates/orrery-cli/src/commands.rs:251` |
| §7 row 23 | `orrery hook set <start\|pass\|retry\|halt> <SERVER:TOOL> [--on-failure halt\|warn]` / `clear <EVENT>` — `HookCmd::{Set, Clear}` | Implemented | `crates/orrery-cli/src/args.rs:429`, `crates/orrery-cli/src/commands.rs:273` |
| §7 row 24 / §9.10 | `orrery run [--loop] [--cycles <N>] [--step <N>]` — `Command::Run` streams `[{kind:?}] {agent_id or -}: {text}` to stdout, cancels on `tokio::signal::ctrl_c()` | Partial | `crates/orrery-cli/src/args.rs:84`, `crates/orrery-cli/src/commands.rs:118`, `crates/orrery-cli/src/commands.rs:726`, `crates/orrery-cli/src/commands.rs:731` (see F64-3) |
| §7 rows 25–27 / §9.10 | `orrery approve`, `orrery reject --feedback <TEXT>`, `orrery resume`, `orrery reset --step <N>` — `Command::{Approve, Reject, Resume, Reset}` | Implemented | `crates/orrery-cli/src/args.rs:92`, `crates/orrery-cli/src/commands.rs:135`, `crates/orrery-cli/src/commands.rs:780`, `crates/orrery-cli/src/commands.rs:809` |
| §7 row 28 / §9.10 / §9.11.8 | `orrery status [--json]` — `Command::Status { json }`, checks `resolved.is_file()` with `orrery ingest <MD_FILE>` hint, outputs `{"run_id", "phase", "current_step", "round", "retry_count", "awaiting_reason", "steps": [{"number", "title", "status"}]}` | Partial | `crates/orrery-cli/src/args.rs:102`, `crates/orrery-cli/src/commands.rs:827` (see F64-1) |
| §7 row 29 / §9.11.8 / §9.12.4 | `orrery logs [--tail <N>] [--follow]` — `Command::Logs { tail, follow }`, prints `tail_log`, then `LogFollower` polled every 500 ms against pinned `ctrl_c` | Implemented | `crates/orrery-cli/src/args.rs:106`, `crates/orrery-cli/src/commands.rs:920` |
| §7 / §9.10 / §9.11.8 | Exit codes `0`, `1`, `2`, `3`, `130` and `render_error`: `0` on command success and `RunOutcome::{Completed, NoPendingSteps}` (`commands.rs:774`); `1` on runtime/validation error via `exit_code_for` (`commands.rs:50`, `main.rs:54`, `70`); `2` on clap parse error (`main.rs:42`) or `cli.validate()` conflict via `e.exit()` (`args.rs:21`, `main.rs:44`); `3` when `run`/`resume` returns `RunOutcome::AwaitingOperator` (`commands.rs:775`, tested at `commands.rs:2858`); `130` on `Error::Interrupted` (`commands.rs:49`), `RunOutcome::Interrupted` (`commands.rs:776`), or Ctrl+C in `logs --follow` (`commands.rs:940`) | Implemented | `crates/orrery-cli/src/args.rs:21`, `crates/orrery-cli/src/commands.rs:37`, `crates/orrery-cli/src/commands.rs:47`, `crates/orrery-cli/src/commands.rs:773`, `crates/orrery-cli/src/commands.rs:940`, `crates/orrery-cli/src/main.rs:42` |
| §9.11.14 / §9.12.7 / §9.13.1 | README commands, flags, env vars, paths, and defaults: build/test commands (`README.md:19-29`, `Cargo.toml:1-9`), Quickstart CLI commands and flags (`README.md:37-70`, `args.rs:42-287`, `config.rs:237-268`), TUI keys table (`README.md:77-96`, `crates/orrery-tui/src/app.rs:332-990`), `.orrery/` project layout and defaults (`README.md:100-124`, `config.rs:14-275`), security notes and embedded secret patterns (`README.md:127-134`, `config.rs:18-20`, `security.rs:61-198`), live-backend test commands and env vars `ORRERY_LIVE_MCP_CMD`, `ORRERY_LIVE_IMAGE`, `ORRERY_LIVE_CONTAINER`, `ORRERY_LIVE_LLM_URL`, `ORRERY_LIVE_LLM_MODEL`, `ORRERY_LIVE_CLOUD_URL`, `ORRERY_LIVE_CLOUD_MODEL`, `orrery.test`/`orrery.test.cloud` (`README.md:135-162`, `crates/orrery-core/tests/live_*.rs`) | Partial | `README.md:19`, `README.md:37`, `README.md:77`, `README.md:100`, `README.md:127`, `README.md:135` (see F64-6) |
| §9.12.1 / §9.12.7 | README platform, Docker, and keychain statements: Linux `x86_64-unknown-linux-gnu` tested, `aarch64-unknown-linux-gnu` checked by `cargo deny` only and untested, macOS/Windows not supported yet (planned) (`README.md:7`, `deny.toml:4`); Rust 1.99.1 / edition 2024 (`README.md:8`, `Cargo.toml:7-8`); `libdbus-1-dev` and `pkg-config` (`README.md:9, 19`); D-Bus Secret Service with headless Linux kernel `keyutils` fallback that does not survive reboot (`README.md:10, 127`, `security.rs:61-74`, `commands.rs:710`); Docker Engine tested, `DOCKER_HOST` honored, rootless Docker and Podman untested (`README.md:11`, `docker.rs:1076`) | Implemented | `README.md:7`, `README.md:8`, `README.md:9`, `README.md:10`, `README.md:11`, `README.md:127` |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F64-1, F64-2, F64-3, F64-6 | Checked all §7 command rows, flags, value parsers, exit codes, JSON shapes (§9.10), error rendering and log following (§9.11.8), and `README.md` against `args.rs`, `commands.rs`, `main.rs`, `config.rs`, and `tests/live_*.rs`. |
| C2 | none | Checked all `let _ =` sites in non-test code: `commands.rs:461` propagates the `Result` with `?` and discards only the canonical `PathBuf`; `main.rs:69` is a best-effort `writeln!` to `stderr` immediately before exiting with a non-zero status code. |
| C3 | findings: F64-3, F64-4 | Checked all `.ok()`, `unwrap_or*`, `if let Ok`, and discarded `Err` arms in non-test code. `commands.rs:705` uses `if let Ok(os_store) = OsKeyringStore::new()` (F64-4), and `commands.rs:756` ignores `RecvError::Lagged(_)` on `stream_rx` (F64-3). Other `unwrap_or*` uses (`args.rs:30-31`, `commands.rs:73`, `82`, `99`, `727`, `855`, `928`) apply documented defaults. |
| C4 | none | Checked all `match` statements in `args.rs:1-443`, `commands.rs:1-953`, and `main.rs:1-73`. Every match on `orrery-cli` enums and `RunOutcome` is exhaustive; `commands.rs:50` `_ => 1` on `orrery_core::Error` implements §9.10 (`exit_code_for`: 130 for `Interrupted`, else 1). |
| C5 | none | Zero occurrences of `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, or `#[allow(..)]` in `args.rs:1-443`, `commands.rs:1-953`, or `main.rs:1-73`. |
| C6 | none | Zero numeric `as` casts in `args.rs`, `commands.rs`, or `main.rs` (`args.rs:21` is a `<Cli as clap::CommandFactory>` trait qualification, and `commands.rs:206` uses `usize::from(count)`). Step number display arithmetic (`current_step_index + 1` at `commands.rs:803`, `849`) is bounded by validated plan step counts. |
| C7 | findings: F64-3 | No lock is held across `.await` in `orrery-cli`. Checked `tokio::select!` loops in `commands.rs:749` (`dispatch_run`) and `commands.rs:938` (`dispatch_logs`). Late stream subscription and ignored `RecvError::Lagged` in `dispatch_run` are reported in F64-3. |
| C8 | none | `orrery-cli` performs all persistent file writes through `ConfigStore::init_project`, `save_project`, `save_agent`, `save_inference`, `save_role`, `save_mcp_server`, and `CheckpointManager::save_state`, which use atomic write-to-temp-and-rename in `orrery-core`. |
| C9 | none | User-supplied file paths inside the project (`Command::Ingest` `md_file` at `commands.rs:100`, `RoleCmd::Add` `rules_file` at `commands.rs:515`, `InferenceCmd::Add` `compose` at `commands.rs:462`, and `cfg.plan_path` in `dispatch_reset` / `dispatch_status` at `commands.rs:816`, `830`) are validated via `ConfigStore::resolve_safe_path` or `resolve_compose_file`. |
| C10 | findings: F64-5 | `SecretCmd::Set` (`commands.rs:692-704`) wraps secrets in `SecretValue` and never prints them to stdout/stderr, and `logs` / `run` emit redacted `StreamEvent`s. Unzeroized temporary `String` buffers in `read_secret_from_stdin` / `read_secret_input` and unredacted error printing in `main.rs` are reported in F64-5. |
| C11 | findings: F64-7 | Reviewed all unit and smoke tests in `args.rs:444-899` (6 tests), `commands.rs:954-3331` (29 tests), and `tests/cli_smoke.rs:1-77` (1 test). Zero `#[ignore]` or assertion-free tests; untested branches in `commands.rs` (`follow: true`, `Run { step: Some(..) }`, `RunOutcome::Interrupted`, and `Reset` without an existing checkpoint) are reported in F64-7. |
| C12 | none | Zero `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, `#[allow(dead_code)]`, or commented-out code in `args.rs`, `commands.rs`, or `main.rs`. |
