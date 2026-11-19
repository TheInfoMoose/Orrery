# Orrery — Stepped Implementation Plan

This plan builds Orrery one Argon step at a time: `orrery-core` (Steps 1–26), `orrery-tui` (27–32) and `orrery-cli` (33–37), then the Phase D follow-up (38–51), Phase E (52–57), Phase F (58), the Phase G audit (59–64), Phase H fixes (65–76), Phase I tidy-ups (77–79), Phase J secret-handling fixes (80–83), Phase K (84–87), Phase L configured tool arguments (88–92), Phase M role protocol and skills (93–99) and Phase N config safety and global profiles (100–110). `ORRERY_TECHNICAL_SPECIFICATION.md` (the spec) is the design. Each step block below says which spec lines it implements, which files it may touch, which tests it must add, and the exact shell lines that gate it.

Restructured 2026-11-13 to the Nebulous plan format (roadmap tables, one-line gates, `ALLOWED_FILES` / `READ_ONLY_REFS` / `FORBIDDEN_SCOPE`, a fixed `HALT_INVARIANT`). Step numbers, scopes and test names are unchanged from the 2026-11-12 plan. The old text is kept in `STEPPED_IMPLEMENTATION_PLAN.pre-2026-11-13.md`.

---

## Step Execution Roadmap

**How to read this:** work goes strictly top to bottom through **Up Next**, one step at a time. A step moves to **Completed** only after its `POST_GATE` exits 0. Step numbers are permanent: spec §3.1 and §3.2 name steps by number, and Step 6's acceptance test counts the `### STEP ` headings in this file (110). Never add, remove or renumber a step heading without updating that test and the spec in the same change.

**Who runs what:**
- **Gemini** runs one step from its Argon block **in this file**, found by its heading (`### STEP <N>:`). It never works from a pasted copy. If a pasted prompt and this file disagree, this file wins and Gemini says so in its report instead of halting. Gemini never edits this file or the spec.
- **Claude** writes and refreshes the blocks, checks each finished step's diff against its `MUTATION_SPEC`, and moves the row to **Completed**. Around each handoff Claude runs `scripts/validate-step.sh start <N>` (runs the step's `PRE_GATE`, then snapshots the tree) and, after Gemini reports, `scripts/validate-step.sh check <N>` (fails on a commit, an edit to this file, any change outside `ALLOWED_FILES`, or a failing `POST_GATE`). Claude commits each step after `check` passes. Gemini never commits.
- **The operator** runs `sudo`, installs host packages and tools, and runs the live-backend tests.

**Tracking (2026-11-18):** side findings become **pending neb items** (`neb status <ID> pending`) in this repo's `.tracker/`, not entries in a Parked list. The operator reviews `neb list --status pending` once at phase end. Every Up Next step has a neb task (`source_ref` = step ID); commits start with `[<TASK-ID>](<scope>)`.

**Orrery (2026-11-18):** steps can also run through Orrery (`.orrery/`, Agents mode, `orrery run --step <position>`): the `hybrid:step_start` hook does `neb start` + `validate-step.sh start`, `agy-impl` executes the Argon block, `claude-val` reads the diff, and the `hybrid:step_check` gate runs `validate-step.sh check` on the host. Failures go back to the implementer automatically, up to 2 retries. Claude still reads the diff, commits and moves the row. See the `orrery-setup` skill.

### Up Next

| # | Step | Command | Scope | Target Files | Waits on |
| :---: | :--- | :---: | :--- | :--- | :--- |
| 1 | **[USER] CI-1** | — | First CI run on GitHub | — | **[USER]** runs the steps under "User Step: CI-1". |
| 2 | **[USER] LIVE-3** | — | Fresh `init` passes on local 27B; skill reaches prompt | scratch repo | **[USER]** runs the steps under "User Step: LIVE-3". |
| 3 | **STEP 101** | `/goal` | TUI saves merge into the file on disk (BUG-0015) | `crates/orrery-tui/src/bridge.rs` | STEP 100 |
| 4 | **STEP 102** | `/goal` | Gate-failure rule in the Agents protocol (BUG-0014) | `crates/orrery-core/src/llm.rs` | STEP 101 |
| 5 | **STEP 103** | `/goal` | `role skills`, `role show` lists skills (TASK-0007) | `crates/orrery-cli/src/args.rs`, `crates/orrery-cli/src/commands.rs` | STEP 102 |
| 6 | **STEP 104** | `/goal` | `ProjectConfig::set_field`, `SETTABLE_KEYS` | `crates/orrery-core/src/config.rs` | STEP 103 |
| 7 | **STEP 105** | `/goal` | `ConfigStore::catalog_entries` with sources | `crates/orrery-core/src/config.rs` | STEP 104 |
| 8 | **STEP 106** | `/goal` | CLI `config set` / `config show` | `crates/orrery-cli/src/args.rs`, `crates/orrery-cli/src/commands.rs` | STEP 105 |
| 9 | **STEP 107** | `/goal` | `ProjectProfile`, load/save/list profiles | `crates/orrery-core/src/config.rs` | STEP 106 |
| 10 | **STEP 108** | `/goal` | `init_project_with_profile` | `crates/orrery-core/src/config.rs` | STEP 107 |
| 11 | **STEP 109** | `/goal` | CLI `init --profile`, `profile save` / `list` | `crates/orrery-cli/src/args.rs`, `crates/orrery-cli/src/commands.rs` | STEP 108 |
| 12 | **STEP 110** | `/goal` | README config, profiles and gate routing | `README.md` | STEP 109 |
| 13 | **[USER] LOOP-1** | — | Profile init, init refusal, TUI keeps CLI edits, Run ↔ Loop | scratch repo | **[USER]** runs the steps under "User Step: LOOP-1" after STEP 110. |

### Completed

| Step | Status | Command | Scope | Target Files |
| :--- | :---: | :---: | :--- | :--- |
| **STEP 1** | **COMPLETED** (2026-11-12) | `/goal` | Workspace scaffold, gates, core errors | `Cargo.toml`, `deny.toml`, `clippy.toml`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs` |
| **STEP 2** | **COMPLETED** (2026-11-12) | `/goal` | Config models and validation | `crates/orrery-core/src/config.rs`, `crates/orrery-core/src/lib.rs` |
| **STEP 3** | **COMPLETED** (2026-11-12) | `/goal` | `ConfigStore` and layered loading | `crates/orrery-core/src/config.rs`, `crates/orrery-core/Cargo.toml`, `Cargo.toml` |
| **STEP 4** | **COMPLETED** (gates re-run by Claude 2026-11-13) | `/goal` | `SecretValue`, `SecretStore`, OS keychain | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/security.rs`, `crates/orrery-core/tests/live_keyring.rs` |
| **STEP 5** | **COMPLETED** (gates re-run by Claude 2026-11-13) | `/goal` | `SecretRedactor` | `crates/orrery-core/src/security.rs` |
| **STEP 6** | **COMPLETED** (2026-11-13, validate-step PASS) | `/goal` | Plan step parser (`plan.rs`), `sha2` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/plan.rs` |
| **STEP 7** | **COMPLETED** (2026-11-13, validate-step PASS) | `/goal` | Agent reports, `chrono` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/checkpoint.rs` |
| **STEP 8** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | Workflow checkpoint and reconcile, `uuid` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/checkpoint.rs` |
| **STEP 9** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | Live stream hub and rotating log, `tokio` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/stream_log.rs` |
| **STEP 10** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | MCP gatekeeper, `WriteScope`, mock transport, `async-trait` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/mcp.rs` |
| **STEP 11** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | MCP transport via `rmcp`, `reqwest`, deny targets | `Cargo.toml`, `Cargo.lock`, `deny.toml`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/mcp.rs`, `crates/orrery-core/tests/live_mcp.rs` |
| **STEP 12** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | Inference config: attach to existing servers | `crates/orrery-core/src/config.rs` |
| **STEP 13** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | Container and inference types, mocks, pure helpers | `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/docker.rs` (NEW) |
| **STEP 14** | **COMPLETED** (2026-11-14, validate-step PASS + Claude review fixes) | `/boost` | `ResourceManager`, inference control, `.orrery` mask, path scope | `crates/orrery-core/src/docker.rs`, `crates/orrery-core/src/config.rs` |
| **STEP 15** | **COMPLETED** (2026-11-14, validate-step PASS + Claude review fixes) | `/goal` | Docker backend via `bollard`, `futures-util` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/docker.rs`, `crates/orrery-core/tests/live_docker.rs` (NEW) |
| **STEP 16** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | Prompt builders, `AgentDriver`, mock driver | `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/llm.rs` (NEW) |
| **STEP 17** | **COMPLETED** (2026-11-14, validate-step PASS + Claude review fix) | `/goal` | HTTP chat: request building and SSE parsing (pure) | `crates/orrery-core/src/llm.rs` |
| **STEP 18** | **COMPLETED** (2026-11-14, validate-step PASS) | `/boost` | `HttpChatDriver` turn loop | `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/llm.rs`, `crates/orrery-core/tests/live_llm.rs` (NEW) |
| **STEP 19** | **COMPLETED** (2026-11-14, validate-step PASS + Claude review fixes) | `/goal` | `CliContainerDriver` | `crates/orrery-core/src/llm.rs` |
| **STEP 20** | **COMPLETED** (2026-11-14, validate-step PASS) | `/goal` | Engine types and pure rules, `tokio-util` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `crates/orrery-core/src/lib.rs`, `crates/orrery-core/src/engine.rs` (NEW) |
| **STEP 21** | **COMPLETED** (2026-11-14, validate-step PASS) | `/boost` | Engine core loop, Run and Loop modes | `crates/orrery-core/src/engine.rs` |
| **STEP 22** | **COMPLETED** (2026-11-15, validate-step PASS after spec-gap correction) | `/boost` | Engine gates, retries, escalation, operator | `crates/orrery-core/src/engine.rs` |
| **STEP 23** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | Engine lifecycle hooks | `crates/orrery-core/src/engine.rs` |
| **STEP 24** | **COMPLETED** (2026-11-15, validate-step PASS after spec-gap correction) | `/goal` | Engine stage concurrency, validator broadcast | `crates/orrery-core/src/engine.rs` |
| **STEP 25** | **COMPLETED** (2026-11-15, validate-step PASS after spec-gap correction) | `/goal` | Engine interrupt, resume, plan reconcile | `crates/orrery-core/src/engine.rs` |
| **STEP 26** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | Engine assembly (`Backends`, `build_engine`), stdio command resolution | `crates/orrery-core/src/engine.rs`, `crates/orrery-core/src/mcp.rs` |
| **STEP 27** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | TUI crate, theme, banner, `ratatui`, `crossterm` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-tui/Cargo.toml` (NEW), `crates/orrery-tui/src/lib.rs` (NEW), `crates/orrery-tui/src/theme.rs` (NEW) |
| **STEP 28** | **COMPLETED** (2026-11-15, validate-step PASS after spec-gap correction) | `/goal` | TUI layout and hit testing | `crates/orrery-tui/src/lib.rs`, `crates/orrery-tui/src/layout.rs` (NEW) |
| **STEP 29** | **COMPLETED** (2026-11-15, validate-step PASS) | `/boost` | TUI app state and keyboard input | `crates/orrery-tui/src/lib.rs`, `crates/orrery-tui/src/app.rs` (NEW) |
| **STEP 30** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | TUI mouse input | `crates/orrery-tui/src/app.rs` |
| **STEP 31** | **COMPLETED** (2026-11-15, validate-step PASS) | `/boost` | TUI widgets and rendering | `crates/orrery-tui/src/lib.rs`, `crates/orrery-tui/src/widgets.rs` (NEW) |
| **STEP 32** | **COMPLETED** (2026-11-15, POST_GATE PASS; check flagged only Claude's mid-step plan and spec edits) | `/goal` | TUI ↔ engine bridge, `run_tui`; steps-list scroll, Settings MCP servers | `crates/orrery-tui/src/lib.rs`, `crates/orrery-tui/src/bridge.rs` (NEW), `crates/orrery-tui/src/app.rs`, `crates/orrery-tui/src/widgets.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 33** | **COMPLETED** (2026-11-15, POST_GATE PASS; check flagged only Claude's mid-step plan and spec edits) | `/goal` | CLI crate and argument parsing, `clap` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-cli/Cargo.toml` (NEW), `crates/orrery-cli/src/main.rs` (NEW), `crates/orrery-cli/src/args.rs` (NEW) |
| **STEP 34** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | CLI project and workflow commands | `crates/orrery-cli/src/main.rs`, `crates/orrery-cli/src/commands.rs` (NEW) |
| **STEP 35** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | CLI catalog and secret commands, `rpassword` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-cli/Cargo.toml`, `crates/orrery-cli/src/main.rs`, `crates/orrery-cli/src/commands.rs` |
| **STEP 36** | **COMPLETED** (2026-11-15, validate-step PASS) | `/boost` | CLI run, operator, status commands; TUI launch | `crates/orrery-cli/src/main.rs`, `crates/orrery-cli/src/commands.rs` |
| **STEP 37** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | Release readiness: CLI smoke test, steps-panel label, final checks | `crates/orrery-cli/tests/cli_smoke.rs` (NEW), `crates/orrery-tui/src/widgets.rs` |
| **[USER] LIVE-1** | **COMPLETED** (2026-11-15, operator; all live tests pass) | — | Live-backend tests on the Step 37 tree | — |
| **STEP 38** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | MCP child stderr into the live stream | `crates/orrery-core/src/mcp.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 39** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | `[limits]` table, guardrail note constant | `crates/orrery-core/src/config.rs` |
| **STEP 40** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | Chat idle timeout, sensitive header, limits wiring | `crates/orrery-core/src/llm.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 41** | **COMPLETED** (2026-11-15, validate-step PASS) | `/boost` | Live stream writer thread, configured sizes | `crates/orrery-core/src/stream_log.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 42** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | Context preflight, unmappable tool names | `crates/orrery-core/src/llm.rs` |
| **STEP 43** | **COMPLETED** (2026-11-15, validate-step PASS) | `/goal` | Connect referenced MCP servers only, cap hook history | `crates/orrery-core/src/engine.rs`, `crates/orrery-core/src/checkpoint.rs` |
| **STEP 44** | **COMPLETED** (2026-11-15, validate-step PASS) | `/boost` | Halted step-pass hook: Approve re-runs the hook | `crates/orrery-core/src/engine.rs` |
| **STEP 45** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | CLI error causes, status hint, log follower | `crates/orrery-cli/src/commands.rs`, `crates/orrery-cli/src/main.rs`, `crates/orrery-core/src/stream_log.rs` |
| **STEP 46** | **COMPLETED** (2026-11-16, validate-step PASS) | `/boost` | TUI controller never blocks the render loop | `crates/orrery-tui/src/bridge.rs` |
| **STEP 47** | **COMPLETED** (2026-11-16, validate-step PASS) | `/boost` | TUI operator answers in every mode and after restart | `crates/orrery-tui/src/app.rs`, `crates/orrery-tui/src/bridge.rs`, `crates/orrery-tui/src/widgets.rs` |
| **STEP 48** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | TUI cycles entry, Loop progress, ingest marker | `crates/orrery-tui/src/app.rs`, `crates/orrery-tui/src/widgets.rs`, `crates/orrery-tui/src/bridge.rs` |
| **STEP 49** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | TUI Settings warnings and agent details | `crates/orrery-tui/src/app.rs`, `crates/orrery-tui/src/widgets.rs` |
| **STEP 50** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Live tests: CloudApi, AgentCli, real-backends run | `crates/orrery-core/tests/live_llm.rs`, `crates/orrery-core/tests/live_docker.rs`, `crates/orrery-core/tests/live_engine.rs`, `crates/orrery-core/Cargo.toml` |
| **STEP 51** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Operator README | `README.md` |
| **STEP 52** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Zeroized Bearer header copy | `crates/orrery-core/src/llm.rs` |
| **STEP 53** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Report hidden MCP tools | `crates/orrery-core/src/mcp.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 54** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Byte-safe log follower | `crates/orrery-core/src/stream_log.rs` |
| **STEP 55** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Surface live-stream log write errors | `crates/orrery-core/src/stream_log.rs` |
| **STEP 56** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | TUI operator forwarder and Loop progress reset | `crates/orrery-tui/src/bridge.rs` |
| **STEP 57** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | README states only what is verified | `README.md` |
| **STEP 58** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | README TUI keys per panel | `README.md` |
| **STEP 59** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Audit: engine and checkpoints | `audit/59-engine-checkpoint.md` |
| **STEP 60** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Audit: config, security, plan, errors | `audit/60-config-security-plan.md` |
| **STEP 61** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Audit: docker and agent drivers | `audit/61-docker-llm.md` |
| **STEP 62** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Audit: MCP gatekeeper and live stream | `audit/62-mcp-stream.md` |
| **STEP 63** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Audit: TUI | `audit/63-tui.md` |
| **STEP 64** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Audit: CLI and README | `audit/64-cli-readme.md` |
| **STEP 65** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | Config ID integrity | `crates/orrery-core/src/config.rs` |
| **STEP 66** | **COMPLETED** (2026-11-16, validate-step PASS) | `/goal` | MCP write-scope hardening | `crates/orrery-core/src/mcp.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 67** | **COMPLETED** (2026-11-17, validate-step PASS after a gate fix; not a strike) | `/goal` | MCP timeouts | `crates/orrery-core/src/mcp.rs`, `crates/orrery-core/src/config.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 68** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | MCP stderr bytes and log tail | `crates/orrery-core/src/mcp.rs`, `crates/orrery-core/src/stream_log.rs` |
| **STEP 69** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Engine exit shutdown and stale decisions | `crates/orrery-core/src/engine.rs` |
| **STEP 70** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Fresh step reports and keychain notices | `crates/orrery-core/src/engine.rs`, `crates/orrery-core/src/checkpoint.rs` |
| **STEP 71** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | CLI container cleanup | `crates/orrery-core/src/llm.rs`, `crates/orrery-core/src/docker.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 72** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Container output line splitter | `crates/orrery-core/src/docker.rs` |
| **STEP 73** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | TUI decisions, interrupt, shutdown on error | `crates/orrery-tui/src/bridge.rs` |
| **STEP 74** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | TUI drain and checkpoint reconcile | `crates/orrery-tui/src/bridge.rs` |
| **STEP 75** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | TUI slot replace and footer keys | `crates/orrery-tui/src/app.rs`, `crates/orrery-core/src/config.rs` |
| **STEP 76** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | CLI: add-cli hyphens, status reconcile, startup notices | `crates/orrery-cli/src/args.rs`, `crates/orrery-cli/src/commands.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 77** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Line splitter caps every line | `crates/orrery-core/src/docker.rs` |
| **STEP 78** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | MCP stderr uses the line splitter | `crates/orrery-core/src/mcp.rs` |
| **STEP 79** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | remove_role checks peers first | `crates/orrery-core/src/config.rs` |
| **STEP 80** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Redacted run errors in the CLI | `crates/orrery-core/src/engine.rs`, `crates/orrery-cli/src/commands.rs` |
| **STEP 81** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Redacted run errors in the TUI | `crates/orrery-tui/src/bridge.rs` |
| **STEP 82** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | MCP bearer header | `crates/orrery-core/src/mcp.rs` |
| **STEP 83** | **COMPLETED** (2026-11-17, validate-step PASS) | `/goal` | Secret input without copies | `crates/orrery-cli/src/main.rs` |
| **STEP 84** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | Swap chrono for jiff in orrery-core | `Cargo.toml`, `Cargo.lock`, `crates/orrery-core/Cargo.toml`, `checkpoint.rs`, `stream_log.rs`, `llm.rs`, `engine.rs` |
| **STEP 85** | **COMPLETED** (2026-11-18, gate fixed by Claude, re-run by Claude) | `/goal` | Spec and `deny.toml` follow Step 84 | `ORRERY_TECHNICAL_SPECIFICATION.md`, `deny.toml` |
| **STEP 86** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | GitHub Actions CI: fmt, clippy, test | `.github/workflows/ci.yml` (NEW) |
| **STEP 87** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | README CI section | `README.md` |
| **STEP 88** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `HookConfig.args`, `ProjectConfig.gate_args`, `validate_tool_args` | `crates/orrery-core/src/config.rs`, `crates/orrery-core/src/engine.rs`, `crates/orrery-cli/src/commands.rs`, `crates/orrery-tui/src/widgets.rs` |
| **STEP 89** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `merge_tool_args`; hooks and gates send merged args | `crates/orrery-core/src/engine.rs` |
| **STEP 90** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `hook set --args`, `gate args`, stale `gate_args` cleanup | `crates/orrery-cli/src/args.rs`, `crates/orrery-cli/src/commands.rs` |
| **STEP 91** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | Live MCP call with merged args | `crates/orrery-core/tests/live_mcp.rs` |
| **STEP 92** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | README hook/gate args and live env vars | `README.md` |
| **[USER] NEB-1** | **COMPLETED** (2026-11-18, operator run) | — | Acceptance: `on_step_pass` → Nebulous `git_commit_documented` | scratch repo: `dry_run: false` committed `.tracker/` only; `dry_run` omitted → no commit, hook reported `"dry_run": true`. Model: Qwen3.8-27B IQ3_XXS with written role rules. |
| **STEP 93** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `allowed_statuses`, `requires_feedback`, `protocol_contract`, `build_agent_system_prompt` | `crates/orrery-core/src/checkpoint.rs`, `crates/orrery-core/src/llm.rs` |
| **STEP 94** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | Engine sends the contract in every system prompt | `crates/orrery-core/src/engine.rs` |
| **STEP 95** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `RoleConfig.skills` + validation | `crates/orrery-core/src/config.rs` + one-line literal fixes in 10 files |
| **STEP 96** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `load_skill`, `compose_role_rules`, skills in `build_engine` | `crates/orrery-core/src/config.rs`, `crates/orrery-core/src/engine.rs` |
| **STEP 97** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | `role add --skill`, awaiting-operator message, parked comment | `crates/orrery-cli/src/args.rs`, `crates/orrery-cli/src/commands.rs` |
| **STEP 98** | **COMPLETED** (2026-11-18, validate-step PASS) | `/goal` | Default role rules describe behavior; parked `rustfmt::skip` | `crates/orrery-core/src/config.rs` |
| **STEP 99** | **COMPLETED** (2026-11-18, validate-step PASS with empty `XDG_CONFIG_HOME`) | `/goal` | README roles, protocol and skills | `README.md` |
| **STEP 100** | **COMPLETED** (2026-11-19, validate-step PASS) | `/goal` | `init` refuses a configured project (BUG-0013) | `crates/orrery-core/src/config.rs` |

---

## Rules for every step

These apply to every block. A block never repeats them.

- **Load the Rust skills** before editing (`Orrery_Initial.md`).
- **Where the design lives.** `READ_ONLY_REFS` names spec lines as `ORRERY_TECHNICAL_SPECIFICATION.md:<from>-<to>`. Those lines are binding: copy the signatures as written. `MUTATION_SPEC` adds only what the spec leaves out (file placement, test names and inputs). Each `PRE_GATE` checks that the spec's section heading still sits on the cited line, so a stale reference fails the pre-gate instead of misleading you.
- **Dependencies.** Add exactly the lines the step gives (from spec §3.1). Don't look up versions or features. After editing manifests, run `cargo fetch` once. It's the only cargo command without `--locked`. Never hand-edit `Cargo.lock`.
- **Every other cargo command except `cargo fmt` uses `--locked`.** Run `cargo fmt --all` before the `POST_GATE`.
- **Code rules.** No `#[ignore]`, no `#[allow(...)]`, no empty or assertion-free tests, no `.unwrap()`/`.expect()` outside tests. Tests go in `#[cfg(test)] mod tests` at the bottom of the file they test (spec §9.0), so test paths are `<module>::tests::<name>`.
- **Tests.** The named tests are the minimum, each with real assertions, using exactly the given names and inputs. The `POST_GATE` counts them by name, so extra tests (for the coverage floor) never break it.
- **Fixtures** use reserved placeholders only (spec §8): `example.com` / `.test`, `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`, `testuser`. No absolute host paths. No server product other than llama.cpp.
- **Paths.** Committed files (code, tests, fixtures, docs) never hold a host's absolute path; tests build every path from a `TempDir`. At run time Orrery takes host paths only from the launch workspace, the machine-scope global dir or `PATH`, and checks each one before use (spec §9.6a path scope, §9.5 `resolve_stdio_command`).
- **Live-backend tests** (`tests/live_*.rs`) are written and compiled, never run by the agent. The operator runs them.
- **Free choices.** Anything spec §9.0 calls a free choice (private helpers, extra derives, field order, test helpers): decide, don't halt. List each one in one line of the step report.
- **Network.** If the sandbox blocks network, run `cargo deny --offline check` in place of `cargo deny check` and say so in the report.
- **Halt** only when:
  - the `PRE_GATE` exits non-zero;
  - the `POST_GATE` can't be made to pass inside `ALLOWED_FILES`;
  - a cited spec signature doesn't compile as written;
  - a named test can't pass as described.
  Report the exact command, its output, and the spec line involved.
- **Step report** (on finish or halt): gate results, files changed, free choices made, anything the operator must do.

---

## Steps 1–5 (completed; kept as executed)

These blocks are the record of what ran, in the pre-2026-11-13 format. They aren't re-run.

### STEP 1: Workspace Scaffold, Gates & Core Errors

**Status:** DONE (gates passed 2026-11-12). Kept as written.

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-01: Workspace Scaffold, Gates & Core Errors

[SCOPE_LOCK]
- Cargo.toml
- deny.toml
- clippy.toml
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs

[PRE_GATE]
- `cargo deny --version` and `cargo llvm-cov --version` succeed. If `cargo llvm-cov` is missing, halt and ask the operator to run `cargo install cargo-llvm-cov --locked`. Also verify `rustup component list --installed` includes `llvm-tools`; if not, halt and ask the operator to run `rustup component add llvm-tools-preview`.

[MUTATION_SPEC]
1. Root `Cargo.toml`: virtual workspace, resolver 3, member `crates/orrery-core`, `[workspace.package]` (edition 2024, `rust-version` = installed stable), `[workspace.dependencies]` with only `thiserror`, `serde`, `serde_json`, `toml`, `tempfile` (latest stable now), and `[workspace.lints]` per spec §8.
2. `deny.toml` per spec §8. `clippy.toml` with the test allowances in spec §8.
3. `crates/orrery-core/Cargo.toml` using workspace deps and `lints.workspace = true`.
4. `lib.rs`: `Error` and `Result` per spec §5.1, **only** the variants that need no crate beyond this step (`Io`, `TomlDe`, `TomlSer`, `Json`, `PathTraversal`, `Validation`, `Security`, `PlanParse`, `Report`, `Agent`, `Engine`, `Interrupted`). Later steps add the others.
5. Tests: `From` conversions and that `source()` is preserved for wrapped errors.
6. Generate and keep `Cargo.lock`.
Note: 5 files because the gates need a member crate and a deny policy to run at all.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if `openssl-sys` appears in `cargo tree`.
```

---

### STEP 2: Config Models & Validation

**Status:** DONE (gates passed 2026-11-12). Kept as written.

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-02: Config Models & Validation

[SCOPE_LOCK]
- crates/orrery-core/src/config.rs
- crates/orrery-core/src/lib.rs

[PRE_GATE]
- `STEP-01` post-gates pass.

[MUTATION_SPEC]
1. Implement the types in spec §5.2 and the schemas in spec §4.1–§4.4 (`deny_unknown_fields` on all of them).
2. Implement `default_template`, `validate`, `validate_for_run`, `effective_validation_mode`, `set_stage_count`, `set_stage_role`, `assign_agent`, `remove_agent`, `clear_stage`, `AgentConfig::validate_no_embedded_secrets`.
3. Tool-id rule from spec §5.8 (ids must map to `[A-Za-z0-9_-]{1,64}` as `<server>__<tool>`): validate at load.
4. Tests: every invariant in spec §4.1 at its boundary (1 and 16 stages rejected; 2 and 15 accepted; orchestrator not at stage 0 rejected; two orchestrator stages rejected; 16 agents rejected; Auto→Agents with a validator, Auto→GateAndOperator without); TOML round-trips for each example in spec §4; embedded-secret rejection.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). No file I/O in this step.
```

---

### STEP 3: ConfigStore & Layered Loading

**Status:** DONE (gates passed 2026-11-12). Kept as written.

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-03: ConfigStore & Layered Loading

[SCOPE_LOCK]
- crates/orrery-core/src/config.rs
- crates/orrery-core/Cargo.toml
- Cargo.toml

[PRE_GATE]
- `STEP-02` post-gates pass.

[MUTATION_SPEC]
1. Add `dirs` to workspace deps.
2. Implement `ConfigStore` per spec §5.2: `new`, `init_project`, `resolve_safe_path`, load/save/remove for project, roles (+ `.md`), agents, MCP servers, inference. Atomic writes per spec §4 (temp file in same dir, fsync, rename).
3. Layering: global first, project replaces by id.
4. Tests in `tempfile::TempDir` with injected XDG dir: init layout matches spec §4, override by id, `..` and symlink escape rejected, atomic save leaves no temp files, missing optional files load as empty.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins).
```

---

### STEP 4: SecretValue, SecretStore & OS Keychain

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-04: SecretValue, SecretStore & OS Keychain

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/security.rs
- crates/orrery-core/tests/live_keyring.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.
- `pkg-config --exists dbus-1` succeeds. If not, halt and ask the operator to run `sudo apt install libdbus-1-dev pkg-config`.

[MUTATION_SPEC]
1. Root `Cargo.toml` `[workspace.dependencies]`, add:
     secrecy = "0.10.3"
     zeroize = "1.9.1"
     keyring-core = "1.0.1"
     dbus-secret-service-keyring-store = { version = "1.0.1", features = ["crypto-rust"] }
     linux-keyutils-keyring-store = "1.0.0"
   `crates/orrery-core/Cargo.toml` `[dependencies]`: the same five as `<name>.workspace = true`.
   `crates/orrery-core/Cargo.toml`: add `[features]` with `live-backends = []`, and
     [[test]]
     name = "live_keyring"
     required-features = ["live-backends"]
   Run `cargo fetch`.
2. `lib.rs`: add `pub mod security;` and the variant `#[error("keychain error")] Keychain(#[source] keyring_core::Error)` (spec §5.1).
3. `security.rs`: implement §9.1 `REDACTED`, `SecretValue`, `SecretStore`, `KeyringBackend`, `OsKeyringStore`, `InMemorySecretStore`. (`SecretRedactor` is Step 5.)
4. `tests/live_keyring.rs`: one test `os_keyring_round_trip` — `OsKeyringStore::new()`, set/get/delete service `"orrery.test"`, account `"testuser"`, value `"live-test-value-123"`; then `get` returns `Error::Keychain`. Print `backend()` with `eprintln!`.
5. Unit tests in `security.rs`:
   - `secret_value_debug_and_display_are_redacted` — `format!("{:?}")` and `format!("{}")` both equal `"[REDACTED]"` and don't contain the value.
   - `secret_value_len_and_expose` — `len()`, `is_empty()`, `expose()` return the original.
   - `in_memory_set_get_delete` — round trip; second `delete` is `Ok`.
   - `in_memory_missing_is_keychain_no_entry` — `get` on an unknown pair matches `Error::Keychain(keyring_core::Error::NoEntry)`.
   - `in_memory_entries_are_keyed_by_service_and_account` — same account, two services, two values.
   - `keychain_error_preserves_source` — `Error::Keychain(NoEntry).source()` is `Some`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if `cargo tree --locked -i zbus` reports a match (a "did not match any packages" error is the expected result), or if `SecretValue` implements `Clone`, `Serialize` or a non-redacting `Debug`/`Display`.
```

---

### STEP 5: SecretRedactor

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-05: SecretRedactor

[SCOPE_LOCK]
- crates/orrery-core/src/security.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Implement §9.1 `SecretRedactor` (`new`, `register`, `redact`) with the private helper `redact_prefixed` exactly as §9.1 describes. No regex crate.
2. Tests:
   - `redacts_registered_secret` — register `"s3cr3t-value-xyz"`; `"token=s3cr3t-value-xyz;"` → `"token=[REDACTED];"`.
   - `short_registered_secret_is_ignored` — register `"abc1234"` (7 chars); text containing it is unchanged.
   - `longest_registered_secret_wins` — register `"abcdefgh"` and `"abcdefgh-ijkl"`; `"abcdefgh-ijkl"` → `"[REDACTED]"` (not `"[REDACTED]-ijkl"`).
   - `redacts_bearer_token` — `"Authorization: Bearer abc.def"` → `"Authorization: [REDACTED]"`.
   - `redacts_sk_key_at_min_length` — `"sk-ABCDEFGH"` → `"[REDACTED]"`; `"sk-ABCDEFG"` (7) unchanged.
   - `redacts_google_key` — `"AIza"` + 20 × `"A"` → `"[REDACTED]"`; with 19 unchanged.
   - `redacts_github_token` — `"ghp_"` + 20 × `"a"` → `"[REDACTED]"`; `"ghp_"` + 20 × `"-"` unchanged.
   - `multiple_matches_in_one_line` — one Bearer and one `sk-` in the same string are both redacted.
   - `plain_text_unchanged` — `"cargo test passed in 3.2s"` unchanged.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins).
```

---

## Phase A — `orrery-core` (Steps 6–26)

### STEP 6: Plan Step Parser

* **Recommended Command:** `/goal`

Scope note (written 2026-11-13, checked against the Step 5 tree: 39 tests, all gates green):
- **`lib.rs`** has `pub mod config;` and `pub mod security;` (lines 1–2). `Error::PlanParse { line: usize, message: String }` (line 33) and `Error::Io { path, source }` (line 9) already exist, so this step adds no variant.
- **No `sha2` yet** in either manifest. `sha2 = "0.11.1"` is the spec §3.1 line, resolved and `cargo deny`-checked 2026-11-12.
- **Why Gemini halted on the old block (2026-11-13):** the prompt it was given said 36 steps, while this file has 37. The prompt was a stale copy. This file is the source now (see "Who runs what").
- **Acceptance fixture marker:** this file's step headings are `### STEP <N>: <title>`, so the fixture parses with marker `"### STEP "`. Each title then starts `"<N>: "` and numbers 1..=37. Spec §5.4 says the same, as of 2026-11-13. The product default `step_marker = "## Step "` in `config.rs` is for users' plans and doesn't change.

```text
[STEP_ID]
STEP 6: plan.rs — ImplementationPlan / PlanStep parser with fence tracking, contiguous numbering and SHA-256 step hashes

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/plan.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:700-717 (§9.2: signatures in the rust block, then the normalize/fence/title/body/number/hash rules; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:343-355 (§5.4: parser summary and acceptance fixture)
  - ORRERY_TECHNICAL_SPECIFICATION.md:649-656 (§9.0: derives, test placement)
  - crates/orrery-core/src/lib.rs:1-2 (module list), :8-13 (Io variant), :32-33 (PlanParse variant)
  - STEPPED_IMPLEMENTATION_PLAN.md (the acceptance fixture; read only)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No new Error variant. No `hex` or `regex` crate: hex is hand-written per §9.2. config.rs is untouched (its `step_marker = "## Step "` default stays). This file and the spec are never edited.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test ! -e crates/orrery-core/src/plan.rs && ! grep -q '^sha2' Cargo.toml && grep -q 'PlanParse { line: usize, message: String }' crates/orrery-core/src/lib.rs && sed -n 700p $S | grep -q '^### 9.2 ' && sed -n 343p $S | grep -q '^### 5.4 ' && sed -n 354p $S | grep -q 'marker `### STEP `' && test "$(grep -c '^### STEP ' STEPPED_IMPLEMENTATION_PLAN.md)" -eq 37

[MUTATION_SPEC]
1. Root Cargo.toml `[workspace.dependencies]`: add `sha2 = "0.11.1"` (alphabetical, after `serde_json`). crates/orrery-core/Cargo.toml `[dependencies]`: add `sha2.workspace = true`. Run `cargo fetch`.
2. lib.rs: add `pub mod plan;` between `pub mod config;` and `pub mod security;`.
3. plan.rs: the §9.2 rust block (spec lines 701-711) as written, implementing the rules on spec lines 712-716. `PlanStep` and `ImplementationPlan` derive `Debug, Clone, PartialEq, Eq`. `from_file` sets `source_path = Some(path.to_path_buf())`. A read error becomes `Error::Io { path, source }`.
4. Tests in `#[cfg(test)] mod tests` at the bottom of plan.rs. Marker `"## S "` unless stated otherwise:
   - `sha256_hex_known_vector`: `sha256_hex(b"abc")` == `"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"`.
   - `parses_preamble_and_steps`: input `"intro\n## S 1: a\nbody a\n## S 2: b\nbody b\n"`. `preamble == "intro"`; titles `"1: a"`, `"2: b"`; bodies `"body a"`, `"body b"`; `index` 0, 1; `number` 1, 2.
   - `marker_inside_backtick_fence_ignored`: input `"## S 1: a\n` + three backticks + `\n## S 2: fake\n` + three backticks + `\n"`. One step, and its body contains `"## S 2: fake"`.
   - `marker_inside_tilde_fence_ignored`: the same with `~~~`.
   - `longer_fence_contains_shorter_fence_line`: a step whose body opens a 4-backtick fence, then has a 3-backtick line and a `"## S 2: fake"` line, then closes the 4-backtick fence. Still one step.
   - `unclosed_fence_errors_with_line`: input `"## S 1: a\ntext\n` + three backticks + `\n## S 2: b\n"` gives `Error::PlanParse { line: 3, .. }`.
   - `empty_marker_rejected`: `parse("## S 1: a\n", "")` matches `Error::PlanParse { .. }`.
   - `zero_steps_rejected`: `"no markers here\n"` matches `Error::PlanParse { .. }`.
   - `numbering_gap_rejected`: `"## S 1: a\n## S 3: b\n"` gives `Error::PlanParse { line: 2, .. }`.
   - `mixed_numbered_and_unnumbered_rejected`: `"## S 1: a\n## S b\n"` gives `Error::PlanParse { line: 2, .. }`.
   - `unnumbered_titles_use_index`: `"## S alpha\n## S beta\n"` gives numbers 1, 2.
   - `crlf_and_lf_hash_identical`: the `parses_preamble_and_steps` input with `"\n"` and with `"\r\n"`. Every `sha256_hex` and `plan_sha256()` is equal.
   - `body_edit_changes_hash`: the same input with `"body b"` changed to `"body c"`. Step 2's hash and `plan_sha256()` differ; step 1's hash is equal.
   - `step_lookup_by_number`: on the `parses_preamble_and_steps` plan, `step(2)` has title `"2: b"`; `step(3)` and `step(0)` are `None`.
   - `from_file_reads_and_sets_source_path`: write the `parses_preamble_and_steps` input to `tempfile::TempDir` + `plan.md`. `from_file` gives 2 steps and `source_path == Some(<that path>)`.
   - `from_file_missing_is_io_error`: a missing file in a TempDir matches `Error::Io { .. }`.
   - `acceptance_fixture_counts_plan_steps`: `ImplementationPlan::parse(include_str!("../../../STEPPED_IMPLEMENTATION_PLAN.md"), "### STEP ")`. `steps.len() == 37`; the `number`s equal `(1..=37).collect::<Vec<_>>()`; `steps[5].title` starts with `"6: "`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^sha2 = "0.11.1"$' Cargo.toml && grep -q '^sha2.workspace = true$' crates/orrery-core/Cargo.toml && grep -q '^pub mod plan;$' crates/orrery-core/src/lib.rs && ! grep -qE '^(hex|regex)' Cargo.toml crates/orrery-core/Cargo.toml && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^plan::tests::(sha256_hex_known_vector|parses_preamble_and_steps|marker_inside_backtick_fence_ignored|marker_inside_tilde_fence_ignored|longer_fence_contains_shorter_fence_line|unclosed_fence_errors_with_line|empty_marker_rejected|zero_steps_rejected|numbering_gap_rejected|mixed_numbered_and_unnumbered_rejected|unnumbered_titles_use_index|crlf_and_lf_hash_identical|body_edit_changes_hash|step_lookup_by_number|from_file_reads_and_sets_source_path|from_file_missing_is_io_error|acceptance_fixture_counts_plan_steps): test$')" -eq 17

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

Decisions the contract leaves open:
- **The acceptance test reads this file at compile time.** An unbalanced code fence or a renumbered heading here breaks `cargo test` for every later step. Whoever edits this file runs `cargo test -p orrery-core --lib --locked plan::tests::acceptance_fixture_counts_plan_steps` afterwards.
- **Two tests are new** compared with the 2026-11-12 block: `mixed_numbered_and_unnumbered_rejected` and `step_lookup_by_number`. Both test §9.2 rules the old list didn't cover.

---

### STEP 7: Agent Reports

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13 against the Step 5 tree. Claude re-checks the `PRE_GATE` against the Step 6 tree before handing it off.

```text
[STEP_ID]
STEP 7: checkpoint.rs part 1 — AgentReport, ReportStatus, SubtaskAllocation, validate_for_role, report_json_schema, StampedReport

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/checkpoint.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:718-747 (§9.3 first rust block + validate_for_role rules 1-5 + report_json_schema; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:356-381 (§5.5: status-per-role table)
  - ORRERY_TECHNICAL_SPECIFICATION.md:649-656 (§9.0)
  - crates/orrery-core/src/config.rs (RoleConfig, RoleKind; validate_safe_relative_path for write scopes)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Nothing from the second §9.3 block (WorkflowCheckpoint, CheckpointManager: Step 8). No uuid yet.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^pub mod plan;$' crates/orrery-core/src/lib.rs && test ! -e crates/orrery-core/src/checkpoint.rs && ! grep -q '^chrono' Cargo.toml && sed -n 718p $S | grep -q '^### 9.3 ' && sed -n 748p $S | grep -q '^```rust'

[MUTATION_SPEC]
1. Root `[workspace.dependencies]`: `chrono = { version = "0.4.45", default-features = false, features = ["clock", "std", "serde"] }`. Crate `[dependencies]`: `chrono.workspace = true`. Run `cargo fetch`.
2. lib.rs: `pub mod checkpoint;` (alphabetical, before `pub mod config;`).
3. checkpoint.rs: the first §9.3 rust block (spec lines 719-738), with `validate_for_role` applying rules 1-5 (lines 739-744) in that order and `report_json_schema` per lines 746-747.
4. Tests (test helper `fn role(kind: RoleKind) -> RoleConfig`, a free choice):
   - `orchestrator_statuses`: `Dispatch`/`StepPassed`/`Escalate` allowed; the other 4 rejected.
   - `implementer_statuses`: `Complete`/`Blocked` allowed; others rejected.
   - `validator_statuses`: `Confirmed`/`Denied` allowed; others rejected.
   - `summary_empty_or_too_long_rejected`: `"  "` rejected; 4096 bytes accepted; 4097 rejected.
   - `dispatch_requires_target_and_allocations`: with `stage_count = 3`, each of these is rejected: missing `target_stage`, `target_stage = 0`, `target_stage = 3`, empty allocations.
   - `dispatch_rejects_bad_write_scope`: allocation write scope `"../x"` rejected.
   - `non_dispatch_rejects_target_or_allocations`.
   - `feedback_required_for_denied_blocked_escalate`.
   - `unknown_field_rejected`: TOML with an extra key fails to deserialize.
   - `stamped_report_toml_round_trip`.
   - `json_schema_required_matches_struct`:
     - schema `required` == `["status","summary"]`;
     - schema property names == keys of a fully populated `AgentReport` serialized to JSON;
     - `status.enum` has 7 entries;
     - a JSON value with only `status` + `summary` deserializes into `AgentReport`, and one missing `summary` doesn't.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^chrono = ' Cargo.toml && grep -q '^pub mod checkpoint;$' crates/orrery-core/src/lib.rs && ! grep -q 'struct WorkflowCheckpoint' crates/orrery-core/src/checkpoint.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^checkpoint::tests::(orchestrator_statuses|implementer_statuses|validator_statuses|summary_empty_or_too_long_rejected|dispatch_requires_target_and_allocations|dispatch_rejects_bad_write_scope|non_dispatch_rejects_target_or_allocations|feedback_required_for_denied_blocked_escalate|unknown_field_rejected|stamped_report_toml_round_trip|json_schema_required_matches_struct): test$')" -eq 11

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 8: Workflow Checkpoint & Reconcile

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `config::atomic_write` is `pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()>` (config.rs:1274).

```text
[STEP_ID]
STEP 8: checkpoint.rs part 2 — WorkflowCheckpoint (new, reconcile, reset_from), ReconcileOutcome, CheckpointManager

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/checkpoint.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:748-778 (§9.3 second rust block + reconcile rule; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:125-146 (§4: checkpoints/ layout and report file names)
  - crates/orrery-core/src/config.rs:1274 (atomic_write; reuse, don't re-implement)
  - crates/orrery-core/src/plan.rs (ImplementationPlan::parse for test plans)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. lib.rs is not edited (the module exists). Every write goes through config::atomic_write.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct AgentReport' crates/orrery-core/src/checkpoint.rs && ! grep -q 'struct WorkflowCheckpoint' crates/orrery-core/src/checkpoint.rs && ! grep -q '^uuid' Cargo.toml && sed -n 748p $S | grep -q '^```rust' && grep -q '^pub fn atomic_write' crates/orrery-core/src/config.rs

[MUTATION_SPEC]
1. Root: `uuid = { version = "1.29.2", features = ["v4", "serde"] }`. Crate: `uuid.workspace = true`. Run `cargo fetch`.
2. checkpoint.rs: the second §9.3 rust block (spec lines 748-776): `StepStatus`, `EnginePhase`, `OperatorDecision`, `HookResult`, `WorkflowCheckpoint` (`new`, `reconcile`, `reset_from`), `ReconcileOutcome`, `CheckpointManager`. `reconcile` follows spec lines 777-778.
3. Tests (TempDir; plans built with `ImplementationPlan::parse`):
   - `new_checkpoint_all_pending`.
   - `state_save_load_round_trip`.
   - `load_state_missing_is_none`.
   - `report_path_format`: `save_report` returns a path ending `step_001/round_02_stage_01_local-coder.toml`.
   - `load_reports_filters_and_sorts`: 3 reports over 2 rounds. `round: Some(1)` returns only round 1; `None` returns all, in (round, stage, agent) order.
   - `delete_reports_removes_round_only`.
   - `reconcile_unchanged`.
   - `reconcile_accepts_later_edit`: step 3 edited while current is step 1 gives `Updated { changed_steps: [2], .. }`, and the stored hash changes.
   - `reconcile_accepts_appended_step`.
   - `reconcile_rejects_passed_step_edit`: gives `Error::Engine { step_number: 1, .. }`.
   - `reconcile_rejects_current_in_progress_edit`.
   - `reconcile_removed_pending_ok_removed_passed_err`.
   - `reset_from_marks_later_pending`: steps 1-3 `Passed`, then `reset_from(2)`, gives `[Passed, Pending, Pending]` and `current_step_index == 1`.
   - `unknown_field_in_state_rejected`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^uuid = ' Cargo.toml && grep -q 'atomic_write' crates/orrery-core/src/checkpoint.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^checkpoint::tests::(new_checkpoint_all_pending|state_save_load_round_trip|load_state_missing_is_none|report_path_format|load_reports_filters_and_sorts|delete_reports_removes_round_only|reconcile_unchanged|reconcile_accepts_later_edit|reconcile_accepts_appended_step|reconcile_rejects_passed_step_edit|reconcile_rejects_current_in_progress_edit|reconcile_removed_pending_ok_removed_passed_err|reset_from_marks_later_pending|unknown_field_in_state_rejected): test$')" -eq 14

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 9: Live Stream Hub & Rotating Log

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. tokio's feature list is replaced (not appended) at Steps 9, 10, 11 and 33 (spec §3.2).

```text
[STEP_ID]
STEP 9: stream_log.rs — LiveStreamHub (ring, redaction, rotating JSON-lines log, broadcast) and tail_log

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/stream_log.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:779-801 (§9.4 rust block + file format, emit order, rotation, oversize rule; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:382-390 (§5.6)
  - ORRERY_TECHNICAL_SPECIFICATION.md:112-124 (§3.2 tokio features: Step 9 row)
  - crates/orrery-core/src/security.rs (SecretRedactor::redact returns Result)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. tokio features exactly ["sync"] in this step.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'struct WorkflowCheckpoint' crates/orrery-core/src/checkpoint.rs && test ! -e crates/orrery-core/src/stream_log.rs && ! grep -q '^tokio' Cargo.toml && sed -n 779p $S | grep -q '^### 9.4 '

[MUTATION_SPEC]
1. Root: `tokio = { version = "1.54.1", features = ["sync"] }`. Crate: `tokio.workspace = true`. Run `cargo fetch`.
2. lib.rs: `pub mod stream_log;` (alphabetical).
3. stream_log.rs: §9.4 as written (spec lines 780-796), with the emit order, rotation and oversize-line rules on lines 797-801.
4. Tests (TempDir; small caps `max_file_bytes = 512`, `max_files = 3`, `ring_capacity = 4`):
   - `ring_evicts_oldest`: after 6 emits, `recent(10)` returns the last 4 in order.
   - `secret_redacted_in_ring_and_file`: a registered secret never appears in `recent` or in any log file.
   - `rotation_order`: after enough emits, `.log`, `.log.1` and `.log.2` exist, and `.log.2` holds the oldest surviving events.
   - `oldest_file_dropped`: `.log.3` never exists.
   - `total_bytes_never_exceed_cap`: 1,000 emits; after each, the three file sizes sum to ≤ `512 * 3`.
   - `oversize_line_is_shortened`: one event with 2,000-char text is written as a line ≤ 512 bytes.
   - `subscriber_receives_redacted_event`: a `tokio::sync::broadcast` receiver's `try_recv` gets it (no runtime needed).
   - `emit_without_subscribers_is_ok`.
   - `tail_log_reads_across_rotated_files`: returns the last N lines, oldest first, across `.log.2`, `.log.1` and `.log`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^tokio = { version = "1.54.1", features = \["sync"\] }$' Cargo.toml && grep -q '^pub mod stream_log;$' crates/orrery-core/src/lib.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^stream_log::tests::(ring_evicts_oldest|secret_redacted_in_ring_and_file|rotation_order|oldest_file_dropped|total_bytes_never_exceed_cap|oversize_line_is_shortened|subscriber_receives_redacted_event|emit_without_subscribers_is_ok|tail_log_reads_across_rotated_files): test$')" -eq 9

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 10: MCP Gatekeeper, WriteScope & Mock Transport

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `config::format_mapped_tool_name` (config.rs:1098) and `config::validate_concrete_tool_ref` (config.rs:1116) are public; reuse them.

```text
[STEP_ID]
STEP 10: mcp.rs part 1 — McpTransport/McpConnector traits, WriteScope, McpGatekeeper, result helpers, mocks; Error::Mcp

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/mcp.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:802-843 (§9.5 rust block + path normalization + execute_for_role order; binding. The RmcpTransport::connect bullet is Step 11)
  - ORRERY_TECHNICAL_SPECIFICATION.md:418-430 (§5.8)
  - ORRERY_TECHNICAL_SPECIFICATION.md:298-324 (§5.1: Mcp variant text)
  - crates/orrery-core/src/config.rs:1098 (format_mapped_tool_name), :1116 (validate_concrete_tool_ref)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No RmcpTransport/RmcpConnector and no rmcp/reqwest dependency (Step 11).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^pub mod stream_log;$' crates/orrery-core/src/lib.rs && test ! -e crates/orrery-core/src/mcp.rs && ! grep -q '^async-trait' Cargo.toml && sed -n 802p $S | grep -q '^### 9.5 '

[MUTATION_SPEC]
1. Root: add `async-trait = "0.1.92"`; replace the tokio line's features with `["sync", "macros", "rt"]`. Crate: `async-trait.workspace = true`. Run `cargo fetch`.
2. lib.rs: `pub mod mcp;` (alphabetical) and the variant `#[error("MCP error on {server_id}")] Mcp { server_id: String, #[source] source: Box<dyn std::error::Error + Send + Sync> }`, placed after `Keychain` in §5.1 order.
3. mcp.rs, from §9.5: `McpToolSchema`, `McpTransport`, `McpConnector`, `WriteScope`, `scopes_overlap`, `tool_result_is_error`, `tool_result_text`, `McpGatekeeper`, `MockMcpTransport`, `MockMcpConnector`.
4. Tests (`#[tokio::test]` where async):
   - `exact_pattern_allows_only_that_tool`.
   - `server_wildcard_allows_server_tools`.
   - `star_star_not_honored`.
   - `disallowed_call_is_security_error_and_never_reaches_transport`: the mock's `calls()` is empty afterwards.
   - `unregistered_server_is_security_error`.
   - `write_tool_path_outside_scope_rejected`: scope `["src"]`, path `"docs/a.md"` gives `Error::Security` and zero calls.
   - `write_tool_path_inside_scope_allowed`: `"src/lib.rs"` reaches the mock.
   - `write_tool_dotdot_rejected`: `"src/../etc/x"` rejected.
   - `write_tool_non_string_path_rejected`.
   - `non_write_tool_ignores_scope`.
   - `anywhere_scope_contains_relative_paths`.
   - `anywhere_scope_rejects_absolute`.
   - `scopes_overlap_cases`: `("src","src/lib.rs")` true, `("src","src2")` false, `("./src/","src")` true.
   - `tools_for_role_filters_catalog`: after `refresh_catalog`, only allowed tools are returned.
   - `tool_result_helpers`: `isError: true` is detected; text parts are joined with `"\n"`.
   - `failing_tool_returns_mcp_error`.
   - `mock_connector_unknown_server_errors`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^async-trait = "0.1.92"$' Cargo.toml && grep -q 'features = \["sync", "macros", "rt"\]' Cargo.toml && grep -q '^pub mod mcp;$' crates/orrery-core/src/lib.rs && ! grep -q 'Rmcp' crates/orrery-core/src/mcp.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^mcp::tests::(exact_pattern_allows_only_that_tool|server_wildcard_allows_server_tools|star_star_not_honored|disallowed_call_is_security_error_and_never_reaches_transport|unregistered_server_is_security_error|write_tool_path_outside_scope_rejected|write_tool_path_inside_scope_allowed|write_tool_dotdot_rejected|write_tool_non_string_path_rejected|non_write_tool_ignores_scope|anywhere_scope_contains_relative_paths|anywhere_scope_rejects_absolute|scopes_overlap_cases|tools_for_role_filters_catalog|tool_result_helpers|failing_tool_returns_mcp_error|mock_connector_unknown_server_errors): test$')" -eq 17

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 11: MCP Transport via `rmcp`

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `deny.toml` has no `[graph]` table yet. `cargo deny` passes today, but `reqwest`/`rustls` bring in the Android-only `webpki-root-certs` (CDLA-Permissive-2.0), which fails the license check unless the targets are limited (spec §8).

Amended 2026-11-14 after Step 10's review: `execute_for_role` only checked path args via `args.get(..)`, so a JSON array or string as args skipped the write-scope check. The spec relied on `RmcpTransport::call_tool` rejecting non-object args, which can't be unit-tested without a live server. Spec §9.5 line 841 now has check (0) in the gatekeeper itself, and this step adds it plus `non_object_args_rejected_before_transport`.

```text
[STEP_ID]
STEP 11: mcp.rs part 2 — RmcpTransport + RmcpConnector (stdio, streamable HTTP), Error::Http, deny.toml targets, live_mcp test

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - deny.toml
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/mcp.rs
  - crates/orrery-core/tests/live_mcp.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:802-843 (§9.5: RmcpTransport, RmcpConnector and the RmcpTransport::connect bullet; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:621-644 (§8: deny targets, live-test env vars)
  - ORRERY_TECHNICAL_SPECIFICATION.md:112-124 (§3.2: Step 11 tokio row)
  - crates/orrery-core/tests/live_keyring.rs (live-test layout to follow)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No openssl. The live test is compiled, never run.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct McpGatekeeper' crates/orrery-core/src/mcp.rs && ! grep -q '^rmcp' Cargo.toml && ! grep -q '^\[graph\]' deny.toml && test ! -e crates/orrery-core/tests/live_mcp.rs && sed -n 802p $S | grep -q '^### 9.5 '

[MUTATION_SPEC]
1. Root `[workspace.dependencies]`:
     reqwest = { version = "0.13.5", default-features = false, features = ["rustls", "json", "stream"] }
     rmcp = { version = "3.9.0", default-features = false, features = ["client", "transport-child-process", "transport-streamable-http-client-reqwest", "reqwest"] }
   and replace tokio's features with `["sync", "macros", "rt", "process", "io-util", "time"]`. Crate: `reqwest.workspace = true`, `rmcp.workspace = true`, and
     [[test]]
     name = "live_mcp"
     required-features = ["live-backends"]
   Run `cargo fetch`.
2. deny.toml: append
     [graph]
     targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
3. lib.rs: variant `#[error("HTTP error")] Http(#[source] reqwest::Error)` in §5.1 order (after `Agent`).
4. mcp.rs: §9.5 `RmcpTransport` (+ its `McpTransport` impl) and `RmcpConnector`, built exactly as the `RmcpTransport::connect` bullet says. Also add check (0) from the `execute_for_role` bullet (spec line 841) as the first statement of `McpGatekeeper::execute_for_role`: `if !args.is_object()` → `Error::Security`, before the allowed-tool check.
5. tests/live_mcp.rs: `live_stdio_list_and_call`. Read `ORRERY_LIVE_MCP_CMD` and split it on whitespace into command + args (a missing var is a `panic!` with a clear message). Connect; `list_tools` is non-empty; call the first tool with `{}` and assert an `Ok` or an MCP-level error result, not a transport error.
6. Unit tests in mcp.rs:
   - `missing_http_secret_is_keychain_error_before_network`: `StreamableHttp` at `https://mcp.example.com/mcp` with a keychain pair and an empty `InMemorySecretStore` gives `Error::Keychain`.
   - `stdio_missing_command_errors`: command `"orrery-test-no-such-binary"` gives an `Err` (any variant), no panic.
   - `non_object_args_rejected_before_transport` (`#[tokio::test]`): a gatekeeper with a `MockMcpTransport` whose tool is a registered write tool and allowed for the role. Calling it with `json!(["src/lib.rs"])` and with `json!("src/lib.rs")` each gives `Error::Security`, and the mock's `calls()` is empty afterwards.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy -p orrery-core --all-targets --features live-backends --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! cargo tree --locked -i openssl-sys >/dev/null 2>&1 && grep -q '^\[graph\]$' deny.toml && grep -q 'fn live_stdio_list_and_call' crates/orrery-core/tests/live_mcp.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^mcp::tests::(missing_http_secret_is_keychain_error_before_network|stdio_missing_command_errors|non_object_args_rejected_before_transport): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 12: Inference Server Config (Attach to Existing Servers)

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. config.rs still has the old shape: `InferenceGpu` (line 844, with private repr types), `UnloadStrategy` (line 895), `InferenceConfig` (line 903). This step replaces it. Orrery attaches to model servers the operator already runs. It never sets images, ports, GPUs, memory or volumes for them.

```text
[STEP_ID]
STEP 12: config.rs — InferenceConfig to the §9.6a attach-only shape (InferenceMode, InferenceLifecycle, chat_url, validate); old GPU/unload types removed

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:922-955 (§9.6a rust block + mode/lifecycle tables; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:204-280 (§4.3: the three inference examples, used as fixtures)
  - crates/orrery-core/src/config.rs:840-930 (current InferenceGpu, UnloadStrategy, InferenceConfig)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `InferenceFile` and `ProjectConfig.inference` stay. No fixture or test mentions any server product other than llama.cpp. `Compose.file` may be absolute or relative, but fixtures use relative placeholders.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct RmcpConnector' crates/orrery-core/src/mcp.rs && grep -q 'pub enum InferenceGpu' crates/orrery-core/src/config.rs && grep -q 'pub enum UnloadStrategy' crates/orrery-core/src/config.rs && sed -n 922p $S | grep -q '^### 9.6a '

[MUTATION_SPEC]
1. config.rs: replace `InferenceConfig` with the §9.6a shape: `InferenceMode`, `InferenceLifecycle`, `chat_url`, `validate`, `default_true`, `default_startup_timeout` (300). Delete `InferenceGpu`, its private repr types, and `UnloadStrategy`.
2. Rewrite every existing inference test and TOML fixture in config.rs to the new shape. The three §4.3 examples are the fixtures.
3. Tests (in config::tests):
   - `inference_defaults`: TOML with only `inference_id` + `endpoint_url` gives `mode == Single`, `lifecycle == External`, `unload_after_stage == true` and `startup_timeout_secs == 300`.
   - `inference_spec_examples_round_trip`: the three §4.3 examples parse, validate and round-trip.
   - `inference_chat_url_trims_slash`: `"http://127.0.0.1:8080/"` gives `"http://127.0.0.1:8080/v1/chat/completions"`.
   - `inference_rejects_bad_endpoint`: `"127.0.0.1:8080"` and `"ftp://x"` are rejected.
   - `inference_rejects_empty_container_and_compose`: `Container { name: "" }` and `Compose { file: "" }` are rejected.
   - `inference_rejects_removed_fields`: TOML containing `image`, `gpu`, `host_port` or `models_volume` fails (`deny_unknown_fields`).
   - `inference_rejects_embedded_secret_in_endpoint`: an endpoint containing `sk-ABCDEFGH12345678` is rejected.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -qE 'InferenceGpu|UnloadStrategy|models_volume|keep_alive' crates/orrery-core/src/config.rs && ! grep -rqi 'ollama' crates && grep -q 'pub enum InferenceLifecycle' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(inference_defaults|inference_spec_examples_round_trip|inference_chat_url_trims_slash|inference_rejects_bad_endpoint|inference_rejects_empty_container_and_compose|inference_rejects_removed_fields|inference_rejects_embedded_secret_in_endpoint): test$')" -eq 7

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 13: Container & Inference Types, Mocks & Pure Helpers

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. Only the pure parts and mocks of §9.6. Real Docker comes in Steps 14–15.

```text
[STEP_ID]
STEP 13: docker.rs part 1 — container/inference types and traits, mocks, effective_concurrency, mounts_for, unload_request, cli_container_name, agent_io_dir

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/docker.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:844-921 (§9.6 rust block + effective_concurrency and mounts_for bullets; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:391-417 (§5.7, §5.7.1 mounts, §5.7.2 guardrails)
  - ORRERY_TECHNICAL_SPECIFICATION.md:922-955 (§9.6a: Router unload endpoint)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No DockerComposeCli, DockerInferenceControl, ResourceManager or BollardContainerBackend (Steps 14-15). No new dependency.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub enum InferenceLifecycle' crates/orrery-core/src/config.rs && test ! -e crates/orrery-core/src/docker.rs && sed -n 844p $S | grep -q '^### 9.6 '

[MUTATION_SPEC]
1. lib.rs: `pub mod docker;` (alphabetical).
2. docker.rs, from §9.6: `EnvValue`, `MountSpec`, `ContainerState`, `ExitStatus`, `ContainerSpec`, `ContainerBackend`, `ComposeRunner`, `InferenceControl`, `InferenceCall`, `MockInferenceControl`, `unload_request`, `MockCall`, `MockRunScript`, `MockContainerBackend`, `cli_container_name`, `agent_io_dir`, `effective_concurrency`, `mounts_for`.
   - `MockContainerBackend`:
     - `start` of a name that wasn't added gives the §9.6 `Error::Validation`; `start` sets `Running`;
     - `inspect` of an unknown name gives `Missing`;
     - `stop` gives `Exited { code: 0 }`; `remove` gives `Missing`;
     - `run_to_completion` sleeps the delay (if set), calls the script, feeds each output line to `on_output`, records `RunToCompletion`, and returns `ExitStatus { code }`.
   - `MockInferenceControl`: `is_running` reads the set; `start` adds and `stop` removes; `wait_ready`/`unload` record and return `Ok`. Every call is recorded.
3. Tests:
   - `concurrency_non_writer_uses_cap`: max 4 with 2 agents gives 2; max 2 with 5 agents gives 2; 0 agents gives 1.
   - `concurrency_writer_without_scopes_is_one`.
   - `concurrency_writer_disjoint_scopes`: `["src/a"]` and `["src/b"]` give the cap.
   - `concurrency_writer_overlapping_scopes_is_one`: `["src"]` vs `["src/lib.rs"]`.
   - `concurrency_writer_with_rw_cli_is_one`.
   - `mounts_local_and_cloud_empty`.
   - `mounts_cli_readonly`.
   - `mounts_cli_readwrite`.
   - `mounts_cli_none_has_no_project_mount`.
   - `mounts_never_bind_orrery_dir`: for every mount mode, no `Bind.host` equals `<root>/.orrery`.
   - `unload_request_router_only`: `Router` gives `("http://127.0.0.1:8081/models/unload", {"model": m})`; `Single` gives `None`.
   - `cli_container_name_format`.
   - `mock_start_unknown_container_fails` (`#[tokio::test]`).
   - `mock_records_calls_and_runs_script` (`#[tokio::test]`).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub mod docker;$' crates/orrery-core/src/lib.rs && ! grep -qE 'struct (ResourceManager|BollardContainerBackend|DockerComposeCli|DockerInferenceControl)' crates/orrery-core/src/docker.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^docker::tests::(concurrency_non_writer_uses_cap|concurrency_writer_without_scopes_is_one|concurrency_writer_disjoint_scopes|concurrency_writer_overlapping_scopes_is_one|concurrency_writer_with_rw_cli_is_one|mounts_local_and_cloud_empty|mounts_cli_readonly|mounts_cli_readwrite|mounts_cli_none_has_no_project_mount|mounts_never_bind_orrery_dir|unload_request_router_only|cli_container_name_format|mock_start_unknown_container_fails|mock_records_calls_and_runs_script): test$')" -eq 14

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 14: ResourceManager, Inference Control & Path Scope

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13; amended 2026-11-14 after Step 13's review.
- Step 13's `/workspace` bind exposed `<root>/.orrery` to CLI agents. A `ReadWrite` agent could then edit `.orrery/mcp/servers.toml`, whose stdio commands Orrery runs on the host, or forge checkpoints. This step masks it.
- It also adds the path-scope rules (spec §9.6a): project-scope config holds only relative paths, and compose files are resolved and checked before Orrery ever runs `docker compose`.
- `external_server_never_started` needs a `ComposeRunner` that records calls. If §9.6 doesn't define one, write a private one inside `mod tests` (a free choice).

```text
[STEP_ID]
STEP 14: docker.rs part 2 — DockerComposeCli (+ compose_args), DockerInferenceControl, ResourceManager, StageLease, SlotPermit, .orrery mask in mounts_for; config.rs — ConfigScope, resolve_compose_file, project-scope path checks

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/docker.rs
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:844-921 (§9.6: DockerComposeCli, begin_stage steps 1-5, end_stage, shutdown, mounts_for; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:922-955 (§9.6a: path scope, ConfigScope, resolve_compose_file, project-scope MCP checks, lifecycle table, readiness; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:410-417 (§5.7.1 mounts, §5.7.2)
  - ORRERY_TECHNICAL_SPECIFICATION.md:204-280 (§4.3 inference examples, used as fixtures)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No bollard (Step 15). Tests never run docker: compose argv is checked through compose_args only.
  - A resolved (absolute) path is never written to a project-scope file. No new dependency.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct MockContainerBackend' crates/orrery-core/src/docker.rs && ! grep -q 'struct ResourceManager' crates/orrery-core/src/docker.rs && ! grep -q 'enum ConfigScope' crates/orrery-core/src/config.rs && sed -n 844p $S | grep -q '^### 9.6 ' && sed -n 922p $S | grep -q '^### 9.6a ' && sed -n 939p $S | grep -q 'resolve_compose_file' && sed -n 911p $S | grep -q 'agent_io_dir/mask'

[MUTATION_SPEC]
1. docker.rs, from §9.6: `DockerComposeCli` (+ `ComposeRunner`), `DockerInferenceControl` (+ `new`, `InferenceControl`), `ResourceManager` (`new`; `begin_stage` steps 1-5; `end_stage`; `shutdown`), `StageLease` (`acquire_slot`, `concurrency`, `Drop`), `SlotPermit`.
   - `DockerComposeCli` builds its argv with a private pure fn `compose_args(file, service, op) -> Vec<String>`.
   - Each `ComposeRunner` method first rejects a relative `file` with the §9.6 `Error::Validation`, before any process starts.
2. docker.rs, `mounts_for` (spec line 911): when the project is mounted, insert `Bind { agent_io_dir.join("mask"), "/workspace/.orrery", read_only: true }` right after the `/workspace` bind. Order: `/workspace`, mask, `/orrery/in`, `/orrery/out`. `WorkspaceMount::None` gets no mask. Update Step 13's `mounts_cli_readonly` and `mounts_cli_readwrite` to expect the mask.
3. config.rs, from §9.6a (spec lines 923 and 939):
   - `pub enum ConfigScope { Project, Machine }` (derive `Debug, Clone, Copy, PartialEq, Eq`).
   - `pub fn resolve_compose_file(..)` exactly as specified. Reuse `resolve_safe_within_dir` for the Project case.
   - `McpServerConfig::validate_project_scope(&self) -> Result<()>`: a Stdio `command` containing `/` or `` → `Error::Validation { field: "transport.command", .. }`; any `args` entry where `Path::new(a).is_absolute()` → `Error::Validation { field: "transport.args", .. }`.
   - `InferenceConfig::validate_project_scope(&self) -> Result<()>`: a Compose `file` that `is_absolute()` → the same `Error::Validation` as `resolve_compose_file` (Project).
   - `ConfigStore::load_inference`: resolve each Compose `file` with `resolve_compose_file` before `upsert_by_key`. Global entries use `Machine` with `global_dir()`; `.orrery/inference.toml` and `project.toml` entries use `Project`, after `validate_project_scope`.
   - `ConfigStore::load_mcp_servers`: call `validate_project_scope` on every project-scope entry.
   - `save_inference`, `save_mcp_server` and `save_project`: call `validate_project_scope` on what they write.
4. Fix Step 3's `config_store_layering_global_first_project_replaces_by_id` (it loads the global `router` Compose entry): create `<global_dir>/docker-models/router/compose.yaml` first, then assert that the loaded `file` equals its canonical path. Don't weaken any other assertion.
5. Tests (`#[tokio::test]` where async; `MockContainerBackend` + `MockInferenceControl`; TempDir project root and TempDir global dir; inference configs from the §4.3 examples; every path is built from a TempDir):
   docker::tests:
   - `second_begin_stage_fails_while_open`.
   - `begin_after_end_succeeds`.
   - `dropped_lease_does_not_block_next_stage`.
   - `stopped_server_started_and_waited`: a server that isn't running gets `Start`, then `WaitReady`.
   - `running_server_not_started_or_stopped`: `set_running(true)` first; no `Start`; `shutdown` records no `Stop`.
   - `external_server_never_started`: `DockerInferenceControl` with the mock backend and a recording `ComposeRunner`. `External` makes no container or compose calls.
   - `same_server_next_stage_not_restarted`: two stages on `coder-14b` give one `Start` and no `Stop`.
   - `single_mode_swapped_between_stages`: stage A uses `coder-14b`, stage B uses a different `Single` server. B's `begin_stage` records `Stop(coder-14b)` before `Start(other)`.
   - `router_unloads_models_next_stage_drops`: stage A uses router model `m1`, stage B uses router model `m2`. B's `begin_stage` records `Unload(router, m1)`; `m2` isn't unloaded.
   - `unload_after_stage_false_keeps_server`: no `Stop`/`Unload` between stages.
   - `end_stage_removes_cli_containers_and_agent_io`: `Remove(cli_container_name(..))` is recorded and `.orrery/agent_io/<run_id>` is gone.
   - `unknown_inference_id_is_validation_error`.
   - `slots_limit_parallelism`: with concurrency 1, a second `acquire_slot` doesn't complete while the first permit is held (a 50 ms `tokio::time::timeout` elapses).
   - `shutdown_unloads_router_and_stops_only_started`.
   - `compose_args_shapes`:
     - up: `["compose","-f",F,"up","-d",S]`;
     - down without a service: `[...,"down"]`;
     - down with a service: `[...,"stop",S]`;
     - ps: `[...,"ps","--status","running","-q",S]`.
   - `mounts_mask_orrery_dir`: `ReadOnly` and `ReadWrite` each give exactly one mount with `container == "/workspace/.orrery"`, `read_only: true`, host `agent_io_dir/mask`. `None` gives no mount whose container starts with `/workspace`.
   - `compose_cli_rejects_relative_file`: `DockerComposeCli.up(Path::new("compose.yaml"), None)` gives `Error::Validation` with `field == "compose"`.
   config::tests:
   - `project_compose_resolves_under_orrery`: `<root>/.orrery/compose/router.yaml` exists; `resolve_compose_file("compose/router.yaml", Project, ..)` returns its canonical path.
   - `project_compose_absolute_rejected`: an absolute path (from a TempDir) with `Project` gives `Error::Validation` with `field == "lifecycle.file"`.
   - `project_compose_escape_rejected`: `"../outside.yaml"` with `Project` gives `Error::PathTraversal`.
   - `machine_compose_absolute_outside_workspace_allowed`: an absolute file in a separate TempDir with `Machine` returns its canonical path.
   - `compose_inside_workspace_rejected`: an absolute `<root>/compose.yaml` with `Machine` gives `Error::Security`; an absolute `<root>/.orrery/compose.yaml` with `Machine` is `Ok`.
   - `compose_missing_file_is_io_error`.
   - `project_stdio_command_with_separator_rejected`: saving a project-scope Stdio server with command `"bin/server"` gives `Error::Validation` with `field == "transport.command"`, and nothing is written.
   - `project_stdio_absolute_arg_rejected`: loading a project `.orrery/mcp/servers.toml` whose args hold an absolute TempDir path gives `Error::Validation` with `field == "transport.args"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub struct ResourceManager' crates/orrery-core/src/docker.rs && grep -q 'fn compose_args' crates/orrery-core/src/docker.rs && grep -q '"/workspace/.orrery"' crates/orrery-core/src/docker.rs && grep -q 'pub fn resolve_compose_file' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^docker::tests::(second_begin_stage_fails_while_open|begin_after_end_succeeds|dropped_lease_does_not_block_next_stage|stopped_server_started_and_waited|running_server_not_started_or_stopped|external_server_never_started|same_server_next_stage_not_restarted|single_mode_swapped_between_stages|router_unloads_models_next_stage_drops|unload_after_stage_false_keeps_server|end_stage_removes_cli_containers_and_agent_io|unknown_inference_id_is_validation_error|slots_limit_parallelism|shutdown_unloads_router_and_stops_only_started|compose_args_shapes|mounts_mask_orrery_dir|compose_cli_rejects_relative_file): test$')" -eq 17 && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(project_compose_resolves_under_orrery|project_compose_absolute_rejected|project_compose_escape_rejected|machine_compose_absolute_outside_workspace_allowed|compose_inside_workspace_rejected|compose_missing_file_is_io_error|project_stdio_command_with_separator_rejected|project_stdio_absolute_arg_rejected): test$')" -eq 8

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 15: Docker Backend via `bollard`

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13; amended 2026-11-14. AMD/ROCm host: no NVIDIA device requests anywhere, and model servers are never created by Orrery. `connect()` takes no socket path: bollard's local defaults honor `DOCKER_HOST`, so no path ever lands in config (spec line 906).

```text
[STEP_ID]
STEP 15: docker.rs part 3 — BollardContainerBackend (connect() with no socket argument, create_body, ContainerBackend impl), Error::Docker, live_docker tests

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/docker.rs
  - crates/orrery-core/tests/live_docker.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:844-921 (§9.6: BollardContainerBackend and the create_body bullet; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:621-644 (§8: ORRERY_LIVE_CONTAINER, ORRERY_LIVE_IMAGE)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `start` never creates a container. No device_requests, devices, port_bindings or exposed_ports. The live tests never remove ORRERY_LIVE_CONTAINER.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct ResourceManager' crates/orrery-core/src/docker.rs && ! grep -q '^bollard' Cargo.toml && test ! -e crates/orrery-core/tests/live_docker.rs && sed -n 844p $S | grep -q '^### 9.6 ' && sed -n 906p $S | grep -q 'pub fn connect() -> Result<Self>'

[MUTATION_SPEC]
1. Root: `bollard = "0.21.1"`, `futures-util = "0.3.34"`. Crate: both as `.workspace = true`, and
     [[test]]
     name = "live_docker"
     required-features = ["live-backends"]
   Run `cargo fetch`.
2. lib.rs: variant `#[error("docker error")] Docker(#[source] bollard::errors::Error)` in §5.1 order (after `Report`).
3. docker.rs: `BollardContainerBackend::connect()` (`Docker::connect_with_local_defaults()`, no arguments), `create_body` (§9.6 bullet), and `ContainerBackend for BollardContainerBackend`:
   - `inspect`: `inspect_container`. 404 gives `Missing`; `state.running` gives `Running`; `state.status == created` gives `Created`; anything else gives `Exited { code: state.exit_code }`.
   - `start`: `inspect` first. `Missing` gives the §9.6 `Error::Validation`; `Running` gives `Ok`; anything else calls `start_container`.
   - `stop` and `remove` (`force: true`): 404 gives `Ok(())`.
   - `run_to_completion` (AgentCli), in order:
     - `create_container` with `create_body`;
     - `attach_container` (stdin, stdout, stderr, stream), then `start_container`;
     - write all of `stdin` to the attach input, then shut it down, concurrently (`tokio::join!`) with reading output via `futures_util::StreamExt`, split into lines for `on_output` (amended 2026-11-14 at review: sequential order can deadlock);
     - `wait_container`, returning `ExitStatus { code: status_code }`.
   Use the `bollard::query_parameters::*OptionsBuilder` types and `bollard::models::{ContainerCreateBody, HostConfig}`.
4. tests/live_docker.rs (a missing env var is a `panic!` with a clear message):
   - `live_start_stop_existing_container`: the container named by `ORRERY_LIVE_CONTAINER` (stopped is fine). `start`; `inspect` == `Running`; `stop`; `inspect` is `Exited`. It's never removed.
   - `live_run_to_completion_echoes_stdin`: image `ORRERY_LIVE_IMAGE`, `cmd ["cat"]`, stdin `"hello\n"`. Output contains `"hello"`, exit 0, then `remove`.
5. Unit tests (pure, no Docker):
   - `create_body_maps_image_cmd_env_memory`.
   - `create_body_binds_readonly_with_ro_suffix`.
   - `create_body_sets_no_gpu_or_ports`: `device_requests`, `devices`, `port_bindings` and `exposed_ports` are all `None`.
   - `secret_env_only_in_create_body`: `format!("{spec:?}")` doesn't contain the secret value; the `create_body` env does.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy -p orrery-core --all-targets --features live-backends --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! cargo tree --locked -i openssl-sys >/dev/null 2>&1 && ! grep -qi 'nvidia' crates/orrery-core/src/docker.rs && ! grep -q 'connect_with_socket' crates/orrery-core/src/docker.rs && grep -q 'fn live_start_stop_existing_container' crates/orrery-core/tests/live_docker.rs && grep -q 'fn live_run_to_completion_echoes_stdin' crates/orrery-core/tests/live_docker.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^docker::tests::(create_body_maps_image_cmd_env_memory|create_body_binds_readonly_with_ro_suffix|create_body_sets_no_gpu_or_ports|secret_env_only_in_create_body): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 16: Prompt Builders, AgentDriver & Mock Driver

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `format_mapped_tool_name` (config.rs:1098) already builds `server__tool`. `parse_mapped_tool_name` is its inverse.

```text
[STEP_ID]
STEP 16: llm.rs part 1 — AgentInvocation, StageSummary, prompt builders, parse_mapped_tool_name, AgentDriver trait, MockAgentDriver

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:956-980 (§9.7 first rust block + system/user prompt text bullets; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:431-440 (§5.9: AgentDriver signature)
  - crates/orrery-core/src/config.rs:1098 (format_mapped_tool_name)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No HTTP or container code (Steps 17-19). Prompts never read LiveStreamHub contents.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct BollardContainerBackend' crates/orrery-core/src/docker.rs && test ! -e crates/orrery-core/src/llm.rs && sed -n 956p $S | grep -q '^### 9.7 '

[MUTATION_SPEC]
1. lib.rs: `pub mod llm;` (alphabetical).
2. llm.rs, from the first §9.7 block: `AgentInvocation` (derive `Clone`), `StageSummary`, `SUBMIT_REPORT_TOOL`, `CLI_REPORT_PATH`, `build_system_prompt`, `build_user_prompt`, `build_orchestrator_prompt` (text exactly per the prompt bullets, spec lines 978-980), `parse_mapped_tool_name`, `AgentDriver` (§5.9 signature), `MockAgentDriver`.
3. Tests:
   - `user_prompt_contains_step_allocation_prior_and_notes`.
   - `user_prompt_omits_empty_sections`.
   - `user_prompt_never_contains_stream_text`: emit an event with text `"STREAM-SENTINEL-7f3a"` to a `LiveStreamHub`, then build every prompt kind for the same step. None contains the sentinel.
   - `system_prompt_chat_backends_mention_submit_report`.
   - `system_prompt_cli_mentions_report_path_and_schema`.
   - `orchestrator_prompt_lists_stages`.
   - `mapped_name_round_trip`: `("filesystem","read_file")` → `"filesystem__read_file"` → back.
   - `mapped_name_rejects_invalid`: `"nodelimiter"`, `"__tool"`, `"server__"` and a 65-char name.
   - `mock_driver_returns_scripted_reports_in_order` (`#[tokio::test]`).
   - `mock_driver_exhausted_is_agent_error` (`#[tokio::test]`).
   - `mock_driver_tracks_max_in_flight`: two concurrent `run_turn` calls with a 50 ms delay give `max_in_flight() == 2`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub mod llm;$' crates/orrery-core/src/lib.rs && ! grep -qE 'struct (HttpChatDriver|CliContainerDriver)' crates/orrery-core/src/llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^llm::tests::(user_prompt_contains_step_allocation_prior_and_notes|user_prompt_omits_empty_sections|user_prompt_never_contains_stream_text|system_prompt_chat_backends_mention_submit_report|system_prompt_cli_mentions_report_path_and_schema|orchestrator_prompt_lists_stages|mapped_name_round_trip|mapped_name_rejects_invalid|mock_driver_returns_scripted_reports_in_order|mock_driver_exhausted_is_agent_error|mock_driver_tracks_max_in_flight): test$')" -eq 11

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 17: HTTP Chat — Request Building & Stream Parsing

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. Pure code only. No network in this step.

```text
[STEP_ID]
STEP 17: llm.rs part 2 — endpoint_for, build_chat_request, SseParser, ToolCallAccumulator and their chunk types (pure)

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:981-997 (§9.7 second rust block + endpoint_for, build_chat_request and SseParser bullets; binding. The turn-loop bullet on line 998 is Step 18)
  - crates/orrery-core/src/checkpoint.rs (report_json_schema)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No reqwest call, no HttpChatDriver.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct MockAgentDriver' crates/orrery-core/src/llm.rs && ! grep -q 'struct SseParser' crates/orrery-core/src/llm.rs && sed -n 981p $S | grep -q '^```rust'

[MUTATION_SPEC]
1. From the second §9.7 block, pure parts only: `endpoint_for`, `build_chat_request`, `ToolCallDelta`, `ChatChunk`, `SseParser`, `SseItem`, `AssembledToolCall`, `ToolCallAccumulator`.
2. Tests (inline fixture strings):
   - `endpoint_local_model_uses_chat_url`.
   - `endpoint_cloud_uses_url`.
   - `endpoint_cli_rejected`.
   - `endpoint_unknown_inference_rejected`.
   - `request_includes_tools_and_submit_report`: tool names are `server__tool`; the last tool is `submit_report` with `report_json_schema()` as its parameters; `stream: true`.
   - `sse_content_chunk`.
   - `sse_reasoning_content_and_reasoning_keys`: both spellings map to `reasoning`.
   - `sse_split_across_pushes`: one `data:` line split over two `push` calls yields one chunk.
   - `sse_done_marker`.
   - `sse_ignores_comments_and_blank_lines`: lines starting with `:`.
   - `sse_bad_json_is_json_error`.
   - `accumulator_joins_argument_fragments`: 3 deltas for index 0 give one call with the full arguments.
   - `accumulator_keeps_parallel_calls_ordered`: indices 1 and 0 come out ordered 0, 1.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'struct HttpChatDriver' crates/orrery-core/src/llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^llm::tests::(endpoint_local_model_uses_chat_url|endpoint_cloud_uses_url|endpoint_cli_rejected|endpoint_unknown_inference_rejected|request_includes_tools_and_submit_report|sse_content_chunk|sse_reasoning_content_and_reasoning_keys|sse_split_across_pushes|sse_done_marker|sse_ignores_comments_and_blank_lines|sse_bad_json_is_json_error|accumulator_joins_argument_fragments|accumulator_keeps_parallel_calls_ordered): test$')" -eq 13

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 18: HttpChatDriver Turn Loop

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13; amended 2026-11-14 after Steps 9 and 17's reviews. No new crates, so `Cargo.lock` isn't in scope. The per-response handler is a private helper (a free choice under §9.0). Its only constraint is that the unit tests can drive it with fixture `SseItem`s and no network.
- **Line-buffered events.** `LiveStreamHub::emit` redacts each event on its own and writes the log file under a std `Mutex`. One event per token delta would let a secret split across two deltas past the redactor, and would do a file write per token on the async worker. So output and reasoning are buffered per kind and emitted one line at a time (spec line 998).
- **Generated tool-call ids.** llama.cpp can stream tool calls without an `id`, and Step 17's accumulator then yields `id: ""`. Give each such call a generated id, so the `tool` reply matches its call.
- Moving `emit`'s file IO off the async worker is a `stream_log.rs` change, out of scope here and left for a later refresh.

```text
[STEP_ID]
STEP 18: llm.rs part 3 — HttpChatDriver (new, with_max_tool_calls) and its AgentDriver turn loop; live_llm test

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/llm.rs
  - crates/orrery-core/tests/live_llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:981-998 (§9.7: HttpChatDriver block and the "Turn loop" bullet on line 998; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:621-644 (§8: ORRERY_LIVE_LLM_URL, ORRERY_LIVE_LLM_MODEL)
  - crates/orrery-core/src/mcp.rs (McpGatekeeper::execute_for_role), crates/orrery-core/src/stream_log.rs (LiveStreamHub)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. A secret exists only inside a SecretValue and is fetched right before the request that uses it. Bearer auth only for CloudApi.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct SseParser' crates/orrery-core/src/llm.rs && ! grep -q 'struct HttpChatDriver' crates/orrery-core/src/llm.rs && test ! -e crates/orrery-core/tests/live_llm.rs && sed -n 998p $S | grep -q '^- Turn loop' && sed -n 998p $S | grep -q 'orrery_call_'

[MUTATION_SPEC]
1. crates/orrery-core/Cargo.toml:
     [[test]]
     name = "live_llm"
     required-features = ["live-backends"]
2. llm.rs: `HttpChatDriver` (`new`, `with_max_tool_calls`; default 50) and `AgentDriver for HttpChatDriver` per the turn-loop bullet (spec line 998). Per-response handling is a private helper that takes fixture `SseItem`s, so the tests below need no network.
   - A private line buffer per event kind (`Output`, `Reasoning`): append each delta; emit every complete line without its `'\n'`; flush the remainder at end of response, and whenever the buffer passes 4096 bytes (cut at a char boundary). Empty lines are not emitted.
   - After `ToolCallAccumulator::finish`, a call with an empty `id` gets `format!("orrery_call_{n}")`, with `n` counting calls across the turn. The same id goes in the assistant message's `tool_calls` and in the `tool` reply's `tool_call_id`.
3. tests/live_llm.rs: `live_single_turn`. A `LocalModel`-style agent pointed at `ORRERY_LIVE_LLM_URL` with model `ORRERY_LIVE_LLM_MODEL` (a missing var is a `panic!` with a message), and an empty gatekeeper. `run_turn` returns `Ok` or `Error::Report` (the model didn't comply), never `Http`/`Agent`.
4. Unit tests (fixture-driven, no network):
   - `submit_report_ends_turn`.
   - `invalid_report_reprompted_once_then_error`: the first invalid report queues a `tool` message with the error; the second gives `Error::Report`.
   - `no_tool_call_reprompted_once_then_error`.
   - `disallowed_tool_returns_error_to_model_and_emits_event`: the `tool` message starts with `"error:"`; a `ToolCall` event containing `"denied"` is in `stream.recent(..)`; the mock transport has zero calls.
   - `allowed_tool_result_returned_to_model`.
   - `tool_call_cap_enforced`: `with_max_tool_calls(2)` and 3 calls give `Error::Agent`.
   - `bearer_only_for_cloud_api`: the private header builder returns `Some("Bearer …")` for `CloudApi` (secret from `InMemorySecretStore`) and `None` for `LocalModel`.
   - `cloud_missing_secret_is_keychain_error`.
   - `output_emitted_per_line`: content deltas `"hel"`, `"lo\nwor"`, `"ld"` then end of response give exactly two `Output` events, `"hello"` and `"world"`.
   - `secret_split_across_deltas_redacted`: register `SecretValue::new("sk-SPLITTEST-0123456789")` with the hub's redactor; deltas `"key sk-SPLIT"` and `"TEST-0123456789 end\n"`. `stream.recent(..)` contains `"[REDACTED]"` and no event text contains the secret.
   - `empty_tool_call_id_gets_generated_id`: a fixture tool call with no `id` gives an assistant message whose `tool_calls[0].id` is non-empty and equals the following `tool` message's `tool_call_id`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy -p orrery-core --all-targets --features live-backends --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn live_single_turn' crates/orrery-core/tests/live_llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^llm::tests::(submit_report_ends_turn|invalid_report_reprompted_once_then_error|no_tool_call_reprompted_once_then_error|disallowed_tool_returns_error_to_model_and_emits_event|allowed_tool_result_returned_to_model|tool_call_cap_enforced|bearer_only_for_cloud_api|cloud_missing_secret_is_keychain_error|output_emitted_per_line|secret_split_across_deltas_redacted|empty_tool_call_id_gets_generated_id): test$')" -eq 11

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 19: CliContainerDriver

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13; amended 2026-11-14. `run_turn` also creates the empty `mask` dir that `mounts_for` binds over `/workspace/.orrery` (Step 14). The spec's `ContainerSpec` literal no longer lists `port`/`gpu`; those fields don't exist (§9.6).

```text
[STEP_ID]
STEP 19: llm.rs part 4 — CliContainerDriver (new) and its AgentDriver run_turn

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1000-1005 (§9.7 CliContainerDriver block + run_turn bullet; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:414-417 (§5.7.2 AgentCli guardrails and secrets)
  - crates/orrery-core/src/docker.rs (MockContainerBackend, MockRunScript, agent_io_dir, mounts_for)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Secrets reach the container only as secret env, never as plain env or in the prompt file.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct HttpChatDriver' crates/orrery-core/src/llm.rs && ! grep -q 'struct CliContainerDriver' crates/orrery-core/src/llm.rs && sed -n 1000p $S | grep -q '^```rust' && sed -n 1004p $S | grep -q 'dir/mask' && grep -q '"/workspace/.orrery"' crates/orrery-core/src/docker.rs

[MUTATION_SPEC]
1. `CliContainerDriver::new` and `AgentDriver for CliContainerDriver`, per the §9.7 `run_turn` bullet (spec line 1004). Create `dir/mask` (empty) only when `workspace_mount` isn't `None`.
2. Tests (`#[tokio::test]`; `MockContainerBackend` with a run script that writes `<host out mount>/report.toml`, found in `spec.mounts` by `container == "/orrery/out"`):
   - `valid_report_accepted`: also, `in/prompt.md` exists and the stdin length equals the prompt length.
   - `output_lines_streamed`: the script's output lines appear as `Output` events.
   - `missing_report_reprompted_then_error`: 2 `RunToCompletion` calls, then `Error::Report`.
   - `invalid_then_valid_report_accepted`: the first run writes bad TOML, the second a valid report, giving `Ok`. The second prompt contains `"Previous report was invalid"`.
   - `timeout_is_agent_error_and_removes_container`: `timeout_secs = 1` and a 2 s mock delay give `Error::Agent`, with a `Remove` call recorded.
   - `secret_env_injected_as_secret`: the recorded `secret_env == ["ANTHROPIC_API_KEY"]`, and `plain_env` doesn't include it.
   - `missing_secret_fails_before_run`: an empty secret store gives `Error::Keychain` and zero `RunToCompletion` calls.
   - `mask_dir_exists_and_empty_during_run`: `ReadWrite` agent; inside the run script, the host side of the `/workspace/.orrery` mount is an existing, empty dir.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub struct CliContainerDriver' crates/orrery-core/src/llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^llm::tests::(valid_report_accepted|output_lines_streamed|missing_report_reprompted_then_error|invalid_then_valid_report_accepted|timeout_is_agent_error_and_removes_container|secret_env_injected_as_secret|missing_secret_fails_before_run|mask_dir_exists_and_empty_during_run): test$')" -eq 8

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 20: Engine Types & Pure Rules

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `run` is a stub in this step only, and Step 21 replaces it. The stub's error text is fixed so Step 21's `POST_GATE` can prove it's gone.

```text
[STEP_ID]
STEP 20: engine.rs part 1 — DriverSet, EngineContext, RunOptions, RunOutcome, EngineEvent, DispatchKind, StageTurnRecord, check_dispatch, agents_pass_rule, WorkflowEngine (new/subscribe/operator_sender, run stub)

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/lib.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1037 (§9.8 rust block + check_dispatch and agents_pass_rule bullets; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:441-492 (§5.10 flow, pass rule, modes)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No run logic beyond the stub.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct CliContainerDriver' crates/orrery-core/src/llm.rs && test ! -e crates/orrery-core/src/engine.rs && ! grep -q '^tokio-util' Cargo.toml && sed -n 1006p $S | grep -q '^### 9.8 '

[MUTATION_SPEC]
1. Root: `tokio-util = "0.7.20"`. Crate: `tokio-util.workspace = true`. Run `cargo fetch`.
2. lib.rs: `pub mod engine;` (alphabetical).
3. engine.rs, from §9.8: `DriverSet`, `EngineContext`, `RunOptions`, `RunOutcome`, `EngineEvent`, `DispatchKind`, `StageTurnRecord`, `check_dispatch`, `agents_pass_rule`, and `WorkflowEngine` with `new`, `subscribe` and `operator_sender`. `run` has the §9.8 signature and its body is exactly `Err(Error::Engine { step_number: 0, message: "not implemented until Step 21".into() })`.
   Inside `#[cfg(test)] mod tests`: `pub(super) fn test_context(dir: &Path, stages: &[(RoleKind, &[&str])]) -> EngineContext`. It builds roles, agents (all `LocalModel`), a 3-step plan, mocks and a TempDir `.orrery/`. Steps 21-25 reuse it.
4. Tests:
   - `dispatch_forward_from_start`: `None` with target 1 gives `Forward`; target 2 is an error.
   - `dispatch_forward_next`: last 1, target 2 gives `Forward`.
   - `dispatch_retry_to_earlier_implementer`: kinds `[O, I, V]`, last 2, target 1 gives `Retry`.
   - `dispatch_to_orchestrator_rejected`.
   - `dispatch_retry_to_validator_rejected`.
   - `dispatch_out_of_range_rejected`.
   - `pass_rule_all_validators_confirmed`.
   - `pass_rule_one_denied_fails`.
   - `pass_rule_missing_validator_report_fails`.
   - `pass_rule_implementer_after_validator_fails`.
   - `pass_rule_gate_failed_fails`.
   - `pass_rule_no_validator_stage_fails`.
   - `new_rejects_config_invalid_for_run`: a stage with zero agents gives `Err`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^tokio-util = "0.7.20"$' Cargo.toml && grep -q '^pub mod engine;$' crates/orrery-core/src/lib.rs && grep -q 'fn test_context' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(dispatch_forward_from_start|dispatch_forward_next|dispatch_retry_to_earlier_implementer|dispatch_to_orchestrator_rejected|dispatch_retry_to_validator_rejected|dispatch_out_of_range_rejected|pass_rule_all_validators_confirmed|pass_rule_one_denied_fails|pass_rule_missing_validator_report_fails|pass_rule_implementer_after_validator_fails|pass_rule_gate_failed_fails|pass_rule_no_validator_stage_fails|new_rejects_config_invalid_for_run): test$')" -eq 13

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 21: Engine Core Loop, Run & Loop Modes

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13. `Agents` validation mode only. Gates, retries and the operator are Step 22. Agents in a stage run one at a time here. Concurrency is Step 24.
- Amended 2026-11-14 after Step 20's review:
  - Delete the placeholder `let _ = (&engine.ctx, &engine.operator_rx, &engine.current_lease);` in `WorkflowEngine::new`. This step uses those fields, so the line has no purpose left.
  - `test_context` hands back its mock driver only as `Arc<dyn AgentDriver>`. To script reports, build your own `Arc<MockAgentDriver>` and assign `ctx.drivers = DriverSet { chat, cli }` (the field is public).

```text
[STEP_ID]
STEP 21: engine.rs part 2 — WorkflowEngine::run core loop (orchestrator ↔ stage turns, StepPassed check, protocol violations, Run and Loop modes)

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:443-471 (§5.10.1 flow per plan step, §5.10.2 pass rule)
  - ORRERY_TECHNICAL_SPECIFICATION.md:482-485 (§5.10.5 modes)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1044 (§9.8; binding)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No gates, hooks, retries, operator waits, concurrency or cancel handling (Steps 22-25).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'not implemented until Step 21' crates/orrery-core/src/engine.rs && sed -n 443p $S | grep -q '^#### 5.10.1 '

[MUTATION_SPEC]
1. Replace the `run` stub. For each plan step:
   a. Load or create the checkpoint (`load_state` / `WorkflowCheckpoint::new`). Pick the first `Pending` step, or `start_step`. None gives `NoPendingSteps`.
   b. Mark it `InProgress` and emit `StepStatusChanged`.
   c. Round loop: `round += 1`, emit `RoundStarted`, save state. Orchestrator turn:
      - build the prompt with `build_orchestrator_prompt`, using this step's saved reports and the current notes;
      - pick the driver by backend through `DriverSet`;
      - stamp the report and `save_report` it, then emit a `ReportSaved` stream event.
   d. `Dispatch` → `check_dispatch` → stage turn:
      - `resources.begin_stage` (store the lease in `current_lease`);
      - run each allocated agent **one at a time** with `build_user_prompt`, calling `acquire_slot` per agent;
      - stamp and save each report, then `end_stage`;
      - record a `StageTurnRecord` and save state.
      An allocation whose `target_agent_id` isn't in the stage is a protocol violation.
   e. `StepPassed` → `agents_pass_rule(history, gate_ok = true)`. If it holds: `Passed`, `steps_passed_this_run += 1`, save, emit. If it fails: protocol violation.
   f. Protocol violation (bad dispatch, bad allocation, failed rule, driver `Error::Report`): add the note `"protocol violation: {msg}"` and redo the orchestrator turn. A second consecutive violation returns `Err(Error::Engine { .. })` (Step 22 changes this to awaiting the operator).
   g. Mode: `Run` stops after one `Passed` (`Completed { steps_passed: 1 }`). `Loop` continues until `cycles > 0 && steps_passed_this_run == cycles`, or until no step is `Pending`. `opts.mode`/`opts.cycles` override the project values when `Some`.
2. Tests (`test_context` + `MockAgentDriver`; stages `[Orchestrator, Implementer, Validator]`):
   - `run_mode_passes_one_step_and_stops`: step 2 is still `Pending`.
   - `loop_cycles_three_passes_three`: a 4-step plan.
   - `loop_cycles_zero_passes_all`.
   - `orchestrator_runs_between_every_stage_turn`: the stage indices of the invocations alternate `0, s, 0, s, …`.
   - `step_passed_while_validator_denied_is_violation`: the orchestrator says `StepPassed` after a `Denied`, and a note containing `"protocol violation"` reaches the next orchestrator prompt.
   - `dispatch_skipping_stage_is_violation`.
   - `checkpoint_saved_after_every_round`: `load_state` after the run shows `round` == the number of orchestrator turns.
   - `reports_stamped_with_step_hash_and_round`.
   - `stage_lease_closed_after_run`: a fresh `begin_stage` on the same `ResourceManager` succeeds.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'not implemented until Step 21' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(run_mode_passes_one_step_and_stops|loop_cycles_three_passes_three|loop_cycles_zero_passes_all|orchestrator_runs_between_every_stage_turn|step_passed_while_validator_denied_is_violation|dispatch_skipping_stage_is_violation|checkpoint_saved_after_every_round|reports_stamped_with_step_hash_and_round|stage_lease_closed_after_run): test$')" -eq 9

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 22: Engine Gates, Retries, Escalation & Operator

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13.
- Amended 2026-11-14 after Step 21's review: delete the placeholder `let _ = &self.operator_rx;` at the top of `WorkflowEngine::run`. This step reads `operator_rx`, so the line has no purpose left.

```text
[STEP_ID]
STEP 22: engine.rs part 3 — MCP gates at pass-check time, GateAndOperator mode, retries, escalation, awaiting-operator handling and stored operator decisions

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1044 (§9.8: gates, operator decisions, wait_for_operator, steps_passed_this_run bullets; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:443-471 (§5.10.1 retries, §5.10.2 pass rule)
  - crates/orrery-core/src/config.rs (effective_validation_mode)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The orchestrator can never override a validator denial.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'not implemented until Step 21' crates/orrery-core/src/engine.rs && cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -q '^engine::tests::run_mode_passes_one_step_and_stops: test$' && sed -n 1006p $S | grep -q '^### 9.8 '

[MUTATION_SPEC]
1. Gates per §9.8: role `"__orrery_gate"`, run at pass-check time, `GateResult` events. `gate_ok` now comes from the gate run; empty `gate_tools` means `true`.
2. `GateAndOperator` mode (`effective_validation_mode`): `StepPassed` → gate. Pass → awaiting operator. Fail → a note with the gate output, back to the orchestrator.
3. Retries (§5.10.1):
   - `DispatchKind::Retry` after a `Denied` report or a failed gate increments `retry_count`; `retry_count > max_step_retries` → awaiting operator.
   - `Escalate` → awaiting operator.
   - A second consecutive protocol violation → awaiting operator (this replaces Step 21's `Err`).
4. Awaiting operator, per §9.8:
   - set phase and status `AwaitingOperator`, set `awaiting_reason`, emit `AwaitingOperator`, save;
   - `wait_for_operator = true` awaits `operator_rx`; `false` returns `RunOutcome::AwaitingOperator`;
   - on entry to `run`, a stored `operator_decision` is applied and cleared;
   - decision effects are exactly as the "Operator decisions" bullet says.
5. Tests:
   - `deny_revise_confirm_passes`: this sequence ends `Passed` with `retry_count == 1`:
     - V `Denied`;
     - O dispatches to stage 1 (retry 1);
     - I `Complete`;
     - V `Confirmed`;
     - O `StepPassed`.
   - `retries_exhausted_awaits_operator`: `max_step_retries = 1` and two denials give `AwaitingOperator`, saved in the checkpoint with that status.
   - `escalate_awaits_operator_headless`: `wait_for_operator = false` gives `RunOutcome::AwaitingOperator`.
   - `stored_approve_resumes_after_escalation`: set `operator_decision = Approve` in the checkpoint and run again. An orchestrator turn runs and `retry_count == 0`.
   - `gate_failure_blocks_pass`: `MockMcpTransport::with_failing_tool`. `StepPassed` doesn't pass, and the next orchestrator prompt has a gate note.
   - `gate_and_operator_approve_passes`: 2-stage workflow `[O, I]`, `wait_for_operator = true`. Sending `Approve` on `operator_sender()` gives `Passed`.
   - `gate_and_operator_reject_feeds_feedback`: `Reject { feedback: "fix the README" }`. The next orchestrator prompt contains it and `retry_count == 1`.
   - `protocol_violation_twice_awaits_operator`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '__orrery_gate' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(deny_revise_confirm_passes|retries_exhausted_awaits_operator|escalate_awaits_operator_headless|stored_approve_resumes_after_escalation|gate_failure_blocks_pass|gate_and_operator_approve_passes|gate_and_operator_reject_feeds_feedback|protocol_violation_twice_awaits_operator): test$')" -eq 8

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 23: Engine Lifecycle Hooks

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. Step 25 calls `on_halt` on cancel. This step only provides it.

```text
[STEP_ID]
STEP 23: engine.rs part 4 — lifecycle hooks (step_start, step_retry, step_pass, halt) with Halt/Warn failure handling and redacted, truncated HookResult

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:477-481 (§5.10.4 hooks and the args object)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1044 (§9.8 hooks bullet; binding)
  - crates/orrery-core/src/mcp.rs (McpGatekeeper::is_tool_allowed)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The hook role may call only the configured hook tools.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '__orrery_gate' crates/orrery-core/src/engine.rs && ! grep -q '__orrery_hooks' crates/orrery-core/src/engine.rs && sed -n 477p $S | grep -q '^#### 5.10.4 '

[MUTATION_SPEC]
1. Hooks per §5.10.4 and §9.8:
   - `on_step_start` runs before the first round of a step;
   - `on_step_retry` runs before each retry dispatch;
   - `on_step_pass` runs after `Passed`;
   - `on_halt` runs on entering `AwaitingOperator`, and on cancel (Step 25 calls it).
   Each hook runs as role `"__orrery_hooks"` with the args object from §5.10.4, and the phase is `Hook` while it runs.
2. Store a `HookResult`: output redacted through `stream.redactor()`, then truncated to 4096 bytes at a char boundary.
3. On failure: `Halt` → awaiting operator with reason `"hook {event} failed"`. `Warn` → a `SystemNotice` event, then continue.
4. Tests:
   - `hook_args_contain_orrery_object`: the mock call args have all 6 keys and `event == "step_start"`.
   - `step_retry_hook_runs_before_each_retry`: two retries give two `on_step_retry` calls, each before the retried stage's invocation.
   - `step_pass_hook_runs_after_pass`.
   - `halt_hook_failure_awaits_operator`.
   - `warn_hook_failure_continues_with_notice`.
   - `hook_role_can_call_only_hook_tools`: a hook configured as `git:commit`, while the gatekeeper also has `git:push`. Build the hook role and assert `McpGatekeeper::is_tool_allowed(&role, "git", "push") == false`.
   - `hook_output_redacted_and_truncated`: mock result text with a registered secret and 5,000 chars gives stored output with `[REDACTED]` and ≤ 4096 bytes.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '__orrery_hooks' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(hook_args_contain_orrery_object|step_retry_hook_runs_before_each_retry|step_pass_hook_runs_after_pass|halt_hook_failure_awaits_operator|warn_hook_failure_continues_with_notice|hook_role_can_call_only_hook_tools|hook_output_redacted_and_truncated): test$')" -eq 7

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 24: Engine Stage Concurrency & Validator Broadcast

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `futures-util` is already a dependency (Step 15). Agents from different stages are never in flight at the same time (`Orrery_Initial.md` Decisions → Concurrency).
- Amended 2026-11-14 after Step 21's review: `run_stage_turn` awaits `acquire_slot()` while holding the `current_lease` mutex. Once agents run in parallel, every permit request would queue behind that lock. Take what the agents need out of the lease (or keep the lease in a local and store it in `current_lease` only for interrupt handling) so no `.await` on a permit happens under the mutex.

```text
[STEP_ID]
STEP 24: engine.rs part 5 — concurrent agents within a stage (lease-bounded join_all), per-agent write scopes, validator broadcast

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:472-476 (§5.10.3 concurrency within a stage)
  - ORRERY_TECHNICAL_SPECIFICATION.md:910-921 (§9.6 effective_concurrency bullet)
  - crates/orrery-core/src/config.rs (RoleConfig::is_writer), crates/orrery-core/src/mcp.rs (WriteScope::new, WriteScope::anywhere)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No manifest change. A stage turn completes (end_stage) before the next orchestrator turn starts.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '__orrery_hooks' crates/orrery-core/src/engine.rs && ! grep -q 'join_all' crates/orrery-core/src/engine.rs && grep -q '^futures-util.workspace = true$' crates/orrery-core/Cargo.toml && sed -n 472p $S | grep -q '^#### 5.10.3 '

[MUTATION_SPEC]
1. A stage turn runs its agents concurrently: one future per agent, each doing `acquire_slot().await` then `run_turn`, joined with `futures_util::future::join_all`. The lease sets the concurrency (`effective_concurrency`). `writer` = `RoleConfig::is_writer(&servers, &stage_agents)`.
2. Each agent's write scope is `WriteScope::new(allocation.write_scope)`, or `WriteScope::anywhere()` when empty.
3. Validator broadcast (§5.10.3): in a `Validator` stage, every agent in the stage runs. An agent with its own allocation uses it; the others get the first allocation's instructions. `StageTurnRecord.expected_agents` = all the stage's agents.
4. Tests:
   - `non_writer_stage_runs_in_parallel`: 2 implementers, `max_concurrent_agents = 2` and a 50 ms mock delay give `max_in_flight() == 2`.
   - `writer_stage_serialized_without_scopes`: `max_in_flight() == 1`.
   - `writer_stage_parallel_with_disjoint_scopes`: `2`.
   - `writer_stage_serialized_with_overlapping_scopes`: `src` and `src/lib.rs` give `1`.
   - `validator_broadcast_runs_every_validator`: 2 validators and 1 allocation. Both are invoked with the same instructions.
   - `pass_requires_every_broadcast_validator`: one of two validators `Denied` means the step doesn't pass.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'join_all' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(non_writer_stage_runs_in_parallel|writer_stage_serialized_without_scopes|writer_stage_parallel_with_disjoint_scopes|writer_stage_serialized_with_overlapping_scopes|validator_broadcast_runs_every_validator|pass_requires_every_broadcast_validator): test$')" -eq 6

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 25: Engine Interrupt, Resume & Plan Reconcile

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13.
- Amended 2026-11-14 after Steps 8 and 21's reviews:
  - **Resume an in-progress step.** Step 21's `pick_next_step_index` takes the first `Pending` step. A step left `InProgress` (or `AwaitingOperator`) by an interrupted run is skipped and stays that way forever. Resume must continue that step first.
  - **`changed_steps` is 0-based.** `ReconcileOutcome::Updated.changed_steps` holds plan indices, not step numbers. Convert with `+ 1` (or look up `plan.steps[i].number`) wherever a step number is shown or compared.
  - **Removing trailing steps.** When the plan shrinks, `reconcile` clamps `current_step_index` to `new_len - 1`, which can point at a `Passed` step. Resume must not continue a `Passed` step; fall through to the next `Pending` one.
- Amended 2026-11-15 after Step 24's review:
  - **Drop the stage turn before ending its lease.** `current_lease` holds an `Arc<StageLease>`, and a running `run_stage_turn` holds its own clone. Its end-of-turn guard returns `Error::Engine` ("stage lease still shared") when `Arc::try_unwrap` fails. So the cancel path must let the stage-turn future (and every clone it holds) drop first, then take `current_lease` and `end_stage` it. Race the whole `run_stage_turn(..)` call in `run` with `cancel.cancelled()`, not the `join_all` inside it. Otherwise Ctrl+C mid-stage returns that error instead of `Interrupted`.
  - A fresh `begin_stage` succeeding after a cancel does not prove `end_stage` ran: `StageLease`'s `Drop` also clears the open flag. The new test below checks the recorded `Remove` call instead.

```text
[STEP_ID]
STEP 25: engine.rs part 6 — cancel (lease closed, checkpoint saved, on_halt), resume of an interrupted round, plan reconcile on start, start_step

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:486-492 (§5.10.6 interrupt and resume)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1044 (§9.8 Cancel bullet; binding)
  - crates/orrery-core/src/checkpoint.rs (reconcile, delete_reports)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Cancel never leaves a stage lease open.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'join_all' crates/orrery-core/src/engine.rs && ! grep -q 'cancelled()' crates/orrery-core/src/engine.rs && sed -n 486p $S | grep -q '^#### 5.10.6 '

[MUTATION_SPEC]
1. Cancel, per the §9.8 "Cancel" bullet: race every await in `run` with `cancel.cancelled()` (`tokio::select!`). On cancel:
   - end the stored lease;
   - save the checkpoint (`InProgress`, phase `OrchestratorTurn`, same `round`);
   - run `on_halt`;
   - return `RunOutcome::Interrupted`.
2. Resume: a step found `InProgress` at start → `delete_reports(step, round)`, then restart that round from its orchestrator turn. Don't increment `round` for it.
3. On start, reconcile an existing checkpoint against the plan (`reconcile`). An `Err` is returned before any agent runs.
4. `start_step = Some(n)`: step `n` must be `Pending` (otherwise `Error::Engine { step_number: n, .. }`), and it becomes the current step.
5. Tests:
   - `cancel_mid_stage_saves_checkpoint_and_closes_lease`: a 500 ms mock delay, cancelled after 50 ms, gives `Interrupted`. The checkpoint step is `InProgress`, and a new `begin_stage` succeeds.
   - `cancel_runs_halt_hook`.
   - `resume_restarts_interrupted_round_orchestrator`: after the cancel above, `run` again. The first invocation is the orchestrator with the same `round`, and that round's stale reports are gone.
   - `edited_passed_step_blocks_start`: pass step 1, edit its body, then `run` a new engine. It gives `Err(Error::Engine { step_number: 1, .. })` with zero driver invocations.
   - `appended_step_accepted_on_start`.
   - `start_step_must_be_pending`.
   - `cancel_mid_cli_stage_removes_container`: make the stage-1 agent `AgentCli` (`ctx.agents` edit), and replace `ctx.resources` with a `ResourceManager` built on an `Arc<MockContainerBackend>` the test keeps. The CLI driver is a `MockAgentDriver` with a 500 ms delay; cancel after 50 ms. Gives `Interrupted`, and the backend's `calls()` contain `MockCall::Remove(cli_container_name(&run_id, agent_id))`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'cancelled()' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(cancel_mid_stage_saves_checkpoint_and_closes_lease|cancel_runs_halt_hook|resume_restarts_interrupted_round_orchestrator|edited_passed_step_blocks_start|appended_step_accepted_on_start|start_step_must_be_pending|cancel_mid_cli_stage_removes_container): test$')" -eq 7

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 26: Engine Assembly (`Backends`, `build_engine`)

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13; amended 2026-11-14. `Backends::real` is never called in tests (it touches the keychain and Docker). Tests use mocks.
- The amendment adds stdio command resolution to `mcp.rs` (spec line 842), because `Backends::real` is where `RmcpConnector` gets the workspace root and `PATH`. `RmcpConnector` resolves the command before spawning, skips relative `PATH` entries (so a `.` in `PATH` can't pick a workspace binary), refuses binaries inside the agent-writable workspace, and starts the child with `cwd` = the workspace root.
- MCP child stderr is still inherited. Piping it into the live stream is a later refresh.

```text
[STEP_ID]
STEP 26: engine.rs part 7 — Backends (+ Backends::real) and build_engine wiring config, plan, redactor, stream hub, gatekeeper, resources and checkpoints; mcp.rs — resolve_stdio_command, RmcpTransport::connect_in, RmcpConnector { project_root, search_path }

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
  - crates/orrery-core/src/mcp.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1044 (§9.8 Backends, build_engine; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:836-842 (§9.5 RmcpTransport::connect_in, RmcpConnector fields, resolve_stdio_command; binding)
  - crates/orrery-core/src/config.rs (ConfigStore::load_*, resolve_safe_path, load_role_rules)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Tests never call Backends::real. `RmcpTransport::connect`'s signature and behavior stay as they are (live_mcp.rs still calls it). No new dependency.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'cancelled()' crates/orrery-core/src/engine.rs && ! grep -q 'pub struct Backends' crates/orrery-core/src/engine.rs && ! grep -q 'fn resolve_stdio_command' crates/orrery-core/src/mcp.rs && sed -n 1006p $S | grep -q '^### 9.8 ' && sed -n 842p $S | grep -q 'resolve_stdio_command'

[MUTATION_SPEC]
1. `Backends` and `Backends::real` (§9.8). `real` takes `project_root`, builds a `ConfigStore` itself (the inference list comes from `ConfigStore::load_inference`), and builds:
   - `OsKeyringStore::new()?`;
   - `BollardContainerBackend::connect()?`;
   - `DockerInferenceControl::new(containers.clone(), Arc::new(DockerComposeCli))`;
   - `RmcpConnector { secrets, project_root, search_path: std::env::var_os("PATH") }`;
   - `DriverSet { chat: HttpChatDriver::new(..), cli: CliContainerDriver::new(..) }`.
2. `build_engine(store, backends)`, in this order:
   a. Load the project, roles (+ `load_role_rules` for each), agents, MCP servers and inference.
   b. `ImplementationPlan::from_file(resolve_safe_path(plan_path), step_marker)`.
   c. A `SecretRedactor` with every secret the run will use registered: each agent keychain pair, and each HTTP MCP pair that `secrets.get` returns `Ok` for.
   d. `LiveStreamHub::new(<.orrery>/logs, Default, redactor)`.
   e. A gatekeeper with every server `connector.connect`ed and `refresh_catalog` called.
   f. `ResourceManager::new(..)` and `CheckpointManager::new(<.orrery>/checkpoints)`.
   g. `WorkflowEngine::new(ctx)`.
3. mcp.rs (spec lines 836-842):
   - `pub fn resolve_stdio_command(..)` exactly as specified. The exec-bit check is `#[cfg(unix)]` (`std::os::unix::fs::PermissionsExt`, `mode() & 0o111 != 0`); elsewhere a regular file is enough.
   - `RmcpTransport::connect_in(server, secrets, workdir)`: today's `connect` body plus `.current_dir(workdir)` on the stdio `Command`. `connect` becomes a one-line call to `connect_in(.., Path::new("."))`.
   - `RmcpConnector` gets `project_root` and `search_path`. For `Stdio`, its `connect` clones the server, swaps in `command` = `resolve_stdio_command(..)` (a non-UTF-8 result → `Error::Validation`), then calls `connect_in(&clone, .., &self.project_root)`. `StreamableHttp` is unchanged. Update the existing `RmcpConnector { secrets }` test literal.
4. Tests (a TempDir project from `ConfigStore::init_project`, a 2-step plan file, `MockMcpConnector`, mocks for the rest):
   - `build_engine_from_initialized_project`: after assigning one agent per stage, `build_engine` succeeds, and `run` with scripted mocks passes step 1.
   - `build_engine_registers_secrets_with_redactor`: a `CloudApi` agent's secret emitted in a stream event is stored as `[REDACTED]`.
   - `build_engine_missing_plan_is_io_error`.
   - `build_engine_unknown_mcp_server_errors`: a role allows a server the connector doesn't know, giving `Err`.
   mcp::tests (every path built from a TempDir; search paths built with `std::env::join_paths`):
   - `resolve_stdio_bare_name_on_path`: an executable `<tmp>/bin/fake-mcp`; search path `[<tmp>/bin]` returns its canonical path.
   - `resolve_stdio_only_relative_entries_not_found`: search path `["bin", ""]` gives `Error::Validation` with `field == "transport.command"`.
   - `resolve_stdio_rejects_workspace_binary`: an executable `<root>/bin/fake-mcp` with search path `[<root>/bin]` gives `Error::Security`; one at `<root>/.orrery/bin/fake-mcp` is `Ok`.
   - `resolve_stdio_absolute_requires_executable` (`#[cfg(unix)]`): an absolute non-executable file gives `Error::Validation`; after `chmod 0o755` it's `Ok`.
   - `resolve_stdio_relative_with_separator_rejected`: `"bin/fake-mcp"` gives `Error::Validation`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub struct Backends' crates/orrery-core/src/engine.rs && grep -q 'pub async fn build_engine\|pub fn build_engine' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(build_engine_from_initialized_project|build_engine_registers_secrets_with_redactor|build_engine_missing_plan_is_io_error|build_engine_unknown_mcp_server_errors): test$')" -eq 4 && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^mcp::tests::(resolve_stdio_bare_name_on_path|resolve_stdio_only_relative_entries_not_found|resolve_stdio_rejects_workspace_binary|resolve_stdio_absolute_requires_executable|resolve_stdio_relative_with_separator_rejected): test$')" -eq 5

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase B — `orrery-tui` (Steps 27–32)

### STEP 27: TUI Crate, Theme & Banner

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. **Changed from the 2026-11-12 block:** `tokio-util` is added to the TUI crate's dependencies here. Step 32's `EngineController` holds a `CancellationToken`, and Step 32 can't edit this manifest.

```text
[STEP_ID]
STEP 27: orrery-tui crate (workspace member, manifest) + theme.rs (palette, BANNER, COMPACT_HEADER, banner_line, block styles)

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-tui/Cargo.toml
  - crates/orrery-tui/src/lib.rs
  - crates/orrery-tui/src/theme.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:521-558 (§6.3 palette table, full banner block, compact header block; binding)
  - crates/orrery-core/Cargo.toml (workspace package fields to mirror)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. lib.rs has `pub mod theme;` only. No rendering code.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct Backends' crates/orrery-core/src/engine.rs && test ! -e crates/orrery-tui && ! grep -q '^ratatui' Cargo.toml && sed -n 521p $S | grep -q '^### 6.3 ' && sed -n 537p $S | grep -q '^```text'

[MUTATION_SPEC]
1. Root Cargo.toml: add `"crates/orrery-tui"` to `members`. `[workspace.dependencies]`: `ratatui = "0.30.2"`, `crossterm = { version = "0.29.0", features = ["event-stream"] }`, `orrery-core = { path = "crates/orrery-core" }`.
2. crates/orrery-tui/Cargo.toml:
   - `name = "orrery-tui"`, with the workspace package fields as in orrery-core;
   - `[dependencies]`: `orrery-core`, `ratatui`, `crossterm`, `tokio`, `tokio-util`, `futures-util` (all `.workspace = true`);
   - `[dev-dependencies]`: `tempfile.workspace = true`;
   - `[lints] workspace = true`.
   Run `cargo fetch`.
3. lib.rs: `pub mod theme;` only.
4. theme.rs per §6.3:
   - the 7 colour constants as `ratatui::style::Color::Rgb(..)` from the table;
   - `BANNER: [&str; 9]`, copied byte for byte from the spec's full-banner text block as raw strings (`r"..."`);
   - `COMPACT_HEADER` from the compact block;
   - `banner_line(row)`;
   - `base_block`, `focused_block`, `alert_block`, `muted`, `highlight`.
5. Tests (theme::tests):
   - `palette_rgb_values`: each constant equals its §6.3 RGB.
   - `banner_shape`: 9 rows, ASCII only, each ≤ 72 chars.
   - `banner_matches_spec_block`: read `include_str!("../../../ORRERY_TECHNICAL_SPECIFICATION.md")` and find the line starting `Full banner (9 rows`. Then find the next line that is exactly three backticks followed by `text`. The 9 lines after it equal `BANNER` exactly.
   - `banner_letter_cell_is_primary`: row 3, column 15 (`_`, 1-based) is `AMBER_PRIMARY`.
   - `banner_planet_cell_is_bright`: row 5, column 3 (`@`) is `AMBER_BRIGHT`.
   - `banner_orbit_cell_is_dim`: row 2, column 13 (`~`, inside the letter columns but on an orbit row) is `AMBER_DIM`.
   - `banner_star_rows_are_dark`: in rows 1 and 9, every span is `AMBER_DARK`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '"crates/orrery-tui"' Cargo.toml && grep -q '^tokio-util.workspace = true$' crates/orrery-tui/Cargo.toml && test "$(grep -c '^pub mod' crates/orrery-tui/src/lib.rs)" -eq 1 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^theme::tests::(palette_rgb_values|banner_shape|banner_matches_spec_block|banner_letter_cell_is_primary|banner_planet_cell_is_bright|banner_orbit_cell_is_dim|banner_star_rows_are_dark): test$')" -eq 7

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 28: TUI Layout & Hit Testing

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13.

```text
[STEP_ID]
STEP 28: layout.rs — UiLayoutRegions::compute, menu item rects, stage canvas columns and slots, hit testing (no rendering)

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/lib.rs
  - crates/orrery-tui/src/layout.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1045-1064 (§9.9 layout block + too_small, menu items, canvas bullets; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:495-511 (§6.1 vertical split: Length/Fill; main_workspace Percentage(25) / Fill(1))
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No widget rendering.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-tui/src/theme.rs && test ! -e crates/orrery-tui/src/layout.rs && sed -n 1045p $S | grep -q '^### 9.9 ' && sed -n 495p $S | grep -q '^### 6.1 '

[MUTATION_SPEC]
1. lib.rs: `pub mod layout;`.
2. layout.rs: the §9.9 layout block and its bullets, plus §6.1.
3. Tests (layout::tests):
   - `too_small_below_80x24`: 79×24 and 80×23 give `too_small` and empty item lists.
   - `rows_cover_height_80x24`: header 3, top 3, main 9, live 6, footer 3.
   - `rows_cover_height_at_120x40_and_200x60`: the five heights sum to the area height, and the header is 11.
   - `header_switches_at_40`: height 39 gives 3; height 40 gives 11.
   - `regions_do_not_overlap`: at 80×24, 120×40 and 200×60, the five vertical rects are pairwise disjoint, and the steps panel and canvas are disjoint.
   - `fifteen_stages_scroll_horizontally`: at 80×24 with 15 stages, visible columns × 18 ≤ the canvas inner width, and the first column index == `stage_scroll`.
   - `slots_scroll_vertically`: at 80×24, the column slot count < 15, and the first slot index == `slot_scroll`.
   - `hit_test_corners`: for every rect in `top_items`, in `footer_items`, each column header and each slot, all four corner cells return that region.
   - `hit_test_outside_is_none`: a border cell returns `None`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub mod layout;$' crates/orrery-tui/src/lib.rs && ! grep -q 'Frame' crates/orrery-tui/src/layout.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^layout::tests::(too_small_below_80x24|rows_cover_height_80x24|rows_cover_height_at_120x40_and_200x60|header_switches_at_40|regions_do_not_overlap|fifteen_stages_scroll_horizontally|slots_scroll_vertically|hit_test_corners|hit_test_outside_is_none): test$')" -eq 9

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 29: TUI App State & Keyboard Input

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13. `handle_mouse_event` returns `Vec::new()` in this step only (Step 30 fills it in). `other_modifier_chords_do_nothing` compares the `App` against a clone. If §9.9 doesn't derive `Clone, PartialEq` on `App`, add them (a free choice under §9.0).

```text
[STEP_ID]
STEP 29: app.rs — App, Focus, modals, AppAction, handle_key_event (key table rows 1-11), apply_engine_event, push_stream_event, unlocked_slots, approve_visible

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/lib.rs
  - crates/orrery-tui/src/app.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1065-1103 (§9.9 app block + key table rows 1-11 + config-edit rule; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:559-573 (§6.4 input)
  - crates/orrery-core/src/config.rs (ProjectConfig::set_stage_count, set_stage_role, assign_agent, remove_agent, clear_stage)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No modifier chord other than Ctrl+C produces an action. Config edits go through the ProjectConfig methods only.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-tui/src/layout.rs && test ! -e crates/orrery-tui/src/app.rs && sed -n 1091p $S | grep -q '^Key table'

[MUTATION_SPEC]
1. lib.rs: `pub mod app;`.
2. app.rs, the §9.9 app block:
   - types, `App::new`;
   - `handle_key_event`, applying key table rows 1-11 in order;
   - `apply_engine_event`, `push_stream_event`, `unlocked_slots`, `approve_visible`;
   - `handle_mouse_event` returns `Vec::new()` (the allowed stub).
3. Tests (app::tests; `key(code)` and `ctrl(c)` helpers):
   - `ctrl_c_interrupts_only_when_running`.
   - `other_modifier_chords_do_nothing`: `Ctrl+Q`, `Ctrl+S`, `Alt+X` and `Ctrl+Tab` give no actions, and `App` is unchanged (compared against a clone).
   - `tab_and_backtab_cycle_focus`.
   - `q_quits_when_idle_confirms_when_running`.
   - `quit_confirm_enter_interrupts_and_quits`.
   - `question_mark_opens_help_esc_closes`.
   - `m_toggles_mouse_capture`.
   - `text_entry_keeps_q_and_m_as_text`: in the Ingest modal, typing `q`, `m` and `?` appends them.
   - `ingest_enter_emits_path`.
   - `mode_toggle_emits_set_mode`.
   - `cycles_plus_minus_and_dimmed_in_run`.
   - `stage_count_clamps_2_to_15`.
   - `role_selection_unlocks_first_slot`.
   - `orchestrator_has_one_slot`.
   - `other_roles_unlock_up_to_15`.
   - `delete_unassigns_agent`.
   - `invalid_edit_sets_status_message_no_action`: for example, setting the orchestrator role on stage 2.
   - `live_stream_scroll_and_end_follows`.
   - `approve_hidden_unless_gate_and_operator`.
   - `engine_events_update_status`: an `AwaitingOperator` event gives `status == AwaitingOperator`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub mod app;$' crates/orrery-tui/src/lib.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^app::tests::(ctrl_c_interrupts_only_when_running|other_modifier_chords_do_nothing|tab_and_backtab_cycle_focus|q_quits_when_idle_confirms_when_running|quit_confirm_enter_interrupts_and_quits|question_mark_opens_help_esc_closes|m_toggles_mouse_capture|text_entry_keeps_q_and_m_as_text|ingest_enter_emits_path|mode_toggle_emits_set_mode|cycles_plus_minus_and_dimmed_in_run|stage_count_clamps_2_to_15|role_selection_unlocks_first_slot|orchestrator_has_one_slot|other_roles_unlock_up_to_15|delete_unassigns_agent|invalid_edit_sets_status_message_no_action|live_stream_scroll_and_end_follows|approve_hidden_unless_gate_and_operator|engine_events_update_status): test$')" -eq 20

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 30: TUI Mouse Input

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13.
- Amended 2026-11-15 after Step 29's review:
  - **Make clicks on the Stages and Cycles controls work.** A click acts as `Enter`, but Step 29 gives `Enter` no meaning on `CyclesControl`, `StagesMinus` or `StagesPlus` (only the `+`/`-` keys work there). Make `Enter`/`Space` on `StagesMinus` act as `-` and on `StagesPlus` as `+` (same clamping and no-action-if-unchanged rule). Leave `Enter` on `CyclesControl` as nothing; it keeps the `+`/`-` keys.
  - **Keep the selection visible.** `App` has no layout, so Step 29 assumes 3 visible stage columns on `Right` and never moves `slot_scroll` on `Down`. Add `pub fn keep_selection_visible(&mut self, regions: &UiLayoutRegions)`: from `regions.stage_columns` (visible count and the visible slot indices), adjust `stage_scroll` so `stage_selected` is in a visible column, and `slot_scroll` so `slot_selected` (when > 0) is a visible slot. Remove the hardcoded `stage_scroll + 3` from `handle_stage_canvas_key`. Step 32's bridge calls the new method after every `compute`.
  - In `apply_selector_choice`, drop the `.or_else(..slot - 1..)` fallback when removing an agent; "(remove)" is only offered on an occupied slot.
  - Tests: `click_stage_plus_minus_changes_count` and `keep_selection_visible_scrolls_slots_and_stages`.

```text
[STEP_ID]
STEP 30: app.rs — handle_mouse_event (click acts as Enter on the hit region; wheel scrolls; ignored when mouse capture is off)

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/app.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1104 (§9.9 "Mouse" line; binding)
  - crates/orrery-tui/src/layout.rs (UiLayoutRegions::compute, region_at)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Approve/Reject clicks do nothing unless approve_visible().

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'fn handle_mouse_event' crates/orrery-tui/src/app.rs && sed -n 1104p $S | grep -q '^Mouse:'

[MUTATION_SPEC]
1. Replace the `handle_mouse_event` stub per the §9.9 "Mouse" line:
   - ignored when `!mouse_capture`;
   - left-down → `region_at` → set focus, with the same effect as `Enter` on that item;
   - wheel over `StepsList`, `LiveStream` or a stage column → `Up`/`Down` there.
2. Tests (app::tests; layout from `UiLayoutRegions::compute` at 120×40):
   - `click_run_emits_run`.
   - `click_ingest_opens_modal`.
   - `click_stage_header_opens_role_selector`.
   - `click_locked_slot_does_nothing`.
   - `wheel_scrolls_live_stream`.
   - `mouse_ignored_when_capture_off`.
   - `click_hidden_approve_does_nothing`.
   - `click_stage_plus_minus_changes_count`: with 3 stages, clicking `StagesPlus` gives `[SetStageCount(4)]`, then clicking `StagesMinus` gives `[SetStageCount(3)]`.
   - `keep_selection_visible_scrolls_slots_and_stages`: at 80×24, a stage with 8 agents and `slot_selected = 8` gives a `slot_scroll` that makes slot index 7 visible; at 200×60 with 10 stages and `stage_selected = 5` from `stage_scroll = 0`, `stage_scroll` stays 0 (all fit).
3. The scope-note items above (Enter on Stages ±, `keep_selection_visible`, the removed fallback).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^app::tests::(click_run_emits_run|click_ingest_opens_modal|click_stage_header_opens_role_selector|click_locked_slot_does_nothing|wheel_scrolls_live_stream|mouse_ignored_when_capture_off|click_hidden_approve_does_nothing|click_stage_plus_minus_changes_count|keep_selection_visible_scrolls_slots_and_stages): test$')" -eq 9

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 31: TUI Widgets & Rendering

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13.

```text
[STEP_ID]
STEP 31: widgets.rs — render(frame, app, regions): every §6.2 widget and modal inside its region, theme styles only

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/lib.rs
  - crates/orrery-tui/src/widgets.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1106-1108 (§9.9 render signature; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:512-520 (§6.2 widgets, badges, labels)
  - crates/orrery-tui/src/theme.rs (the only source of styles)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No colour literal outside theme.rs. Every cell background is BG_BLACK, or PANEL_BG inside modals.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -q '^app::tests::click_run_emits_run: test$' && test ! -e crates/orrery-tui/src/widgets.rs && sed -n 512p $S | grep -q '^### 6.2 '

[MUTATION_SPEC]
1. lib.rs: `pub mod widgets;`.
2. widgets.rs: `render(frame, app, regions)`:
   - draws every widget and modal of §6.2, each strictly inside its `UiLayoutRegions` rect, using styles from theme.rs only;
   - `too_small` → only the centered notice `"Terminal too small (need 80×24)"`;
   - modals are centered at 60% × 50% of the area (min 40×8) and cleared with `ratatui::widgets::Clear` first.
3. Tests (widgets::tests; `ratatui::backend::TestBackend` + `Terminal::draw`):
   - `banner_at_80x40_compact_at_80x24`: the buffer contains banner row 3 at height 40, and the `COMPACT_HEADER` text at height 24.
   - `too_small_notice_at_79x24`.
   - `status_badges`: each `RunStatus` renders its §6.2 label.
   - `top_menu_labels`: `[Ingest]`, `[Mode: RUN]`, `[Stages:`.
   - `steps_panel_badges`: `[ ]`, `[>]`, `[✓]`, `[?]`, `[!]`.
   - `slot_backend_tags`: `LM`, `CLI`, `API`.
   - `locked_slots_use_amber_dark`: the fg of a locked slot cell == `AMBER_DARK`.
   - `denial_text_uses_amber_alert`.
   - `approve_reject_only_when_awaiting_gate_and_operator`.
   - `every_cell_has_black_or_panel_background`: scan the whole buffer.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub mod widgets;$' crates/orrery-tui/src/lib.rs && ! grep -q 'Color::Rgb' crates/orrery-tui/src/widgets.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^widgets::tests::(banner_at_80x40_compact_at_80x24|too_small_notice_at_79x24|status_badges|top_menu_labels|steps_panel_badges|slot_backend_tags|locked_slots_use_amber_dark|denial_text_uses_amber_alert|approve_reject_only_when_awaiting_gate_and_operator|every_cell_has_black_or_panel_background): test$')" -eq 10

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 32: TUI ↔ Engine Bridge & `run_tui`

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. `tokio-util` is in the TUI manifest since Step 27, for `CancellationToken`.
- Amended 2026-11-15 after Step 29's review: after every `UiLayoutRegions::compute`, call `app.keep_selection_visible(&regions)` (added in Step 30), then recompute if `stage_scroll` or `slot_scroll` changed, so the drawn frame matches the selection.
- Amended 2026-11-15 after Step 31's review: two fixes that need `app.rs` and `widgets.rs` move here. (1) The steps list scrolled from `step_selected` at draw time while a click mapped the clicked row straight to a step index, so clicks picked the wrong step once the list scrolled; `App` now stores `step_scroll`. (2) Settings showed `gate_tools` because `App` had no MCP server list; `App` now holds `mcp_servers`, filled by `run_tui`.
- Amended 2026-11-15 during Step 32's correction: `WorkflowEngine` had no way to reach its `LiveStreamHub` (it lives in the private `ctx`), so `engine.rs` joins ALLOWED_FILES for one accessor, `subscribe_stream()` (§9.8). This edit lands mid-step, so `check 32` will flag the plan, the spec and `engine.rs`; Claude runs the POST_GATE directly and reviews the `engine.rs` diff by hand.

```text
[STEP_ID]
STEP 32: bridge.rs — BridgeInput, Effects, step(), EngineController (config persistence, engine spawn, interrupt, operator), run_tui with terminal guard and panic hook

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/lib.rs
  - crates/orrery-tui/src/bridge.rs
  - crates/orrery-tui/src/app.rs
  - crates/orrery-tui/src/widgets.rs
  - crates/orrery-core/src/engine.rs (only `WorkflowEngine::subscribe_stream`)
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1106-1119 (§9.9 bridge block; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:574-580 (§6.5 engine bridge)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1075-1078 (§9.9 App fields incl. mcp_servers, step_scroll; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:519 (§6.2 Settings modal lists MCP servers)
  - crates/orrery-core/src/engine.rs (build_engine, Backends, RunOptions, WorkflowEngine::operator_sender)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The render loop never awaits engine work. Tests never open a terminal.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-tui/src/widgets.rs && test ! -e crates/orrery-tui/src/bridge.rs && grep -q '^tokio-util.workspace = true$' crates/orrery-tui/Cargo.toml && sed -n 574p $S | grep -q '^### 6.5 ' && sed -n 1075p $S | grep -q 'pub mcp_servers: Vec<McpServerConfig>' && ! grep -q 'step_scroll' crates/orrery-tui/src/app.rs

[MUTATION_SPEC]
1. lib.rs: `pub mod bridge;` and `pub use bridge::run_tui;`.
2. bridge.rs, the §9.9 bridge block:
   - `step` maps inputs to `App` calls and returns `Effects`. `redraw` is true for every input except a `Tick` with no change; `quit` = `app.should_quit`.
   - `EngineController::handle`:
     - config-edit actions → save via `ConfigStore::save_project`;
     - `IngestPlan` → parse with `ImplementationPlan::from_file`, set `plan_path`/`step_marker`, save;
     - `Run` → `build_engine(store, make_backends()?)`, then spawn `engine.run(RunOptions { wait_for_operator: true, ..Default }, cancel.child_token())` on a Tokio task, and forward its `EngineEvent`s and its `LiveStreamHub` events (`engine.subscribe_stream()`, added to `engine.rs` as `self.ctx.stream.subscribe()`) into the app's channels;
     - `Interrupt` → `cancel()`;
     - `Approve`/`Reject` → `operator_sender().send(..)`.
   - `run_tui`:
     - enable raw mode, the alternate screen and mouse capture;
     - install a panic hook that restores the terminal, then calls the previous hook; a guard struct also restores on drop;
     - loop with `tokio::select!` over `crossterm::event::EventStream` (`futures_util::StreamExt::next`), the engine/stream channels and a 250 ms tick;
     - recompute `UiLayoutRegions` on each draw;
     - `SetMouseCapture` toggles crossterm mouse capture;
     - builds the `App` with `App::new(..)`, then sets `app.mcp_servers = store.load_mcp_servers()?`.
3. app.rs (§9.9 fields):
   - add `pub mcp_servers: Vec<McpServerConfig>` (empty in `new`) and `pub step_scroll: usize` (0 in `new`);
   - `keep_selection_visible` also keeps `step_selected` inside `step_scroll..step_scroll + rows`, where rows = the steps panel's inner height (`steps_panel` minus its border); skip it when `rows == 0`;
   - a left click on `StepsList` selects `step_scroll + clicked_row` (still only when that index is `< len`).
4. widgets.rs:
   - `render_steps_panel` draws from `app.step_scroll` instead of deriving the scroll from `step_selected`;
   - the Settings modal's tools line becomes `MCP servers: <server_id, ...>` from `app.mcp_servers` (`(none)` when empty); gate tools stay on their own `Gate tools:` line.
5. Tests (bridge::tests; no terminal; `EngineController` with mock `Backends`; `#[tokio::test]`):
   - `step_key_returns_actions`.
   - `step_engine_event_updates_app`.
   - `step_stream_event_appends`.
   - `run_action_starts_engine_status_running`.
   - `interrupt_cancels_and_saves_checkpoint`: the status returns to `Idle`, and the checkpoint step is `InProgress`.
   - `approve_reaches_operator_channel`: with the engine awaiting the operator, `Approve` makes the step `Passed`.
   - `config_edit_persists`: `SetStageCount(4)`, then `ConfigStore::load_project` shows 4 stages.
6. Tests (app::tests): `keep_selection_visible_scrolls_steps`: 20 steps, 80×24, `step_selected = 18` → `step_scroll` such that 18 is the last visible row; then `step_selected = 0` → `step_scroll == 0`. `steps_click_respects_scroll`: with `step_scroll = 12`, a click on the first inner row selects step 12.
7. Tests (widgets::tests): `settings_lists_mcp_servers`: one MCP server `fs` in `app.mcp_servers`, Settings modal open → the buffer contains `MCP servers: fs`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub use bridge::run_tui;$' crates/orrery-tui/src/lib.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^bridge::tests::(step_key_returns_actions|step_engine_event_updates_app|step_stream_event_appends|run_action_starts_engine_status_running|interrupt_cancels_and_saves_checkpoint|approve_reaches_operator_channel|config_edit_persists): test$')" -eq 7 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(app::tests::(keep_selection_visible_scrolls_steps|steps_click_respects_scroll)|widgets::tests::settings_lists_mcp_servers): test$')" -eq 3 && grep -q 'pub step_scroll: usize' crates/orrery-tui/src/app.rs

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase C — `orrery-cli` (Steps 33–37)

### STEP 33: CLI Crate & Argument Parsing

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. **Changed from the 2026-11-12 block:** `tokio-util` is added to the CLI crate's dependencies. `orrery run` cancels the engine on Ctrl+C through a `CancellationToken`. `main` prints the parsed command in this step only (Step 34 replaces it).
- Amended 2026-11-15 during Step 33's review: §9.10's `args_conflicts_with_subcommands = true` broke `orrery --project ./x status` under clap 4.6.7 (parsed as `path = "status"`), and without it `orrery ./x status` silently dropped `./x`. Replaced by `Cli::validate` (§9.10). This edit lands mid-step, so `check 33` will flag the plan and the spec; Claude runs the POST_GATE directly.

```text
[STEP_ID]
STEP 33: orrery-cli crate (workspace member, [[bin]] orrery) + args.rs (every spec §7 command and flag)

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-cli/Cargo.toml
  - crates/orrery-cli/src/main.rs
  - crates/orrery-cli/src/args.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:581-620 (§7 command table; names binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1121-1123 (§9.10 args bullet; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:112-124 (§3.2: Step 33 tokio row)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Command and flag names exactly as §7. No handlers (Step 34).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-tui/src/bridge.rs && test ! -e crates/orrery-cli && ! grep -q '^clap' Cargo.toml && sed -n 581p $S | grep -q '^## 7\. ' && sed -n 1121p $S | grep -q '^### 9.10 '

[MUTATION_SPEC]
1. Root:
   - add `"crates/orrery-cli"` to `members`;
   - `[workspace.dependencies]`: `clap = { version = "4.6.7", features = ["derive"] }`, `orrery-tui = { path = "crates/orrery-tui" }`;
   - replace tokio's features with `["sync", "macros", "rt", "rt-multi-thread", "process", "io-util", "time", "signal"]`.
2. crates/orrery-cli/Cargo.toml:
   - `name = "orrery-cli"`, with the workspace package fields;
   - `[[bin]] name = "orrery"`, `path = "src/main.rs"`;
   - `[dependencies]`: `orrery-core`, `orrery-tui`, `clap`, `tokio`, `tokio-util`, `serde_json` (all `.workspace = true`);
   - `[dev-dependencies]`: `tempfile.workspace = true`;
   - `[lints] workspace = true`.
   Run `cargo fetch`.
3. args.rs: the §9.10 args bullet, with every §7 command and flag under exactly those names.
4. main.rs: `mod args;` and a `main` that parses `Cli`, calls `cli.validate()` (on `Err`, `e.exit()`), and prints `{:?}` of `into_command()`.
5. Tests (args::tests; `Cli::try_parse_from`):
   - `every_spec_row_parses`: one assertion per §7 row (table-driven `&[&[&str]]`).
   - `bare_orrery_is_tui_dot`.
   - `orrery_path_is_tui_path`.
   - `stage_count_out_of_range_rejected`: `workflow stages 1` and `16` fail; `2` and `15` parse.
   - `project_flag_is_global`: `orrery status --project ./x` and `orrery --project ./x status` both parse.
   - `path_with_subcommand_is_usage_error`: `orrery ./x status` → `validate()` is `Err` with kind `ArgumentConflict`; `orrery ./x`, `orrery status` and `orrery --project ./x status` validate `Ok`, the last as `Status`.
   - `mount_and_inference_flags`: `--mount rw` and `--mode router` parse; `--mount bad` and `--mode other` fail; `inference add` with both `--container` and `--compose` fails.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '"crates/orrery-cli"' Cargo.toml && grep -q '"signal"' Cargo.toml && grep -q '^tokio-util.workspace = true$' crates/orrery-cli/Cargo.toml && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^args::tests::(every_spec_row_parses|bare_orrery_is_tui_dot|orrery_path_is_tui_path|stage_count_out_of_range_rejected|project_flag_is_global|mount_and_inference_flags): test$')" -eq 6

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 34: CLI Project & Workflow Commands

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. Commands not handled yet return a fixed `Validation` error in this step only. Steps 35–36 replace it, and Step 36's `POST_GATE` proves the text is gone.

```text
[STEP_ID]
STEP 34: commands.rs — CliContext, dispatch, exit_code_for; handlers for init, ingest, workflow *, gate *, hook *; real async main

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/main.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1121-1133 (§9.10 commands block, main bullet, JSON shapes; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:581-620 (§7)
  - crates/orrery-core/src/config.rs (ConfigStore, ProjectConfig methods; the only validation used)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No duplicated validation logic in handlers.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-cli/src/args.rs && test ! -e crates/orrery-cli/src/commands.rs && sed -n 1121p $S | grep -q '^### 9.10 '

[MUTATION_SPEC]
1. commands.rs: `CliContext`, `dispatch`, `exit_code_for` (§9.10).
   - Handlers in this step: `init`, `ingest`, `workflow show|stages|set-role|assign|unassign|clear|mode`, `gate set|clear`, `hook set|clear`. Each uses `ConfigStore` + `ProjectConfig` methods only.
   - `workflow show --json` has the §9.10 shape.
   - Every other command returns exactly `Err(Error::Validation { field: "command".into(), message: "not implemented until Step 35/36".into() })`.
2. main.rs: `mod commands;` and the real `main` per §9.10: async, a `CliContext` with `OsKeyringStore`, exit codes.
3. Tests (commands::tests; TempDirs; `CliContext` writing to `Vec<u8>`; `InMemorySecretStore`):
   - `init_creates_layout_and_template`.
   - `ingest_sets_plan_and_prints_steps`.
   - `ingest_bad_plan_exits_1`.
   - `workflow_show_json_shape`.
   - `workflow_stages_grows_and_shrinks`.
   - `workflow_set_role_assign_unassign_clear`.
   - `workflow_invalid_edit_exits_1`: the orchestrator role on stage 2.
   - `workflow_mode_loop_with_cycles`.
   - `gate_set_and_clear`.
   - `hook_set_and_clear`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^mod commands;$' crates/orrery-cli/src/main.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(init_creates_layout_and_template|ingest_sets_plan_and_prints_steps|ingest_bad_plan_exits_1|workflow_show_json_shape|workflow_stages_grows_and_shrinks|workflow_set_role_assign_unassign_clear|workflow_invalid_edit_exits_1|workflow_mode_loop_with_cycles|gate_set_and_clear|hook_set_and_clear): test$')" -eq 10

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 35: CLI Catalog & Secret Commands

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13; amended 2026-11-14. The fixtures `llama-coder-14b` and `docker-models/router/compose.yaml` are placeholders, not host paths.
- The amendment makes `mcp add-*` validate the connection before saving, so a server that's saved has already answered `list_tools`.
- `inference add --compose` now takes a path relative to `.orrery/` (spec §7 rows, §9.6a path scope).

```text
[STEP_ID]
STEP 35: commands.rs — agent *, inference *, role *, mcp *, secret * handlers; read_secret wiring (rpassword)

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-cli/Cargo.toml
  - crates/orrery-cli/src/main.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:581-620 (§7 rows for agent, inference, role, mcp, secret)
  - ORRERY_TECHNICAL_SPECIFICATION.md:414-417 (§5.7.2 residual-risk and guardrail notes)
  - ORRERY_TECHNICAL_SPECIFICATION.md:336-342 (§5.3 keyutils re-run note)
  - ORRERY_TECHNICAL_SPECIFICATION.md:939 (§9.6a resolve_compose_file, ConfigScope, project-scope checks)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. A secret value never reaches stdout, stderr or a log.
  - A server or inference entry that fails its check is never written. No absolute path in any fixture.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-cli/src/commands.rs && ! grep -q '^rpassword' Cargo.toml && sed -n 414p $S | grep -q '^#### 5.7.2 ' && sed -n 939p $S | grep -q 'resolve_compose_file' && grep -q 'pub fn resolve_compose_file' crates/orrery-core/src/config.rs

[MUTATION_SPEC]
1. Root: `rpassword = "7.5.5"`. CLI crate: `rpassword.workspace = true`. Run `cargo fetch`.
2. Handlers: `agent add-local-model|add-cli|add-cloud|list|show|remove`, `inference add|list|remove`, `role add|list|show|remove`, `mcp add-stdio|add-http|list|tools|remove`, `secret set|delete`.
   - `agent add-cli` prints both §5.7.2 residual-risk notes (Docker socket env, `.git/hooks`); `agent show` for `AgentCli` prints the §5.7.2 guardrail note.
   - `mcp add-stdio` / `add-http`: build the `McpServerConfig`, run `validate` and `validate_project_scope`, then `make_backends(project)?.connector.connect(&server)` and `list_tools()`. Save only if both succeed, then print `"{id}: {n} tools"`. Any failure exits 1 with the error and writes nothing.
   - `inference add --compose <FILE>`: run `resolve_compose_file(FILE, ConfigScope::Project, root, None)` first and save the relative path as given (never the resolved one). Failure exits 1 and writes nothing.
   - `role add --rules-file` reads that Markdown file and saves it via `save_role`.
   - `mcp tools <ID>` connects through `make_backends(project)?.connector`.
   - `secret set` uses `ctx.read_secret` and `ctx.secrets.set` and never prints the value. When the real store's backend is `Keyutils`, it prints the §5.3 re-run note.
3. main.rs: wire `read_secret` (rpassword on a TTY, a stdin line otherwise).
4. Tests (commands::tests):
   - `agent_add_each_backend_and_list_json`.
   - `agent_add_cli_prints_residual_risk`.
   - `agent_add_with_embedded_secret_exits_1`: `--model sk-ABCDEFGH12345678`.
   - `agent_show_and_remove`.
   - `inference_add_defaults_external_single`: only `--id` and `--endpoint` give `mode == Single` and `lifecycle == External`.
   - `inference_add_container_and_compose`: `--container llama-coder-14b` gives `Container`. With `<root>/.orrery/docker-models/router/compose.yaml` created first, `--compose docker-models/router/compose.yaml --service llama --mode router` gives `Compose` + `Router`, and the saved `file` is still the relative path.
   - `inference_add_compose_absolute_rejected`: an absolute TempDir path exits 1, and `inference list` shows nothing.
   - `inference_list_remove`.
   - `role_add_reads_rules_file`.
   - `mcp_add_stdio_and_http_list`: `MockMcpConnector` has transports for both ids; each add prints its tool count.
   - `mcp_add_unreachable_server_not_saved`: the connector has no transport for the id. Exit 1, and `mcp list` is empty.
   - `mcp_tools_lists_mock_tools`: `MockMcpConnector` via `make_backends`.
   - `secret_set_and_delete`: the value lands in the `InMemorySecretStore`, and the output doesn't contain it.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^rpassword = "7.5.5"$' Cargo.toml && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(agent_add_each_backend_and_list_json|agent_add_cli_prints_residual_risk|agent_add_with_embedded_secret_exits_1|agent_show_and_remove|inference_add_defaults_external_single|inference_add_container_and_compose|inference_list_remove|role_add_reads_rules_file|mcp_add_stdio_and_http_list|mcp_tools_lists_mock_tools|secret_set_and_delete|inference_add_compose_absolute_rejected|mcp_add_unreachable_server_not_saved): test$')" -eq 13

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 36: CLI Run, Operator & Status Commands; TUI Launch

* **Recommended Command:** `/boost`

Scope note: written 2026-11-13. After this step every `Command` variant is handled and the Step 34 stub text is gone.
- Amended 2026-11-15 after Step 34's review: a plan step's `title` already includes its number (spec §5.4: the marker line minus the marker, e.g. `1: Alpha`), so `ingest` printing `Step {number}: {title}` shows `Step 1: 1: Alpha`. Item 2 makes it print the title alone.
- Amended 2026-11-15 after Step 35's review: `secret set` stored an empty value (for example `echo "" | orrery secret set ...`), which only shows up later as a confusing auth failure. Item 3 rejects it.

```text
[STEP_ID]
STEP 36: commands.rs — run, approve, reject, resume, reset, status, logs handlers; bare `orrery` launches the TUI; TUI ↔ CLI action parity test

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/main.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1121-1133 (§9.10: run output format, exit codes, approve/reject, JSON shapes; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:581-620 (§7)
  - crates/orrery-core/src/stream_log.rs (tail_log), crates/orrery-tui/src/app.rs (AppAction), crates/orrery-tui/src/bridge.rs (run_tui)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Bare `orrery` routes to the TUI.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'not implemented until Step 35/36' crates/orrery-cli/src/commands.rs && grep -q '^rpassword.workspace = true$' crates/orrery-cli/Cargo.toml && sed -n 1121p $S | grep -q '^### 9.10 '

[MUTATION_SPEC]
1. Handlers: `run [--loop] [--cycles] [--step]`, `approve`, `reject --feedback`, `resume`, `reset --step`, `status [--json]`, `logs [--tail] [--follow]`, and `Command::Tui { path }` → `orrery_tui::run_tui(path)`.
   - Behaviour and exit codes per §9.10: `RunOptions { wait_for_operator: false, .. }`; Ctrl+C via `tokio::signal::ctrl_c()` cancelling a `CancellationToken`.
   - `logs` uses `stream_log::tail_log`; `--follow` polls the file every 500 ms until Ctrl+C.
   - Remove the Step 34 stub, so every `Command` variant is handled.
2. `ingest` prints each step as `{title}` alone (no `Step {number}: ` prefix); update `ingest_sets_plan_and_prints_steps` to expect `1: Setup` and `2: Build`, and to assert the output does not contain `Step 1: 1:`.
3. `secret set`: when `ctx.read_secret` returns an empty string, return `Err(Error::Validation { field: "secret".into(), message: "secret value is empty".into() })` (exit 1) before calling `ctx.secrets.set`.
4. Tests (commands::tests; mock `Backends` via `make_backends`):
   - `secret_set_empty_rejected`: `read_secret` returns `""`; exit 1, and `InMemorySecretStore::get` for that service/account finds nothing.
   - `run_mode_passes_one_step_exit_0`.
   - `run_loop_cycles_two`.
   - `escalation_exits_3_then_approve_resume_completes`.
   - `reject_records_feedback`: the next orchestrator prompt (from `MockAgentDriver::invocations`) contains it.
   - `approve_without_awaiting_step_exits_1`.
   - `reset_step_one_marks_pending`.
   - `status_json_shape`.
   - `logs_tail_shows_redacted_lines`.
   - `tui_action_parity`: `fn cli_equivalent(a: &AppAction) -> Option<Vec<String>>`, a `match` with one arm per `AppAction` variant and no `_` arm, so a new variant fails to compile. The arms:
     - `Run` → `["orrery","run"]`;
     - `Approve` → `["orrery","approve"]`;
     - `Reject(f)` → `["orrery","reject","--feedback",f]`;
     - `IngestPlan(p)` → `["orrery","ingest",p]`;
     - `SetMode` → `["orrery","workflow","mode","run"|"loop"]`;
     - `SetCycles(n)` → `["orrery","workflow","mode","loop","--cycles",n]`;
     - `SetStageCount(n)` → `["orrery","workflow","stages",n]`;
     - `SetStageRole` → `set-role`; `AssignAgent` → `assign`; `UnassignAgent` → `unassign`;
     - `None` only for `Interrupt` (Ctrl+C during `orrery run`), `SetMouseCapture` and `Quit`.
     Assert that every `Some` argv parses with `Cli::try_parse_from`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'not implemented until Step' crates/orrery-cli/src/commands.rs && grep -q 'run_tui' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(run_mode_passes_one_step_exit_0|run_loop_cycles_two|escalation_exits_3_then_approve_resume_completes|reject_records_feedback|approve_without_awaiting_step_exits_1|reset_step_one_marks_pending|status_json_shape|logs_tail_shows_redacted_lines|tui_action_parity|secret_set_empty_rejected): test$')" -eq 10

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 37: Release Readiness Check

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. The smoke plan uses the product default marker `## Step ` (what `init` writes), not this file's `### STEP `.
- Amended 2026-11-15 after Step 34's review: the TUI steps panel (Step 31) prints `{badge} {number}. {title}`, but `title` already includes the number (`[ ] 1. 1: first`). `widgets.rs` joins ALLOWED_FILES for item 3 only.

```text
[STEP_ID]
STEP 37: tests/cli_smoke.rs — end-to-end CLI smoke test of the built binary; final dependency and live-test compile checks

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/tests/cli_smoke.rs
  - crates/orrery-tui/src/widgets.rs (only `render_steps_panel`'s label and `steps_panel_badges`)
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:621-644 (§8: live-backend env vars and command)
  - ORRERY_TECHNICAL_SPECIFICATION.md:581-620 (§7)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The smoke test uses no network, no Docker and no keychain.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'not implemented until Step' crates/orrery-cli/src/commands.rs && test ! -e crates/orrery-cli/tests/cli_smoke.rs && sed -n 621p $S | grep -q '^## 8\. '

[MUTATION_SPEC]
1. tests/cli_smoke.rs, one test `init_ingest_show_status_exit_zero`. Run `env!("CARGO_BIN_EXE_orrery")` in a TempDir:
   - `init --name smoke`;
   - write `plan.md` as `"## Step 1: first\nalpha\n## Step 2: second\nbeta\n"`;
   - `ingest plan.md`;
   - `workflow show --json` (parse the output with `serde_json`);
   - `status`.
   Each exits 0.
2. widgets.rs: when a plan is loaded, `render_steps_panel` labels each step `{badge} {title}` (title as-is). Extend `steps_panel_badges` to assert the panel shows `[ ] 1: A`-style labels with no `1. 1:`.
3. Step report to the operator: the live-backend command
     cargo test --workspace --locked --features orrery-core/live-backends
   with `ORRERY_LIVE_MCP_CMD`, `ORRERY_LIVE_IMAGE`, `ORRERY_LIVE_CONTAINER`, `ORRERY_LIVE_LLM_URL`, `ORRERY_LIVE_LLM_MODEL` (§8). `live_keyring` needs a desktop session or keyutils.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy --workspace --all-targets --features orrery-core/live-backends --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! cargo tree --locked -i openssl-sys >/dev/null 2>&1 && ! cargo tree --locked -i zbus >/dev/null 2>&1 && cargo test -p orrery-cli --test cli_smoke --locked 2>&1 | grep -q 'test result: ok. 1 passed' && ! grep -q 'step.number, step.title' crates/orrery-tui/src/widgets.rs

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase D — follow-up (Steps 38–51)

**Phase goal (frozen until the phase ends):** close the review notes parked during Steps 1–37 and the spec-vs-code gaps found after Step 37, so every config field does what it says, MCP servers and model servers can't hang or garble a run, the TUI never blocks or hides an operator decision, and operators have a README. Design: spec §9.11 (`ORRERY_TECHNICAL_SPECIFICATION.md:1135-1296`).

Written 2026-11-15 against the Step 37 tree (`152d415`). Line numbers in `READ_ONLY_REFS` are from that tree; a step that edits a file before a later step reads it shifts them, so later steps name functions as well as lines.

**Decisions the contract leaves open** (settled here, not by the executor):
- `max_tool_calls`, the model idle timeout and the live-stream sizes live in one project-level `[limits]` table (§9.11.2), not per agent. Per-agent fields would add a field to every `AgentBackend` struct literal (about 50 sites in 9 files).
- `max_context_tokens` is a preflight check that fails the turn (§9.11.5). No history trimming and no tokenizer.
- On a halted `on_step_pass` hook, Approve re-runs the hook only (§9.11.7, operator's call 2026-11-15).
- Approve/Reject show in the TUI whenever a step awaits the operator, in every validation mode (§9.11.10).
- Windows stays out of scope for this phase (spec §8 is silent on platform; §9.12.1 now states Linux only for now). `PATHEXT` is parked.

### User Step: LIVE-1 — run the live-backend tests first (COMPLETED 2026-11-15)
Done by the operator; every live test passed. `live_docker` needs an existing container and a small image, which nothing documented. The operator's setup (Step 51 puts it in the README):
```bash
docker pull busybox:1.37
docker create --name orrery-live-test busybox:1.37 sh -c 'trap exit TERM; while :; do sleep 1; done'
ORRERY_LIVE_IMAGE=busybox:1.37 ORRERY_LIVE_CONTAINER=orrery-live-test cargo test -p orrery-core --locked --features orrery-core/live-backends --test live_docker
```
`trap exit TERM` matters: `sh` as PID 1 ignores SIGTERM, so without it `docker stop` waits out its 10 s grace period.

The full run:
```bash
cargo test --workspace --locked --features orrery-core/live-backends
```
Env vars: `ORRERY_LIVE_MCP_CMD`, `ORRERY_LIVE_IMAGE`, `ORRERY_LIVE_CONTAINER`, `ORRERY_LIVE_LLM_URL`, `ORRERY_LIVE_LLM_MODEL`. `live_keyring` needs Secret Service or keyutils.

### Parked
Tracked in neb since 2026-11-18 (`neb list --status pending`). The four items below → TASK-0001 (PATHEXT), FEAT-0001 (TUI catalog editing), BUG-0003 (TUI error chain), TASK-0002 (chrono); the 13 open Phase G Lows → BUG-0004…0012, TASK-0003…0006; Nebulous audit A3 → BUG-0001. Kept below for history; do not add here.

Phase D review (2026-11-16): the operator picked six items for Phase E (Steps 52–56). The rest stay parked.
Phase E review (2026-11-16): the README keys note became Step 58; LIVE-2 runs the live suite.
Phase H review (2026-11-17): the operator picked the four cheap Lows for Phase I (Steps 77–79; the §7 `add-cli` synopsis was fixed in the spec directly). The rest stay parked.
Phase I review (2026-11-17): the operator picked the secret-handling Phase G Lows F63-9, F64-5 and F62-5 for Phase J (Steps 80–83). The other 13 Phase G Lows (validation notes) and the items below stay parked.
- `mcp.rs` `resolve_stdio_command` — Windows `PATHEXT` for bare names — needs Windows as a target first (spec §9.12.1: Linux only for now; Windows planned).
- TUI catalog editing (agents, roles, MCP servers, inference) — §6.2 leaves editing to the CLI in v1.
- `orrery-tui` `status_message = Some(err.to_string())` (`app.rs`, `bridge.rs`) drops the error's source chain, like the CLI did before Step 45 — reuse a core helper once one exists.
- `crates/orrery-core` `chrono` use (`Utc::now`, `DateTime<Utc>` in `stream_log.rs`, `checkpoint.rs`, `llm.rs`) — RUSTSEC-2026-0543 marks chrono unmaintained; `deny.toml` ignores it (operator, 2026-11-17) because rmcp 3.9 depends on it too. Migrate orrery-core to `jiff` or `time` to cut our direct use, and drop the ignore once rmcp no longer pulls chrono. Low.

---

### STEP 38: MCP Child Stderr into the Live Stream

* **Recommended Command:** `/goal`

Scope note: the Step 11 and Step 26 notes. A stdio MCP server's stderr is inherited today, so any server that logs draws over the TUI. `RmcpConnector` is built in `Backends::real` before the hub exists, so the hub reaches it through a default-no-op trait method instead of a new `Backends` field (no change to the `Backends` literals in the CLI and TUI tests).

```text
[STEP_ID]
STEP 38: mcp.rs + engine.rs — stdio MCP stderr is piped, line-split and emitted as redacted SystemNotice events once build_engine attaches the hub; CLI-only connects keep inherited stderr

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1138-1159 (§9.11.1; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:802-842 (§9.5)
  - crates/orrery-core/src/mcp.rs:442-519 (RmcpTransport::connect_in), 559-614 (RmcpConnector, MockMcpConnector)
  - crates/orrery-core/src/engine.rs:1839-1880 (Backends, Backends::real), 1882-1943 (build_engine up to the connect loop)
  - rmcp 3.9.0 `src/transport/child_process.rs` in the cargo registry (TokioChildProcess::builder, TokioChildProcessBuilder::stderr, spawn → (TokioChildProcess, Option<tokio::process::ChildStderr>))
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The `Backends` struct fields, `RmcpTransport::connect` and `connect_in` signatures stay as they are.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'attach_stream' crates/orrery-core/src/mcp.rs && grep -q 'TokioChildProcess::new(cmd)' crates/orrery-core/src/mcp.rs && sed -n 1138p $S | grep -q '^#### 9.11.1 '

[MUTATION_SPEC]
1. mcp.rs: `McpConnector` gains `fn attach_stream(&self, _stream: Arc<LiveStreamHub>) {}` (default body; `MockMcpConnector` keeps the default).
2. mcp.rs: `RmcpConnector` gains `pub stream: std::sync::OnceLock<Arc<LiveStreamHub>>`; its `attach_stream` is `let _ = self.stream.set(stream);`. Its `connect`, Stdio arm, calls `RmcpTransport::connect_with_stream(&clone, self.secrets.as_ref(), &self.project_root, self.stream.get().cloned())`.
3. mcp.rs: `RmcpTransport::connect_with_stream(server, secrets, workdir, stream: Option<Arc<LiveStreamHub>>)` holds today's `connect_in` body. Stdio with `None` → `TokioChildProcess::new(cmd)` as now. Stdio with `Some(hub)` → `TokioChildProcess::builder(cmd).stderr(std::process::Stdio::piped()).spawn()`; on `Some(handle)` call `spawn_stderr_forwarder(server_id.clone(), handle, hub)` before `serve`. `connect_in` becomes a one-line call to it with `None`.
4. mcp.rs: `pub fn spawn_stderr_forwarder<R>(server_id: String, stderr: R, stream: Arc<LiveStreamHub>) -> tokio::task::JoinHandle<()> where R: tokio::io::AsyncRead + Unpin + Send + 'static` per §9.11.1 (`tokio::io::AsyncBufReadExt::lines`; skip lines whose `trim()` is empty; text `"[mcp:{server_id}] {line}"`; ignore `emit` errors).
5. engine.rs: `Backends::real` sets `stream: std::sync::OnceLock::new()`. `build_engine` calls `backends.connector.attach_stream(Arc::clone(&stream));` right after the hub is created, before the connect loop.
6. Tests:
   - `mcp::tests::stderr_forwarder_emits_redacted_notices` (`#[tokio::test]`): a `SecretRedactor` with `SecretValue::new("hunter2-secret-value")` registered; hub in a `TempDir`; input `&b"first line\n\n   \nleaks hunter2-secret-value\n"[..]` (`&[u8]` implements `AsyncRead`); await the handle; `hub.recent(10)` has exactly 2 `SystemNotice` events with text `"[mcp:fs] first line"` and `"[mcp:fs] leaks [REDACTED]"`.
   - `mcp::tests::attach_stream_first_wins`: an `RmcpConnector` with `stream: OnceLock::new()`; attach hub A, then hub B; `Arc::ptr_eq(connector.stream.get().unwrap(), &a)`.
   - `engine::tests::build_engine_attaches_stream_to_connector` (`#[tokio::test]`): a test-only connector that wraps `MockMcpConnector` and sets an `AtomicBool` in `attach_stream`; build the engine with it (the existing `build_engine` test setup); the flag is `true`.
   - Update every `RmcpConnector { .. }` literal in `mcp.rs` tests with `stream: std::sync::OnceLock::new()`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn attach_stream' crates/orrery-core/src/mcp.rs && grep -q 'Stdio::piped()' crates/orrery-core/src/mcp.rs && grep -q 'attach_stream(Arc::clone(&stream))' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(mcp::tests::(stderr_forwarder_emits_redacted_notices|attach_stream_first_wins)|engine::tests::build_engine_attaches_stream_to_connector): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 39: Runtime Limits Table and Guardrail Note Constant

* **Recommended Command:** `/goal`

Scope note: config surface only. Steps 40, 41 and 45 read these. `ProjectConfig` has one struct literal (`default_template`), so the new field breaks nothing else.

```text
[STEP_ID]
STEP 39: config.rs — RuntimeLimits as an optional [limits] table in project.toml, validated in validate_staging; AGENT_CLI_GUARDRAIL_NOTE constant

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1160-1192 (§9.11.2; binding)
  - crates/orrery-core/src/config.rs:150-311 (ProjectConfig, default_template, validate_staging)
  - crates/orrery-cli/src/commands.rs:420-427 (the guardrail literal the constant copies)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Existing ProjectConfig fields and their serde attributes stay as they are; a project.toml without [limits] serializes byte-for-byte as before.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'fn attach_stream' crates/orrery-core/src/mcp.rs && ! grep -q 'RuntimeLimits' crates/orrery-core/src/config.rs && sed -n 1160p $S | grep -q '^#### 9.11.2 '

[MUTATION_SPEC]
1. config.rs: `RuntimeLimits` with the derives, serde attributes, defaults, `validate` and `is_default` of §9.11.2.
2. config.rs: `ProjectConfig` gains `#[serde(default, skip_serializing_if = "RuntimeLimits::is_default")] pub limits: RuntimeLimits` after `inference`; `default_template` sets `limits: RuntimeLimits::default()`.
3. config.rs: `validate_staging` calls `self.limits.validate()?;` right after `self.hooks.validate()?;`.
4. config.rs: `pub const AGENT_CLI_GUARDRAIL_NOTE: &str` with exactly the §9.11.2 text.
5. Tests (config::tests):
   - `limits_default_when_omitted`: `ProjectConfig::default_template("p")` serialized with `toml::to_string` has no `[limits]`; parsing a project.toml without `[limits]` gives `RuntimeLimits::default()`.
   - `limits_round_trip`: `max_tool_calls = 7`, `model_idle_timeout_secs = 42` (others default) → `toml::to_string` contains `[limits]` → parses back equal.
   - `limits_validation_bounds`: each of `max_tool_calls = 0`, `model_idle_timeout_secs = 0`, `live_stream_ring_capacity = 0`, `live_stream_max_file_bytes = 4095`, `live_stream_max_files = 11` (one at a time on a default) → `Error::Validation` whose `field` is `"limits.<that name>"`; `max_tool_calls = 1001` fails too; a default-template project with `max_files = 11` fails `validate_staging`.
   - `limits_unknown_key_rejected`: `[limits]\nmax_tools = 3\n` in a project.toml fails to parse.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub struct RuntimeLimits' crates/orrery-core/src/config.rs && grep -q 'pub const AGENT_CLI_GUARDRAIL_NOTE' crates/orrery-core/src/config.rs && grep -q 'self.limits.validate()?' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(limits_default_when_omitted|limits_round_trip|limits_validation_bounds|limits_unknown_key_rejected): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 40: Chat Driver Idle Timeout, Sensitive Header, Limits Wiring

* **Recommended Command:** `/goal`

Scope note: the Step 17/18 notes. `reqwest::Client::new()` has no timeout, so a model server that stops sending hangs the run until Ctrl+C. The timeout goes around `send()` and each chunk read, not on the client, so `HttpChatDriver::new` stays infallible. The timeout tests use a loopback `tokio::net::TcpListener` (no external network); the existing turn-loop tests drive a private handler with fixture items and stay as they are.

```text
[STEP_ID]
STEP 40: llm.rs + engine.rs — HttpChatDriver::with_idle_timeout around send() and every chunk; Authorization header marked sensitive; Backends::real applies [limits] max_tool_calls and idle timeout

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1193-1201 (§9.11.3; binding)
  - crates/orrery-core/src/llm.rs:567-612 (DEFAULT_MAX_TOOL_CALLS, HttpChatDriver, bearer_header), 951-993 (run_turn)
  - crates/orrery-core/src/engine.rs:1847-1880 (Backends::real)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `HttpChatDriver::new` keeps its signature and stays infallible; no client-builder timeouts.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct RuntimeLimits' crates/orrery-core/src/config.rs && ! grep -q 'with_idle_timeout' crates/orrery-core/src/llm.rs && sed -n 1193p $S | grep -q '^#### 9.11.3 '

[MUTATION_SPEC]
1. llm.rs: `HttpChatDriver` gains a private `idle_timeout: Duration` (default `Duration::from_secs(300)`, a `const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300`) and `pub fn with_idle_timeout(mut self, timeout: Duration) -> Self`.
2. llm.rs: `run_turn` wraps `req.send()` and every `byte_stream.next()` in `tokio::time::timeout(self.idle_timeout, ..)`; elapsed → `Error::Agent { agent_id: inv.agent.agent_id.clone(), message: format!("no response from model within {}s", self.idle_timeout.as_secs()) }`.
3. llm.rs: replace `bearer_header` with private `fn auth_header_value(agent: &AgentConfig, secrets: &dyn SecretStore) -> Result<Option<reqwest::header::HeaderValue>>`: `CloudApi` → `HeaderValue::from_str(&format!("Bearer {}", secret.expose()))` (error → the §9.11.3 `Error::Validation`), `set_sensitive(true)`, `Some`; other backends → `None`. `run_turn` passes the value to `req.header(AUTHORIZATION, value)`. Update `bearer_only_for_cloud_api` to the new helper (cloud → `Some` and `is_sensitive()`; local → `None`).
4. engine.rs: `Backends::real` loads `let limits = store.load_project()?.limits;` and builds the chat driver with `.with_max_tool_calls(limits.max_tool_calls).with_idle_timeout(Duration::from_secs(limits.model_idle_timeout_secs))`.
5. Tests (llm::tests):
   - `send_timeout_is_agent_error` (`#[tokio::test]`): a `TcpListener` on `127.0.0.1:0` that accepts and reads but never writes; a `LocalModel` agent whose inference `endpoint_url` is `http://<that addr>`; driver `.with_idle_timeout(Duration::from_secs(1))`; `run_turn` → `Error::Agent` with message `"no response from model within 1s"`.
   - `stalled_stream_is_agent_error` (`#[tokio::test]`): the listener writes `HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n` (no length: the body runs until close) and then `data: {"choices":[{"delta":{"content":"hi"}}]}\n\n`, then stalls with the socket open; same driver and error.
   - `auth_header_marked_sensitive`: `CloudApi` agent with an `InMemorySecretStore` holding `"test-token-123"` → `Some(v)`, `v.is_sensitive()`, `v.to_str().unwrap() == "Bearer test-token-123"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn with_idle_timeout' crates/orrery-core/src/llm.rs && grep -q 'set_sensitive(true)' crates/orrery-core/src/llm.rs && ! grep -q 'fn bearer_header' crates/orrery-core/src/llm.rs && grep -q 'with_idle_timeout(Duration::from_secs(limits.model_idle_timeout_secs))' crates/orrery-core/src/engine.rs && grep -q 'with_max_tool_calls(limits.max_tool_calls)' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^llm::tests::(send_timeout_is_agent_error|stalled_stream_is_agent_error|auth_header_marked_sensitive): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 41: Live Stream Writer Thread and Configured Sizes

* **Recommended Command:** `/boost`

Scope note: the Step 17 note. `emit` opens, appends to and closes the log file under a std `Mutex` on whichever async worker emits. This moves file IO to one writer thread and takes the sizes from `[limits]`. Callers outside `stream_log.rs` that read the log file (the CLI `logs` test) do so after the engine, and so the hub, is dropped; `Drop` joins the writer, so they see every line.

```text
[STEP_ID]
STEP 41: stream_log.rs + engine.rs — LiveStreamHub writes the log on a dedicated thread (flush + Drop join); build_engine sizes the hub from project [limits]

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/stream_log.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1202-1215 (§9.11.4; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:779-800 (§9.4: rotation rule, file format, line fitting)
  - crates/orrery-core/src/stream_log.rs:50-297 (LiveStreamConfig, LiveStreamHub, rotate_files, tail_log, fit_event_line)
  - crates/orrery-core/src/engine.rs:1925-1935 (hub creation in build_engine)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Public signatures of new, emit, subscribe, recent, redactor and tail_log stay as they are; the on-disk format and rotation rule don't change.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn with_idle_timeout' crates/orrery-core/src/llm.rs && ! grep -q 'pub fn flush' crates/orrery-core/src/stream_log.rs && sed -n 1202p $S | grep -q '^#### 9.11.4 '

[MUTATION_SPEC]
1. stream_log.rs: the writer thread, `WriterMsg`, `flush` and `Drop` of §9.11.4. The hub's `Mutex` holds only the ring buffer. Rotation moves into the writer, unchanged in behaviour (same `.log → .log.1 → .log.2` rule, before a write would pass `max_file_bytes`). The writer keeps one append handle and reopens it after a rotation.
2. stream_log.rs: `emit` follows the §9.11.4 order and never touches the file system.
3. stream_log.rs tests: every existing test that reads a log file after `emit` calls `hub.flush().unwrap()` first. No assertion is weakened.
4. engine.rs: `build_engine` builds `LiveStreamConfig` from `project.limits` (§9.11.4 last bullet).
5. New tests (stream_log::tests):
   - `drop_writes_pending_lines`: emit 100 events, drop the hub, `tail_log(dir, usize::MAX)` has 100 lines.
   - `flush_reports_write_error`: create a directory named `live_stream.log` inside the log dir; `emit` returns `Ok`; `flush` returns `Err(Error::Io { .. })`; a second `flush` returns `Ok` (the error was taken).
   - `rotation_unchanged_with_writer_thread`: `max_file_bytes = 300`, `max_files = 3`, emit 30 events, `flush`; `.log`, `.log.1`, `.log.2` exist, no `.log.3`, and each file is ≤ 300 bytes.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn flush' crates/orrery-core/src/stream_log.rs && grep -q 'orrery-log-writer' crates/orrery-core/src/stream_log.rs && grep -q 'impl Drop for LiveStreamHub' crates/orrery-core/src/stream_log.rs && grep -q 'project.limits.live_stream_max_files' crates/orrery-core/src/engine.rs && ! grep -q 'LiveStreamConfig::default(),' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^stream_log::tests::(drop_writes_pending_lines|flush_reports_write_error|rotation_unchanged_with_writer_thread): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 42: Context Preflight and Unmappable Tool Names

* **Recommended Command:** `/goal`

Scope note: `max_context_tokens` is validated but never used, and a tool name that can't map to a function name is silently renamed (`llm.rs` `unwrap_or_else` in `build_chat_request`), so the model can never call it. (In practice `tools_for_role` already filters those names, so the rename never ran; item 3 removes the dead fallback.)

```text
[STEP_ID]
STEP 42: llm.rs — estimate_tokens preflight against max_context_tokens before every chat request; unmappable tool names left out of requests

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1216-1224 (§9.11.5; binding)
  - crates/orrery-core/src/llm.rs:330-377 (build_chat_request), the run_turn turn loop
  - crates/orrery-core/src/config.rs format_mapped_tool_name
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `build_chat_request` keeps its signature.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn flush' crates/orrery-core/src/stream_log.rs && grep -q 'unwrap_or_else(|_| format!("{}__{}"' crates/orrery-core/src/llm.rs && sed -n 1216p $S | grep -q '^#### 9.11.5 '

[MUTATION_SPEC]
1. llm.rs: `pub fn estimate_tokens(body: &serde_json::Value) -> usize` per §9.11.5.
2. llm.rs: in `run_turn`, after each `build_chat_request` and before the request, compare against the agent's `max_context_tokens` (`LocalModel` / `CloudApi`) and return the §9.11.5 `Error::Agent` when over.
3. llm.rs: `build_chat_request` uses `filter_map` and drops tools whose mapped name fails; the `unwrap_or_else` fallback is gone.
4. Withdrawn 2026-11-15: `McpGatekeeper::is_tool_allowed` (mcp.rs:165-167) already drops unmappable tools from `tools_for_role`, so a `build_engine` check could never fire. `engine.rs` is not changed.
5. Tests:
   - `llm::tests::estimate_tokens_bytes_div_4`: `json!("abcd")` (6 bytes serialized) → 2; `json!({})` (2 bytes) → 1.
   - `llm::tests::context_preflight_rejects_before_request` (`#[tokio::test]`): `LocalModel` agent with `max_context_tokens: 10` and an inference whose `endpoint_url` is `http://127.0.0.1:9` (nothing listens there); `run_turn` → `Error::Agent` whose message starts `"prompt is about "` and contains `"over max_context_tokens 10"` (not `Error::Http`, so no request was sent).
   - `llm::tests::unmappable_tool_left_out_of_request`: tools `fs:read_file` and `fs:read.file`; the request's `tools` names are exactly `["fs__read_file", "submit_report"]`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn estimate_tokens' crates/orrery-core/src/llm.rs && ! grep -q 'unwrap_or_else(|_| format!("{}__{}"' crates/orrery-core/src/llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(llm::tests::(estimate_tokens_bytes_div_4|context_preflight_rejects_before_request|unmappable_tool_left_out_of_request)): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 43: Connect Only Referenced MCP Servers; Cap Hook History

* **Recommended Command:** `/goal`

Scope note: `build_engine` connects every configured MCP server, so one unused server that is down fails every run. `hook_results` grows by one entry per hook call for as long as the checkpoint lives (the Step 23 note).

```text
[STEP_ID]
STEP 43: engine.rs + checkpoint.rs — referenced_mcp_servers decides which servers build_engine connects (others get a notice); WorkflowCheckpoint::push_hook_result keeps the last 200

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
  - crates/orrery-core/src/checkpoint.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1225-1236 (§9.11.6; binding)
  - crates/orrery-core/src/checkpoint.rs:247-307 (HookResult, WorkflowCheckpoint, new)
  - crates/orrery-core/src/engine.rs run_hook (the hook_results.push), build_engine (the connect loop and the unknown-server check)
  - crates/orrery-core/src/config.rs HooksConfig, validate_concrete_tool_ref
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. WorkflowCheckpoint's serde shape is unchanged (old state files load).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn estimate_tokens' crates/orrery-core/src/llm.rs && grep -q 'state.hook_results.push(' crates/orrery-core/src/engine.rs && sed -n 1225p $S | grep -q '^#### 9.11.6 '

[MUTATION_SPEC]
1. checkpoint.rs: `pub const MAX_HOOK_RESULTS: usize = 200;` and `WorkflowCheckpoint::push_hook_result` per §9.11.6.
2. engine.rs: `run_hook` calls `state.push_hook_result(HookResult { .. })`.
3. engine.rs: `pub fn referenced_mcp_servers(project: &ProjectConfig, roles: &[RoleConfig]) -> BTreeSet<String>` per §9.11.6.
4. engine.rs: `build_engine` connects only servers in that set and emits the §9.11.6 notice for each other server. Keep the unknown-server check as it is.
5. Tests:
   - `checkpoint::tests::hook_results_capped`: push 205 results with `step_number` 1..=205 → `len() == 200` and `hook_results[0].step_number == 6`.
   - `engine::tests::referenced_servers_from_stages_gates_hooks`: stage roles allow `fs:read_file` and `git:*`; an unstaged role allows `web:fetch`; `gate_tools = ["cargo:test"]`; `on_step_pass.tool = "notify:send"` → `{"cargo", "fs", "git", "notify"}` (no `"web"`).
   - `engine::tests::build_engine_skips_unused_server` (`#[tokio::test]`): servers `fs` and `unused` in config; the `MockMcpConnector` knows only `fs`; roles reference only `fs` → `build_engine` is `Ok` (it was `Err` before this step).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub const MAX_HOOK_RESULTS: usize = 200' crates/orrery-core/src/checkpoint.rs && grep -q 'push_hook_result(' crates/orrery-core/src/engine.rs && ! grep -q 'state.hook_results.push(' crates/orrery-core/src/engine.rs && grep -q 'pub fn referenced_mcp_servers' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(checkpoint::tests::hook_results_capped|engine::tests::(referenced_servers_from_stages_gates_hooks|build_engine_skips_unused_server)): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 44: Approve on a Halted Step-Pass Hook Re-runs the Hook Only

* **Recommended Command:** `/boost`

Scope note: the Step 23 note. Today a `Halt` from `on_step_pass` returns a passed step to `AwaitingOperator`, and Approve restarts the orchestrator turn, so agents redo finished work. The operator decided 2026-11-15 that Approve re-runs the hook only. `MockMcpTransport` results are fixed per tool, so the "hook passes after approve" test uses a second engine on the same project dir with a succeeding transport, like a CLI resume.

```text
[STEP_ID]
STEP 44: engine.rs — AwaitingContext::StepPassHook; Approve re-runs on_step_pass without agent turns (loop in complete_step_pass), Reject restarts the orchestrator turn; resume detects it by awaiting_reason

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1237-1243 (§9.11.7; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1006-1044 (§9.8 operator decisions, resume)
  - crates/orrery-core/src/engine.rs: PASS_CHECK_AWAITING_REASON (36), AwaitingContext (222), run() AwaitingOperator resume branch (~403-476), enter_awaiting_operator, apply_operator_decision, complete_step_pass, run_hook's Halt reason
  - crates/orrery-core/src/engine.rs tests step_pass_hook_runs_after_pass and halt_hook_failure_awaits_operator (helpers test_context, attach_mock_driver, script_passing_step)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. PassCheck and HaltOrEscalation behaviour is unchanged for every other awaiting reason.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn referenced_mcp_servers' crates/orrery-core/src/engine.rs && ! grep -q 'StepPassHook' crates/orrery-core/src/engine.rs && sed -n 1237p $S | grep -q '^#### 9.11.7 '

[MUTATION_SPEC]
1. engine.rs: `const STEP_PASS_HOOK_AWAITING_REASON: &str = "hook step_pass failed";` and `AwaitingContext::StepPassHook`. `complete_step_pass` awaits with that reason and context on a `Halt` from `on_step_pass`.
2. engine.rs: `apply_operator_decision`: `(StepPassHook, Approve)` → `AwaitResolution::PassStep` with `retry_count` unchanged and the step status left for `complete_step_pass` to set; `(StepPassHook, Reject { feedback })` → exactly the `HaltOrEscalation` reject arm.
3. engine.rs: `complete_step_pass` is a `loop`: on a `Halt`, `PassStep` goes round again (re-runs the hook), `ContinueOrchestrator` → `StepPassOutcome::ContinueRound`, `HaltRun` → `FinishedRun`.
4. engine.rs: in `run`'s resume branch, `awaiting_reason == STEP_PASS_HOOK_AWAITING_REASON` → `StepPassHook`, checked before the `PassCheck` condition.
5. Tests (engine::tests):
   - `step_pass_hook_halt_approve_reruns_hook_only` (`#[tokio::test]`): run 1 with `on_step_pass = git:commit` (Halt) failing (`isError: true`) and `wait_for_operator: false` → `AwaitingOperator { reason: "hook step_pass failed" }`. Set `operator_decision = Some(Approve)` on the saved checkpoint. Run 2 on the same dir with a succeeding `commit` and a fresh `MockAgentDriver` with no scripts → `Completed { steps_passed: 1 }`, run 2's driver has no invocations, run 2's transport saw exactly one `commit` call, step 1 is `Passed`.
   - `step_pass_hook_halt_reject_reruns_orchestrator` (`#[tokio::test]`): same run 1; decision `Reject { feedback: "tag missing" }`; run 2 scripted with `script_passing_step` → the first orchestrator invocation's `user_prompt` contains `"operator rejected: tag missing"`.
   - `step_pass_hook_halt_approve_while_waiting` (`#[tokio::test]`): `wait_for_operator: true`, the hook always fails; after the first `EngineEvent::AwaitingOperator`, send `Approve`; wait for the second `AwaitingOperator`; the transport saw 2 `commit` calls, and the driver's invocation count did not change between the two; cancel the token and await the run.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'STEP_PASS_HOOK_AWAITING_REASON' crates/orrery-core/src/engine.rs && grep -q 'AwaitingContext::StepPassHook' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(step_pass_hook_halt_approve_reruns_hook_only|step_pass_hook_halt_reject_reruns_orchestrator|step_pass_hook_halt_approve_while_waiting): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 45: CLI Error Causes, Status Hint, Log Follower

* **Recommended Command:** `/goal`

Scope note: the Step 36 notes. `main.rs` prints `{err}` only, so every `Error::Io` loses its cause (`status` before `ingest` prints just `I/O error at <path>`). `logs --follow` re-reads all log files every 500 ms, makes a new `ctrl_c()` future each loop, and stops printing once rotation keeps the line count flat.

```text
[STEP_ID]
STEP 45: commands.rs + main.rs + stream_log.rs — render_error prints the source chain; status hints at ingest; logs --follow uses LogFollower and one pinned ctrl_c; agent show uses AGENT_CLI_GUARDRAIL_NOTE

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/commands.rs
  - crates/orrery-cli/src/main.rs
  - crates/orrery-core/src/stream_log.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1244-1260 (§9.11.8; binding)
  - crates/orrery-cli/src/commands.rs:420-427 (agent show guardrail), 820-902 (dispatch_status), 904-938 (dispatch_logs)
  - crates/orrery-cli/src/main.rs:40-73
  - crates/orrery-core/src/stream_log.rs tail_log, LOG_FILE
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Exit codes and the §9.10 output formats are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'AwaitingContext::StepPassHook' crates/orrery-core/src/engine.rs && grep -q 'pub const AGENT_CLI_GUARDRAIL_NOTE' crates/orrery-core/src/config.rs && ! grep -q 'LogFollower' crates/orrery-core/src/stream_log.rs && sed -n 1244p $S | grep -q '^#### 9.11.8 '

[MUTATION_SPEC]
1. commands.rs: `pub fn render_error(err: &Error) -> String` per §9.11.8; main.rs prints it at both error sites.
2. commands.rs: `dispatch_status` returns the §9.11.8 `plan_path` validation error when the resolved plan isn't a regular file.
3. stream_log.rs: `LogFollower` with `new` and `poll` per §9.11.8.
4. commands.rs: `dispatch_logs --follow` per §9.11.8 (tail, then `LogFollower::new`, pinned `ctrl_c`, 500 ms sleep, print `poll()`).
5. commands.rs: `agent show` writes `orrery_core::config::AGENT_CLI_GUARDRAIL_NOTE`; the literal is gone.
6. Tests:
   - `stream_log::tests::follower_reads_appended_lines`: write `"a\n"` to `live_stream.log`, `new` (positioned at end), append `"b\nc\n"` → `poll() == ["b", "c"]`, then `poll()` is empty.
   - `stream_log::tests::follower_survives_rotation`: `new` on `"a\n"`; append `"b\n"`; rename `live_stream.log` to `live_stream.log.1`; write a new `live_stream.log` with `"c\n"` → `poll() == ["b", "c"]`.
   - `stream_log::tests::follower_keeps_partial_line`: `new` on an empty file; append `"par"` → `poll()` empty; append `"tial\n"` → `poll() == ["partial"]`.
   - `commands::tests::render_error_includes_source`: `Error::Io { path: "plan.md".into(), source: io::Error::new(io::ErrorKind::NotFound, "missing") }` → the string contains `"plan.md"` and `"missing"`.
   - `commands::tests::status_before_ingest_hints_ingest` (`#[tokio::test]`): `init` in a `TempDir`, then `status` → `Err(Error::Validation { field, message })` with `field == "plan_path"` and `message` containing `"orrery ingest"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn render_error' crates/orrery-cli/src/commands.rs && grep -q 'render_error(&err)' crates/orrery-cli/src/main.rs && grep -q 'tokio::pin!(ctrl_c)' crates/orrery-cli/src/commands.rs && ! grep -q 'tail_log(&log_dir, usize::MAX)?.len()' crates/orrery-cli/src/commands.rs && grep -q 'AGENT_CLI_GUARDRAIL_NOTE' crates/orrery-cli/src/commands.rs && grep -q 'pub struct LogFollower' crates/orrery-core/src/stream_log.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^stream_log::tests::(follower_reads_appended_lines|follower_survives_rotation|follower_keeps_partial_line): test$')" -eq 3 && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(render_error_includes_source|status_before_ingest_hints_ingest): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 46: TUI Engine Controller Never Blocks the Render Loop

* **Recommended Command:** `/boost`

Scope note: `handle(Run)` awaits `build_engine` (every MCP connect) on the render loop, and `handle(Interrupt)` awaits the engine task, then the forwarder. The forwarder's `send().await` into the bounded stream channel can block while nothing drains it, which deadlocks the TUI on Interrupt. Existing bridge tests that call `handle(Interrupt)` and then check the checkpoint (`interrupt_cancels_and_saves_checkpoint`, `run_action_starts_engine_status_running`) call `controller.shutdown(&mut app).await` after it.

```text
[STEP_ID]
STEP 46: bridge.rs — Run spawns make_backends + build_engine + run in one task; Interrupt only cancels; stream forwarding uses try_send; operator decisions go through a controller-owned channel; EngineController::shutdown drains and waits (30 s) at TUI exit

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1261-1267 (§9.11.9; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:574-579 (§6.5)
  - crates/orrery-tui/src/bridge.rs:97-330 (EngineController), 332-375 (drain_remaining), 469-574 (run_tui), 575- (tests)
  - crates/orrery-tui/src/app.rs:231-306 (apply_engine_event; RunFinished(Interrupted) sets Idle)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `EngineController::new`, `handle`, `step` and `run_tui` keep their signatures.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub struct LogFollower' crates/orrery-core/src/stream_log.rs && ! grep -q 'pub async fn shutdown' crates/orrery-tui/src/bridge.rs && sed -n 1261p $S | grep -q '^#### 9.11.9 '

[MUTATION_SPEC]
1. bridge.rs: `make_backends` stored as `Arc<dyn Fn() -> Result<Backends> + Send + Sync>` (`Arc::from(box)` in `new`).
2. bridge.rs: `handle(Run)` per §9.11.9: create the operator channel, then spawn one task that runs `make_backends()`, `build_engine`, the subscriptions, the event forwarding and `engine.run(RunOptions { wait_for_operator: true, .. }, child_token)`. Set `app.status = Running` before returning. No `.await` on engine work in `handle(Run)`.
3. bridge.rs: stream forwarding and the task's error notice use `try_send` with the dropped-count notice of §9.11.9; engine events keep `send().await`.
4. bridge.rs: `handle(Interrupt)` cancels and sets `status_message = Some("Stopping…".into())`; nothing else. `Approve` / `Reject` use `try_send` on the controller's operator channel.
5. bridge.rs: `pub async fn shutdown(&mut self, app: &mut App)` per §9.11.9; `run_tui` calls it after the loop, before returning.
6. Tests (bridge::tests):
   - `run_build_failure_reported_via_poll` (`#[tokio::test]`): `make_backends` returns `Err(Error::Validation { field: "x".into(), message: "no backends".into() })`; `handle(&Run, ..)` returns `Ok(())` and `app.status == Running`; poll `poll_finished` (with `tokio::time::sleep(10 ms)` between, at most 2 s) until it returns `true`; then `app.status == Error` and `status_message` contains `"no backends"`.
   - `interrupt_returns_without_waiting` (`#[tokio::test]`): mock driver with `set_delay(Duration::from_secs(10))`; after `Run` and the first `RoundStarted` event, `tokio::time::timeout(Duration::from_millis(100), controller.handle(&Interrupt, &mut app))` completes; then `shutdown` completes within 5 s and the saved checkpoint's step 1 is `InProgress`.
   - Update `interrupt_cancels_and_saves_checkpoint` and any other test that relied on `Interrupt` awaiting the task to call `shutdown` after it. No assertion is weakened.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub async fn shutdown' crates/orrery-tui/src/bridge.rs && grep -q 'controller.shutdown(&mut app).await' crates/orrery-tui/src/bridge.rs && grep -q 'try_send' crates/orrery-tui/src/bridge.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^bridge::tests::(run_build_failure_reported_via_poll|interrupt_returns_without_waiting): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 47: TUI Operator Answers in Every Mode and After Restart

* **Recommended Command:** `/boost`

Scope note: `approve_visible()` requires `GateAndOperator`, so in `Agents` mode (the default 3-stage workflow) a TUI user can't answer an escalation, exhausted retries or a `Halt` hook. A step left awaiting by `orrery run` also shows no Approve/Reject until Run is pressed, and Approve then has no engine to talk to. `approve_hidden_unless_gate_and_operator` is replaced, not weakened: its new form asserts the new rule. The same goes for `widgets::tests::approve_reject_only_when_awaiting_gate_and_operator` (added 2026-11-16 after the first run halted on it).

```text
[STEP_ID]
STEP 47: app.rs + bridge.rs + widgets.rs — approve_visible = awaiting_operator in every mode; load_initial_app restores the awaiting state; Approve/Reject without a running engine save operator_decision and start the engine

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/app.rs
  - crates/orrery-tui/src/bridge.rs
  - crates/orrery-tui/src/widgets.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1268-1272 (§9.11.10; binding)
  - crates/orrery-tui/src/app.rs:204-207 (approve_visible), 1532-1575 (approve_hidden_unless_gate_and_operator)
  - crates/orrery-tui/src/widgets.rs:1107-1133 (approve_reject_only_when_awaiting_gate_and_operator)
  - crates/orrery-tui/src/bridge.rs load_initial_app, EngineController::handle (Approve, Reject, Run)
  - crates/orrery-cli/src/commands.rs approve/reject handlers (the same checkpoint write)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. AppAction and Modal are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub async fn shutdown' crates/orrery-tui/src/bridge.rs && grep -q 'fn approve_hidden_unless_gate_and_operator' crates/orrery-tui/src/app.rs && sed -n 1268p $S | grep -q '^#### 9.11.10 '

[MUTATION_SPEC]
1. app.rs: `approve_visible` returns `self.awaiting_operator`. Drop the now-unused imports.
2. app.rs: rename `approve_hidden_unless_gate_and_operator` to `approve_visible_whenever_awaiting`: with the default (`Agents`) config, `awaiting_operator = true` → visible, footer Approve → `[Approve]`, Reject opens `RejectFeedback`; `awaiting_operator = false` → hidden and both footer buttons do nothing.
3. bridge.rs: `load_initial_app` restores the awaiting state per §9.11.10.
4. bridge.rs: `handle(Approve | Reject)` with no running task per §9.11.10.
5. Tests (bridge::tests):
   - `initial_app_shows_awaiting_step`: a project whose saved checkpoint has step 1 `AwaitingOperator` with `awaiting_reason = Some("escalated")` → `load_initial_app` gives `status == AwaitingOperator`, `awaiting_operator`, `status_message == Some("escalated")`.
   - `approve_without_running_engine_resumes` (`#[tokio::test]`): that checkpoint plus a mock driver scripted to pass step 1; `handle(&Approve, ..)`; `shutdown` → the saved checkpoint has step 1 `Passed` and `operator_decision == None`.
   - `approve_without_awaiting_step_sets_message` (`#[tokio::test]`): a checkpoint with step 1 `Pending` → `handle(&Approve, ..)` is `Ok`, `status_message == Some("no step awaiting operator")`, no engine started.
6. widgets.rs (tests only; no non-test code changes): rename `approve_reject_only_when_awaiting_gate_and_operator` to `approve_reject_whenever_awaiting`. Keep its idle block (`GateAndOperator`, not awaiting → no `[Approve]`/`[Reject]`, `[Mouse: OFF]`) and its `GateAndOperator` + awaiting block (both shown). The `Agents` + awaiting block now asserts `[Approve]` and `[Reject]` are present. Add one block: `Agents`, `awaiting_operator = false`, `status = Idle` → neither shown.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'approve_hidden_unless_gate_and_operator' crates/orrery-tui/src/app.rs && ! sed -n '/pub fn approve_visible/,/^    }/p' crates/orrery-tui/src/app.rs | grep -q 'GateAndOperator' && grep -q 'no step awaiting operator' crates/orrery-tui/src/bridge.rs && ! grep -q 'approve_reject_only_when_awaiting_gate_and_operator' crates/orrery-tui/src/widgets.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(app::tests::approve_visible_whenever_awaiting|bridge::tests::(initial_app_shows_awaiting_step|approve_without_running_engine_resumes|approve_without_awaiting_step_sets_message)|widgets::tests::approve_reject_whenever_awaiting): test$')" -eq 5

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 48: TUI Cycles Entry, Loop Progress, Ingest Marker

* **Recommended Command:** `/goal`

Scope note: clicking `[Cycles]` does nothing (it routes to `Enter`, which has no Cycles arm), `[Passed: X/Y]` never shows Loop progress toward `cycle_count`, and the Ingest modal can't set the step marker (`orrery ingest --marker` can). `AppAction::IngestPlan(PathBuf)` stays as it is: the marker travels in `app.config`, which `IngestPlan` already parses with and saves, so `tui_action_parity` in the CLI crate still compiles.

```text
[STEP_ID]
STEP 48: app.rs + widgets.rs + bridge.rs — Modal::CyclesEntry opened by Enter/click on Cycles in Loop mode; Ingest modal with a marker field (Tab switches); App::run_passed drives [Passed: X/cycles] in Loop mode; failed ingest restores the saved marker

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/app.rs
  - crates/orrery-tui/src/widgets.rs
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1273-1282 (§9.11.11; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1045-1120 (§9.9 key table and Modal)
  - crates/orrery-tui/src/app.rs:80-95 (Modal), 336-380 (text modals), 579-630 (handle_top_menu_key), 231-306 (apply_engine_event)
  - crates/orrery-tui/src/widgets.rs:151-200 (render_top_menu), 556-635 (render_modal)
  - crates/orrery-tui/src/bridge.rs EngineController::handle (IngestPlan, Run)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. AppAction is unchanged. Top-menu label widths are unchanged (layout.rs is not touched).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'fn approve_visible_whenever_awaiting' crates/orrery-tui/src/app.rs && ! grep -q 'CyclesEntry' crates/orrery-tui/src/app.rs && sed -n 1273p $S | grep -q '^#### 9.11.11 '

[MUTATION_SPEC]
1. app.rs: `Modal::CyclesEntry { input: String }` and `Modal::Ingest { input, marker, marker_focused }` with the key handling of §9.11.11; Enter/Space on Cycles in Loop mode opens `CyclesEntry`.
2. app.rs: `pub run_passed: usize` (0 in `new`), incremented on `StepStatusChanged { status: Passed }`.
3. widgets.rs: the two modals' rendering and the `[Passed: X/Y]` rule of §9.11.11.
4. bridge.rs: `handle(Run)` sets `app.run_passed = 0` when it starts a run; `IngestPlan` restores `app.config.step_marker` from the saved project when `from_file` fails.
5. Update existing tests that build `Modal::Ingest { input }` to the new shape (`marker` = the config's marker, `marker_focused: false`).
6. Tests:
   - `app::tests::cycles_entry_sets_cycles`: Loop mode, top menu index 2, `Enter` opens `CyclesEntry { input: "0" }`; `Backspace`, type `"12"`, `Enter` → `[SetCycles(12)]`, `config.cycle_count == 12`, modal closed.
   - `app::tests::cycles_entry_ignored_in_run_mode`: Run mode, index 2, `Enter` → `[]`, `modal == Modal::None`.
   - `app::tests::ingest_modal_edits_marker`: open Ingest, type `"plan.md"`, `Tab`, clear the marker with `Backspace`es, type `"### STEP "`, `Enter` → `[IngestPlan("plan.md")]` and `config.step_marker == "### STEP "`.
   - `app::tests::ingest_empty_marker_rejected`: marker cleared to `"   "` → `Enter` gives `[]`, modal still `Ingest`, `status_message == Some("step marker must not be empty")`.
   - `app::tests::run_passed_counts_passed_events`: two `StepStatusChanged { status: Passed }` events → `run_passed == 2`.
   - `widgets::tests::passed_counter_shows_loop_progress`: Loop mode, `cycle_count = 5`, `run_passed = 2` → the rendered top menu contains `"[Passed: 2/5]"`; Run mode → `"[Passed: <passed>/<total>]"` as before.
   - `bridge::tests::failed_ingest_restores_marker` (`#[tokio::test]`): saved marker `"## Step "`; set `app.config.step_marker = "@@ "` and handle `IngestPlan("missing.md")` → `Err`, and `app.config.step_marker == "## Step "`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'CyclesEntry' crates/orrery-tui/src/app.rs && grep -q 'marker_focused' crates/orrery-tui/src/widgets.rs && grep -q 'pub run_passed: usize' crates/orrery-tui/src/app.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(app::tests::(cycles_entry_sets_cycles|cycles_entry_ignored_in_run_mode|ingest_modal_edits_marker|ingest_empty_marker_rejected|run_passed_counts_passed_events)|widgets::tests::passed_counter_shows_loop_progress|bridge::tests::failed_ingest_restores_marker): test$')" -eq 7

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 49: TUI Settings Shows Warnings and Agent Details

* **Recommended Command:** `/goal`

Scope note: spec §4.1 says validation warnings show "in CLI/TUI", and §5.7.2 says the AgentCli guardrail note shows in the TUI agent details. Neither happens: Settings lists agent names only.

```text
[STEP_ID]
STEP 49: app.rs + widgets.rs — App::warnings refreshed on load and after config edits; Settings lists warnings and per-agent details with AGENT_CLI_GUARDRAIL_NOTE; Settings scrolls with Up/Down

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/app.rs
  - crates/orrery-tui/src/widgets.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1283-1288 (§9.11.12; binding)
  - crates/orrery-tui/src/app.rs App::new, the config-edit arms that set status_message on Err, the Help/Settings modal keys (~428)
  - crates/orrery-tui/src/widgets.rs render_settings_modal (~637-705)
  - crates/orrery-core/src/config.rs ProjectConfig::validate (Vec<Warning>), Warning, AGENT_CLI_GUARDRAIL_NOTE
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Settings stays read-only.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'CyclesEntry' crates/orrery-tui/src/app.rs && ! grep -q 'settings_scroll' crates/orrery-tui/src/app.rs && sed -n 1283p $S | grep -q '^#### 9.11.12 '

[MUTATION_SPEC]
1. app.rs: `pub warnings: Vec<String>`, `pub settings_scroll: u16`, `pub fn refresh_warnings(&mut self)` per §9.11.12, called at the end of `new` and after each config edit that succeeds.
2. app.rs: Settings modal keys per §9.11.12 (Help keeps its keys).
3. widgets.rs: `render_settings_modal` per §9.11.12 (title, line order, detail lines, guardrail note in the alert style, wrap, scroll).
4. Tests:
   - `app::tests::warnings_refreshed_after_edit`: a config whose roles have no validator and `gate_tools` empty → `warnings` non-empty after `new`; set `validation_mode` to `Agents` through the same path the edits use → `refresh_warnings` runs and `warnings` changes accordingly (assert the exact count before and after).
   - `app::tests::settings_scroll_keys`: open Settings; `Down` ×3 → 3; `Up` → 2; `Esc` closes and resets to 0.
   - `widgets::tests::settings_shows_agent_details_and_guardrail`: one `AgentCli` agent (`image = "registry.example.com/agents/cli@sha256:0000"`) and one `LocalModel` → the rendered buffer contains `"image registry.example.com/agents/cli@sha256:0000"`, `"inference "` and the first 40 chars of `AGENT_CLI_GUARDRAIL_NOTE`.
   - `widgets::tests::settings_title_counts_warnings`: two warnings → the title contains `"2 warning(s)"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn refresh_warnings' crates/orrery-tui/src/app.rs && grep -q 'AGENT_CLI_GUARDRAIL_NOTE' crates/orrery-tui/src/widgets.rs && grep -q 'settings_scroll' crates/orrery-tui/src/widgets.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(app::tests::(warnings_refreshed_after_edit|settings_scroll_keys)|widgets::tests::(settings_shows_agent_details_and_guardrail|settings_title_counts_warnings)): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 50: Live Tests for CloudApi, AgentCli and a Real-Backends Run

* **Recommended Command:** `/goal`

Scope note: no live test covers `CloudApi` with a real keychain key, `CliContainerDriver` against real Docker, or `build_engine` over `Backends::real`. These are written and compiled only; the operator runs them (Rules: live-backend tests).

```text
[STEP_ID]
STEP 50: tests/live_llm.rs, tests/live_docker.rs, tests/live_engine.rs (NEW) — live_cloud_api_turn, live_cli_driver_reads_report, live_engine_run_with_real_backends; Cargo [[test]] entry for live_engine

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/tests/live_llm.rs
  - crates/orrery-core/tests/live_docker.rs
  - crates/orrery-core/tests/live_engine.rs (NEW)
  - crates/orrery-core/Cargo.toml (one [[test]] entry only)
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1289-1294 (§9.11.13; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:621-644 (§8: live-test rules; a missing var panics with a clear message)
  - crates/orrery-core/tests/live_llm.rs (live_single_turn, the pattern to copy), crates/orrery-core/tests/live_docker.rs
  - crates/orrery-core/src/config.rs ConfigStore (init_project, save_project, save_agent), AgentBackend
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No live test runs in the default gate.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn refresh_warnings' crates/orrery-tui/src/app.rs && test ! -e crates/orrery-core/tests/live_engine.rs && sed -n 1289p $S | grep -q '^#### 9.11.13 '

[MUTATION_SPEC]
1. Cargo.toml: `[[test]]` `name = "live_engine"`, `required-features = ["live-backends"]`, after `live_llm`.
2. live_llm.rs `live_cloud_api_turn`: as `live_single_turn`, with a `CloudApi` agent (`endpoint_url` = `ORRERY_LIVE_CLOUD_URL`, `model` = `ORRERY_LIVE_CLOUD_MODEL`, `max_context_tokens: 16384`, `keychain_service: "orrery.test.cloud"`, `keychain_account: "testuser"`) and `OsKeyringStore::new()` as the secret store.
3. live_docker.rs `live_cli_driver_reads_report`: `ORRERY_LIVE_IMAGE`; `CliContainerDriver::new(Arc::new(BollardContainerBackend::connect()?), Arc::new(InMemorySecretStore::new()), tempdir)`; agent `AgentCli { image, command: ["sh", "-c", "cat >/dev/null; printf 'status = \"Complete\"\\nsummary = \"live cli\"\\n' > /orrery/out/report.toml"], workspace_mount: None, memory_limit_mb: 256, timeout_secs: 120, no keychain, no secret_env }`; an Implementer role → report `status == Complete`, `summary == "live cli"`; afterwards `inspect(cli_container_name(..)) == ContainerState::Missing`.
4. live_engine.rs `live_engine_run_with_real_backends` per §9.11.13: `ConfigStore::new(tmp, None)`, `init_project("live")`, write `plan.md` = `"## Step 1: trivial\nCall submit_report now.\n"`, set `plan_path`, `max_step_retries = 1`, one `External` inference (`ORRERY_LIVE_LLM_URL`, trimmed as `live_single_turn` does), one `LocalModel` agent `live-lm` on all three stages; `build_engine(&store, Backends::real(tmp)?)`; `tokio::time::timeout(600 s, engine.run(RunOptions { wait_for_operator: false, ..Default::default() }, CancellationToken::new()))`; `Ok(Ok(_))` passes, anything else panics with the error.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy --workspace --all-targets --features orrery-core/live-backends --locked -- -D warnings && cargo test --workspace --locked && cargo test --workspace --locked --features orrery-core/live-backends --no-run && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn live_cloud_api_turn' crates/orrery-core/tests/live_llm.rs && grep -q 'fn live_cli_driver_reads_report' crates/orrery-core/tests/live_docker.rs && grep -q 'fn live_engine_run_with_real_backends' crates/orrery-core/tests/live_engine.rs && grep -q 'name = "live_engine"' crates/orrery-core/Cargo.toml

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 51: Operator README

* **Recommended Command:** `/goal`

Scope note: the repo has the spec and this plan, and nothing an operator reads first. The README describes what exists after Step 50, so it is the last step.

```text
[STEP_ID]
STEP 51: README.md (NEW) — requirements, build, quickstart, TUI keys, .orrery layout, security notes, live tests

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md (NEW)
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1295-1296 (§9.11.14; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:125-295 (§4 layout and schemas), 559-573 (§6.4 keys), 581-620 (§7 CLI), 621-644 (§8), 414-416 (§5.7.2)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No host paths, real hostnames or real accounts (§8 fixtures rule).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'fn live_engine_run_with_real_backends' crates/orrery-core/tests/live_engine.rs && test ! -e README.md && sed -n 1295p $S | grep -q '^#### 9.11.14 '

[MUTATION_SPEC]
1. README.md with these `##` sections, in order: `Requirements`, `Build`, `Quickstart`, `TUI keys`, `Project layout`, `Security`, `Live-backend tests`. Content per §9.11.14; every command is one that §7 lists, with flags as §7 spells them.
2. Quickstart uses `my-service`, `coder-14b`, `http://127.0.0.1:8080` and `local-coder` (the §4 examples), and ends with `orrery status`.
3. `Live-backend tests` includes the `live_docker` setup from "User Step: LIVE-1" verbatim: the `busybox:1.37` pull, the `docker create --name orrery-live-test ...` line with `trap exit TERM`, and why the trap is there.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && test "$(grep -cE '^## (Requirements|Build|Quickstart|TUI keys|Project layout|Security|Live-backend tests)$' README.md)" -eq 7 && grep -q 'orrery-core/live-backends' README.md && grep -q 'orrery init' README.md && grep -q 'trap exit TERM' README.md && grep -q 'ORRERY_LIVE_CONTAINER=orrery-live-test' README.md && ! grep -qE '/(home|srv|Users|root)/' README.md

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase E — parked fixes and doc accuracy (Steps 52–57)

**Phase goal (frozen until the phase ends):** fix the six parked Phase D findings the operator picked (2026-11-16: the zeroized Bearer copy, hidden MCP tools, `logs --follow` on split UTF-8, log write errors, the TUI operator forwarder, Loop progress after Approve/Reject), and make the README state only what has been verified, platform support above all. Design: spec §9.12 (`ORRERY_TECHNICAL_SPECIFICATION.md:1298-1328`).

Written 2026-11-16 against the Step 51 tree (`8ed077a`). Line numbers in `READ_ONLY_REFS` are from that tree. Steps 54 and 55 both edit `stream_log.rs`, so Step 55 names functions as well as lines.

**Decisions the contract leaves open** (settled here, not by the executor):
- Platform: Linux x86_64 is the only tested target. macOS and Windows are planned but not in this phase; docs say "not supported yet" (§9.12.1, operator 2026-11-16).
- A log write error is reported once per `kept_error` episode: once ever in production, where nothing calls `flush()`, and again after each `flush()` in tests (§9.12.5).
- Step 52 adds no test: zeroization on drop can't be observed from safe code. `bearer_only_for_cloud_api` keeps the header value covered.

### STEP 52: Zeroized Bearer Header Copy

* **Recommended Command:** `/goal`

Scope note: Parked from Step 40. `auth_header_value` formats `"Bearer {key}"` into a plain `String` that is dropped without being wiped.

```text
[STEP_ID]
STEP 52: llm.rs — auth_header_value builds the Bearer string in zeroize::Zeroizing<String>

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1306-1307 (§9.12.2; binding)
  - crates/orrery-core/src/llm.rs:611-633 (auth_header_value), the test bearer_only_for_cloud_api
  - crates/orrery-core/Cargo.toml (zeroize is already a dependency)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The auth_header_value signature, its error mapping and set_sensitive(true) stay as they are. No test is added or changed.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'Zeroizing' crates/orrery-core/src/llm.rs && grep -q 'from_str(&format!("Bearer {}", secret.expose()))' crates/orrery-core/src/llm.rs && sed -n 1306p $S | grep -q '^#### 9.12.2 '

[MUTATION_SPEC]
1. llm.rs auth_header_value, CloudApi arm: `let bearer = zeroize::Zeroizing::new(format!("Bearer {}", secret.expose()));` then `reqwest::header::HeaderValue::from_str(&bearer)` with today's `map_err`. Nothing else changes.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'Zeroizing::new(format!("Bearer {}", secret.expose()))' crates/orrery-core/src/llm.rs && ! grep -q 'from_str(&format!("Bearer' crates/orrery-core/src/llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -c '^llm::tests::bearer_only_for_cloud_api: test$')" -eq 1

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 53: Report Hidden MCP Tools

* **Recommended Command:** `/goal`

Scope note: Parked from Step 42. A `<server>:*` role pattern silently drops a tool whose name can't map to a function name, so the operator never learns it's unavailable.

```text
[STEP_ID]
STEP 53: mcp.rs + engine.rs — McpGatekeeper::catalog accessor; build_engine emits one notice per unmappable catalog tool

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1309-1311 (§9.12.3; binding)
  - crates/orrery-core/src/mcp.rs:122-198 (McpGatekeeper, refresh_catalog, is_tool_allowed, tools_for_role), 270-290 (MockMcpTransport::new, with_tool), 656-674 (MockMcpConnector)
  - crates/orrery-core/src/config.rs:1269-1285 (format_mapped_tool_name), 1341-1361 (allowed chars [A-Za-z0-9_-])
  - crates/orrery-core/src/engine.rs:1993-2025 (build_engine connect loop through refresh_catalog), 5550-5595 (test build_engine_skips_unused_server, the pattern to copy)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. is_tool_allowed, tools_for_role and the catalog field stay as they are.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'pub fn catalog(&self)' crates/orrery-core/src/mcp.rs && grep -q 'gatekeeper.refresh_catalog().await?;' crates/orrery-core/src/engine.rs && sed -n 1309p $S | grep -q '^#### 9.12.3 '

[MUTATION_SPEC]
1. mcp.rs: `impl McpGatekeeper` gains `pub fn catalog(&self) -> &[McpToolSchema] { &self.catalog }`.
2. engine.rs build_engine: right after `gatekeeper.refresh_catalog().await?;`, loop `for tool in gatekeeper.catalog()`; where `format_mapped_tool_name(&tool.server_id, &tool.name).is_err()`, `stream.emit(StreamEvent::notice(format!("MCP tool '{}:{}' is hidden: its name can't map to a function name", tool.server_id, tool.name)))?;`. Import `format_mapped_tool_name` from `crate::config` if it isn't already.
3. Tests:
   - `mcp::tests::catalog_returns_unfiltered_tools` (`#[tokio::test]`): register a `MockMcpTransport::new("fs")` with tools `read_file` and `read.file`; `refresh_catalog().await`; `catalog()` has 2 entries with names `["read_file", "read.file"]` in that order.
   - `engine::tests::build_engine_reports_hidden_tool` (`#[tokio::test]`): copy `build_engine_skips_unused_server`, with the coder role's `allowed_mcp_tools = vec!["fs:*".to_string()]`, only the `fs` server saved, and the `fs` mock having tools `read_file` and `read.file`. After `build_engine`, `engine.ctx.stream.recent(20)` has exactly 1 `SystemNotice` with text `"MCP tool 'fs:read.file' is hidden: its name can't map to a function name"`, and no event's text contains `"'fs:read_file' is hidden"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn catalog(&self) -> &\[McpToolSchema\]' crates/orrery-core/src/mcp.rs && grep -q "is hidden: its name can't map to a function name" crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(mcp::tests::catalog_returns_unfiltered_tools|engine::tests::build_engine_reports_hidden_tool): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 54: Byte-Safe Log Follower

* **Recommended Command:** `/goal`

Scope note: Parked from Step 45. If a `poll` lands in the middle of a multi-byte UTF-8 character, `read_to_string` fails with `InvalidData` and `orrery logs --follow` exits on an I/O error.

```text
[STEP_ID]
STEP 54: stream_log.rs — LogFollower.partial is Vec<u8>; complete lines decode with from_utf8_lossy

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/stream_log.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1313-1314 (§9.12.4; binding)
  - crates/orrery-core/src/stream_log.rs:334-437 (LogFollower, new, poll), 948-970 (test follower_keeps_partial_line, the pattern to copy)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. LogFollower::new and poll keep their signatures; the rotation and inode logic in poll stays as it is. LogWriter and LiveStreamHub are untouched (Step 55).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '    partial: String,' crates/orrery-core/src/stream_log.rs && sed -n 1313p $S | grep -q '^#### 9.12.4 '

[MUTATION_SPEC]
1. stream_log.rs: `LogFollower.partial: Vec<u8>`; both constructors in `new` use `Vec::new()`.
2. poll: both `file.read_to_string(&mut self.partial)` calls become `file.read_to_end(&mut self.partial)` with the same error mapping.
3. poll line split: `while let Some(pos) = self.partial.iter().position(|&b| b == b'\n')`; take `let raw: Vec<u8> = self.partial.drain(..=pos).collect();`, decode `String::from_utf8_lossy(&raw[..pos])`, trim a trailing `'\r'` as now, and push it as an owned `String`.
4. Test `stream_log::tests::follower_handles_split_utf8`: copy `follower_keeps_partial_line`; append `b"first\nsplit \xC3"` → `poll()` is `Ok(vec!["first".to_string()])`; append `b"\xA9 done\n"` → `poll()` is `Ok(vec!["split é done".to_string()])`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '    partial: Vec<u8>,' crates/orrery-core/src/stream_log.rs && ! grep -q 'read_to_string(&mut self.partial)' crates/orrery-core/src/stream_log.rs && grep -q 'from_utf8_lossy' crates/orrery-core/src/stream_log.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -c '^stream_log::tests::follower_handles_split_utf8: test$')" -eq 1

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 55: Surface Live-Stream Log Write Errors

* **Recommended Command:** `/goal`

Scope note: Parked from Step 41. `LogWriter` keeps a write error only for `flush()`, and nothing in production calls `flush()`, so a full disk or a permissions error is never seen.

```text
[STEP_ID]
STEP 55: stream_log.rs — LogWriter shares its first write error with the hub; the next emit broadcasts one Error event

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/stream_log.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1316-1319 (§9.12.5; binding), 1202-1215 (§9.11.4)
  - crates/orrery-core/src/stream_log.rs (8ed077a lines; Step 54 edits only LogFollower, below these): 29-49 (StreamEvent, notice), 74-121 (LogWriter, record_error, write_line), 123-232 (LiveStreamHub, new, emit), 845-862 (test flush_reports_write_error, the pattern to copy)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The signatures of emit, flush and LiveStreamHub::new stay as they are, and so does flush's behavior. The Error event is never sent to the writer.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '    partial: Vec<u8>,' crates/orrery-core/src/stream_log.rs && ! grep -q 'write_error: Arc<Mutex' crates/orrery-core/src/stream_log.rs && sed -n 1316p $S | grep -q '^#### 9.12.5 '

[MUTATION_SPEC]
1. stream_log.rs: `LogWriter` gains `shared_error: Arc<Mutex<Option<String>>>`. In `record_error`, inside `if self.kept_error.is_none()`, after setting `kept_error`, also set the shared slot to the same message: `if let Ok(mut slot) = self.shared_error.lock() { *slot = Some(msg.clone()); }`. A poisoned lock here is skipped, because the writer thread has no caller to return an error to.
2. `LiveStreamHub` gains `write_error: Arc<Mutex<Option<String>>>`. `new` creates it, gives a clone to the writer as `shared_error`, and stores it.
3. emit: after `let _ = self.sender.send(event);`, take the slot (poisoned lock → `Error::Security { message: "LiveStreamHub lock poisoned".into() }`). On `Some(msg)`, build `StreamEvent { timestamp: Utc::now(), step_number: None, stage_index: None, agent_id: None, kind: StreamEventKind::Error, text: self.redactor.redact(&format!("live stream log write failed: {msg}"))? }`, push it to the ring with the same capacity rule as the main event, and broadcast it. Don't send it to the writer.
4. Test `stream_log::tests::write_error_broadcasts_once`: copy `flush_reports_write_error`'s setup (directory at `LOG_FILE`). Emit notice "a"; `hub.flush()` is `Err` (this also syncs with the writer); emit notice "b"; `hub.recent(20)` has exactly 1 event with `kind == StreamEventKind::Error` whose text starts with `"live stream log write failed: "`.
5. `flush_reports_write_error` stays unchanged and passing.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'shared_error: Arc<Mutex<Option<String>>>' crates/orrery-core/src/stream_log.rs && grep -q 'write_error: Arc<Mutex<Option<String>>>' crates/orrery-core/src/stream_log.rs && grep -q 'live stream log write failed: {msg}' crates/orrery-core/src/stream_log.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^stream_log::tests::(write_error_broadcasts_once|flush_reports_write_error): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 56: TUI Operator Forwarder and Loop Progress Reset

* **Recommended Command:** `/goal`

Scope note: Parked from Steps 46 and 48. The run task's `eng_op_tx.send(dec).await` can stall the same `select!` that polls `engine.run`. And `run_passed` resets only on `Run`, so a run started by Approve or Reject overstates `[Passed: X/cycles]`.

```text
[STEP_ID]
STEP 56: bridge.rs — forward_operator_decision uses try_send with a dropped notice; run_passed reset moves into start_run

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1321-1323 (§9.12.6; binding)
  - crates/orrery-tui/src/bridge.rs:224-300 (start_run and the run task's select!), 365-371 (handle Run), 383-389 (the existing "operator decision dropped" message), 405-440 (flush_dropped_notice, forward_stream_event: helper style to copy), 1529-1641 (test approve_without_running_engine_resumes)
  - crates/orrery-core/src/engine.rs:338-340 (operator_sender returns tokio::sync::mpsc::Sender<OperatorDecision>)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other select! arm changes; the 8-slot channel sizes stay as they are.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'let _ = eng_op_tx.send(dec).await;' crates/orrery-tui/src/bridge.rs && ! grep -q 'fn forward_operator_decision' crates/orrery-tui/src/bridge.rs && sed -n 1321p $S | grep -q '^#### 9.12.6 '

[MUTATION_SPEC]
1. bridge.rs: add a free function `fn forward_operator_decision(eng_op_tx: &mpsc::Sender<OperatorDecision>, stream_tx: &mpsc::Sender<StreamEvent>, dec: OperatorDecision)` next to `flush_dropped_notice`. It matches `eng_op_tx.try_send(dec)`: `Ok(())` and `Err(Closed(_))` do nothing; `Err(Full(_))` runs `let _ = stream_tx.try_send(StreamEvent::notice("operator decision dropped: channel full"));`.
2. The run task's `Some(dec) =>` arm becomes `forward_operator_decision(&eng_op_tx, &stream_tx, dec);` with no `.await`.
3. Move `app.run_passed = 0;` from `handle(AppAction::Run)` into `start_run`, directly after `app.awaiting_operator = false;`.
4. Tests (in `mod tests`):
   - `forward_operator_decision_delivers` (`#[tokio::test]`): `mpsc::channel(1)` for each of the operator and stream channels; call once with `Approve`; the operator receiver gets `Approve`, and `stream_rx.try_recv()` is `Err`.
   - `forward_operator_decision_full_sends_notice` (`#[tokio::test]`): fill the 1-slot operator channel with `Approve`, then call with `Reject { feedback: "x".into() }`; the stream receiver gets one `SystemNotice` with text `"operator decision dropped: channel full"`, and the operator channel still holds only the first `Approve`.
   - `approve_without_running_engine_resumes`: set `app.run_passed = 5;` right after `load_initial_app(..)`, and at the end assert `app.run_passed == 1`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'eng_op_tx.send(dec).await' crates/orrery-tui/src/bridge.rs && grep -q 'fn forward_operator_decision' crates/orrery-tui/src/bridge.rs && test "$(grep -c 'app.run_passed = 0;' crates/orrery-tui/src/bridge.rs)" -eq 1 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^bridge::tests::(forward_operator_decision_delivers|forward_operator_decision_full_sends_notice|approve_without_running_engine_resumes): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 57: README States Only What Is Verified

* **Recommended Command:** `/goal`

Scope note: Checked 2026-11-16 against the code and the live-test record, the README claims more than has been verified. It lists aarch64 as supported, but only `cargo deny` sees that target. It lists rootless Docker and Podman, which have never been tested. It gives the Rust version as "stable supporting Edition 2024", while the workspace pins `rust-version = "1.99.1"`. And it says arrow keys scroll Help, which doesn't scroll (`app.rs` `Modal::Help` handles only `Esc`/`Enter`/`q`).

```text
[STEP_ID]
STEP 57: README.md — platform, Rust and Docker lines match §9.12.1; TUI key rows match the Help/Settings modal code

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1300-1304 (§9.12.1), 1325-1328 (§9.12.7; binding)
  - README.md:5-12 (Requirements), 73-90 (TUI keys)
  - Cargo.toml:7-8 (edition, rust-version), deny.toml:35 (targets)
  - crates/orrery-tui/src/app.rs:505-535 (Modal::Help and Modal::Settings key handling)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The seven `##` sections, the Quickstart, Project layout, Security and Live-backend tests stay word for word. No host paths, real hostnames or real accounts.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'aarch64-unknown-linux-gnu' README.md && grep -q 'Podman' README.md && sed -n 1325p $S | grep -q '^#### 9.12.7 '

[MUTATION_SPEC]
1. Requirements, the OS line: Linux on x86_64 (`x86_64-unknown-linux-gnu`) is the tested platform. `aarch64-unknown-linux-gnu` is dependency-checked by `cargo deny` only and is untested. macOS and Windows are not supported yet (planned).
2. Requirements, the Rust line: "Rust 1.99.1 or newer (workspace `rust-version`), edition 2024."
3. Requirements, the Docker line: Docker Engine (used by the live-backend tests). Orrery honors `DOCKER_HOST`; rootless Docker and Podman are untested.
4. TUI keys: the arrow-key row says arrows scroll the Settings modal (not Help). The `Esc` row, or a new row, says that in Help and Settings, `Esc`, `Enter` or `q` close the modal.
5. No other line changes.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && test "$(grep -cE '^## (Requirements|Build|Quickstart|TUI keys|Project layout|Security|Live-backend tests)$' README.md)" -eq 7 && grep -q '1.99.1' README.md && grep -qi 'macOS and Windows are not supported yet' README.md && grep -qi 'untested' README.md && ! grep -qE 'modal \(`Settings`, `Help`\)' README.md && grep -q 'trap exit TERM' README.md && ! grep -qE '/(home|srv|Users|root)/' README.md && test "$(git diff --numstat -- README.md | awk '{print $1}')" -le 8

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase F — README keys and live verification (Step 58, User Step LIVE-2)

**Phase goal (frozen until the phase ends):** the README's TUI keys table lists every key `app.rs` handles in each panel, so keyboard-only operation is documented as base function; and the operator runs the live-backend suite against Steps 50–57, since nothing from that range has touched a real backend yet. Design: spec §9.13 (`ORRERY_TECHNICAL_SPECIFICATION.md:1330-1341`).

Written 2026-11-16 against `7dba031`. LIVE-2 and Step 58 don't depend on each other. Any LIVE-2 failure is parked and reviewed at phase end, like any side finding.

### User Step: LIVE-2 — live-backend suite after Phase E (COMPLETED 2026-11-16)
Done by the operator; every live test passed. Values: `qwen25-coder-3b-q4` on `127.0.0.1:8080` for `ORRERY_LIVE_LLM_*` and for `ORRERY_LIVE_CLOUD_*` (the CloudApi path against a local OpenAI-compatible server, not a real cloud provider); `npx -y @modelcontextprotocol/server-everything` for `ORRERY_LIVE_MCP_CMD`; busybox and `orrery-live-test` for Docker. The only warnings came from npm deprecation and update notices.
The operator runs this; it needs their endpoints and API key. It is the first real run of `live_cloud_api_turn`, `live_cli_driver_reads_report` and `live_engine_run_with_real_backends` (Step 50). It also exercises the Step 52 Bearer path, the Step 53 hidden-tool scan and the Step 55 log path.
```bash
cargo run -p orrery-cli --locked -- secret set --service orrery.test.cloud --account testuser
docker inspect orrery-live-test >/dev/null 2>&1 || docker create --name orrery-live-test busybox:1.37 sh -c 'trap exit TERM; while :; do sleep 1; done'
# Fill in the five quoted values first (no angle brackets: bash reads <x> as a redirect).
export ORRERY_LIVE_MCP_CMD="FILL_ME"        # stdio MCP server command line; split on whitespace
export ORRERY_LIVE_IMAGE=busybox:1.37
export ORRERY_LIVE_CONTAINER=orrery-live-test
export ORRERY_LIVE_LLM_URL="FILL_ME"        # llama-server base URL, e.g. http://127.0.0.1:8080
export ORRERY_LIVE_LLM_MODEL="FILL_ME"
export ORRERY_LIVE_CLOUD_URL="FILL_ME"      # full chat-completions URL
export ORRERY_LIVE_CLOUD_MODEL="FILL_ME"
cargo test --workspace --locked --features orrery-core/live-backends
```
Report: pass/fail per test, and the panic text of any failure. `live_engine_run_with_real_backends` can take up to 600 s.

### STEP 58: README TUI Keys Per Panel

* **Recommended Command:** `/goal`

Scope note: Found validating Step 57. The README folds all four arrows into one vague row, so `Left`/`Right` look unsupported even though TopMenu, StageCanvas and FooterMenu use them. It also leaves out LiveStream's `PageUp`/`PageDown`/`End`, StageCanvas `Delete` and the quit-confirmation keys.

```text
[STEP_ID]
STEP 58: README.md — TUI keys table gets one arrow row per focus area plus LiveStream paging, StageCanvas Delete and quit-confirm keys

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1332-1341 (§9.13.1; binding)
  - README.md:73-90 (TUI keys)
  - crates/orrery-tui/src/app.rs:19 (STREAM_PAGE_STEP), 459-505 (Selector and QuitConfirm modals), 516-533 (Settings modal), 682-692 (handle_top_menu_key), 749-830 (handle_steps_panel_key, handle_stage_canvas_key incl. Delete), 879-903 (handle_live_stream_key), 905-916 (handle_footer_menu_key)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Only the TUI keys table changes. The seven `##` sections and every other section stay word for word. No host paths, real hostnames or real accounts.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^| `Up` / `Down` / `Left` / `Right` |' README.md && ! grep -q 'PageUp' README.md && sed -n 1332p $S | grep -q '^#### 9.13.1 '

[MUTATION_SPEC]
1. README.md TUI keys table: replace the `| `Up` / `Down` / `Left` / `Right` | … |` row with one row per §9.13.1 bullet, in that order (TopMenu/FooterMenu, StepsPanel, StageCanvas arrows, StageCanvas `Delete`, LiveStream, Selector/Settings/Help). Each row's first cell names the keys; the second names the focus area or modal and the action.
2. Add a row for the quit confirmation: `y` / `Enter` quits, `n` / `Esc` cancels.
3. No other row, line or section changes.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && test "$(grep -cE '^## (Requirements|Build|Quickstart|TUI keys|Project layout|Security|Live-backend tests)$' README.md)" -eq 7 && ! grep -q '^| `Up` / `Down` / `Left` / `Right` |' README.md && grep -q 'PageUp' README.md && grep -q 'PageDown' README.md && grep -q '`End`' README.md && grep -q '`Delete`' README.md && grep -q 'FooterMenu' README.md && grep -qE '`y`' README.md && grep -q 'macOS and Windows are not supported yet' README.md && grep -q 'trap exit TERM' README.md && ! grep -qE '/(home|srv|Users|root)/' README.md && test "$(git diff --numstat -- README.md | awk '{print $2}')" -le 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase G — full audit (Steps 59–64)

**Phase goal (frozen until the phase ends):** find what the build gates can't see before the project closes out. That means spec requirements with no code or no test, actions or commands that aren't wired through, swallowed errors and logic that's wrong but passes, and docs that disagree with the code. Each step audits one area and writes one report. Nothing is fixed in this phase; the operator picks which findings become fix steps at phase end.

Written 2026-11-16 against `c1a7776`. All line numbers below are from that tree. No step edits code, so they stay valid for the whole phase.

**Decisions the contract leaves open** (settled here, not by the executor):
- Reports live in `audit/` at the repo root and are committed, so the next phase can cite them.
- POST_GATE checks the report's shape and that every cited `path:line` exists. It runs `cargo fmt --check` only: the scope lock already proves no code changed. Claude checks the content during validation: each finding is re-read at its location, and fabricated or mis-cited findings fail the step.
- Findings marked `Confidence: Low`, and spec questions that need cross-area context, go to Claude reviewers at phase end. Gemini doesn't chase them across areas.

### Phase G audit contract (every Step 59–64 follows this)

**Read first:** the step's spec ranges, then every `####` amendment heading in spec §9.11–§9.13 (`grep -n '^#### 9.1[123]' ORRERY_TECHNICAL_SPECIFICATION.md`). An amendment marked "(overrides §X)" replaces §X where they differ; "(adds to §X)" extends it. Audit against the amended spec, not the original text.

**Non-test code** is everything above the file's first `#[cfg(test)]`. Tests are read only for check C11.

**Checks.** Run all twelve; each one gets a row in the report's `## Checklist` table:
- C1 Spec coverage: every requirement in the step's spec ranges is implemented as written (names, signatures, ordering, messages, defaults).
- C2 `let _ =` on a `Result`, a `Future` or a `send`/`try_send` in non-test code: each one is either justified by the spec or is a finding.
- C3 Error swallowing: `.ok()`, `unwrap_or_default()`, `unwrap_or(..)`, `if let Ok(..)` with no `else`, and `Err(_) =>` arms that discard an error the operator should see.
- C4 Catch-all `_ =>` arms on this crate's enums that would silently absorb a new variant.
- C5 `unreachable!`, `panic!`, `todo!`, `unimplemented!`, `unwrap`/`expect` (and any `#[allow(..)]`) in non-test code.
- C6 Numeric `as` casts that can truncate or wrap; arithmetic that can overflow or underflow without `saturating_`/`checked_`.
- C7 Concurrency: a lock held across `.await`; a `select!` arm whose body awaits something that can block the other arms; a bounded channel whose sender can block or silently drop; a task that's spawned and never joined or aborted.
- C8 File I/O: writes the spec calls atomic that aren't (write-to-temp plus rename); partial-write and crash windows; missing `fsync` where the spec requires durability.
- C9 Paths: user- or agent-supplied paths that skip `resolve_safe_path` or the write-scope checks; symlink escapes; `..` handling.
- C10 Secrets: any path where a secret or API key reaches a log, the live stream, an error message, `Debug` output or a file without redaction.
- C11 Tests: spec'd behaviors with no test; tests whose assertions don't check the spec'd outcome; `#[ignore]` or assertion-free tests.
- C12 Leftovers: `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, dead code kept behind `#[allow(dead_code)]`, commented-out code.

**Report format.** Exactly this; POST_GATE parses it.
```markdown
# Audit: <area> (Step <N>)

## Scope
- Files read: <each path and the line range read>
- Spec ranges: <each § and line range read, including the amendments that apply>

## Findings
### F<N>-<k>: <one-line title>
- Severity: Critical | High | Medium | Low
- Confidence: High | Medium | Low
- Category: missing-feature | wiring | bug | error-handling | concurrency | file-io | path-safety | secrets | test-gap | leftover | doc-drift | spec-gap
- Location: `<repo-relative path>:<line>`
- Spec: §<x.y> | none
- Evidence: <the code or spec text that shows it, at most 5 lines, quoted inline or in a fenced block>
- Impact: <a concrete input or state, and the wrong result or crash it causes>
- Suggested fix: <one or two lines>

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §x.y | ... | Implemented / Partial / Missing / N/A | `path:line` |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F<N>-1, … / none | what was examined |
| … one row per C1–C12 … |
```
Rules:
- `<k>` counts from 1 in report order. Each finding has exactly one `Location:` line (other locations go in Evidence), and it must point to the line that shows the problem.
- Severity: **Critical** = data loss, a secret leak, or a run that corrupts its checkpoint. **High** = a wrong result or a hang in normal use. **Medium** = a wrong result in an edge case, or a missing spec'd behavior. **Low** = docs, tests, or hygiene.
- If something only *might* be wrong and depends on code outside the step's files, write it with `Confidence: Low` and say what would settle it. Don't chase it.
- No finding without evidence. "Looks risky" isn't a finding.
- Don't edit any file except the step's report. Don't run live tests.

---

### Phase G validation notes (Claude's verdict per finding; reviewed at phase end)
- **F59-1** confirmed, High. `ResourceManager::shutdown` (`docker.rs:944`) has no non-test caller in any crate, so inference servers Orrery started are never stopped when a run ends. It's also a spec gap: neither §5.7 nor §9.8 says who calls `shutdown`.
- **F59-2** confirmed as described, reclassified Low / spec-gap. On resume, `consecutive_violations` and `stage_ran_since_gate_failure` both reset after an operator decision, which reads as intentional. The spec doesn't say whether an operator decision resets them.
- **F59-3** confirmed, Medium. `engine.rs:1964` and `:1975` skip keychain errors with `if let Ok`.
- **F59-4** confirmed, raised to Medium and widened. `AgentConfig::validate` (`config.rs:768`) checks `agent_id` only for emptiness. `load_agents` and `read_agents_from_dir` never call `validate_config_file_id` (`config.rs:1512`, which `save_agent` uses), so a hand-edited `agent_id = "../x"` loads and reaches `save_report` (`checkpoint.rs:447`) and the `agent_io/<run_id>/<agent_id>/` mounts. Recheck under Steps 60/61.
- **F59-5** confirmed, Medium. A fresh step start (`engine.rs:511`) resets `round` but never calls `delete_reports`, so the reports from before `reset --step` survive and are loaded by `load_reports(step, None)` into the new prompts.
- **F60-1** confirmed, Medium. This is the same defect as F59-4, extended to `RoleConfig::validate` (`config.rs:645`) and `InferenceConfig::validate`. Fix it once for all three IDs.
- **F60-2** confirmed, Medium. `load_role_rules` (`config.rs:1710`) looks up `{role_id}.toml` by filename, while the loaders key by the `role_id` field inside the file, so a file-stem mismatch passes load and fails at `build_engine`. `sorted_toml_entries` skips dot-files (`config.rs:2103`) that `validate_config_file_id` allows.
- **F60-3** confirmed, Low. The reserved role IDs from §9.0 (spec line 655) aren't rejected anywhere.
- **F60-4** confirmed, Medium. `config.rs:1802` `.unwrap_or_default()`: one bad role file makes `remove_role` delete a shared rules file.
- **F60-5** confirmed, Low. The global Compose `file` is canonicalized (`config.rs:1953`) before the project override by `inference_id` applies.
- **F60-6** confirmed, Low edge case. The `usize` overflow is dropped by `.ok()` (`plan.rs:74`). It only matters when every numbered title overflows; a mixed plan already fails the all-or-none rule.
- **F61-1** partly wrong; corrected, Medium. Attach and start errors do NOT leak: they come back as `Ok(Err(..))` in `exit_res`, and `llm.rs:1201` `remove(&name)` runs before `exit_res?`. What is real: orchestrator turns (`engine.rs:1620`) open no `StageLease`, so a cancel during an AgentCli orchestrator turn drops the future with no `remove`. `end_stage` (`docker.rs:922`) never runs, and the container (secret env included) and `agent_io/<run_id>` are left behind. A 409 on resume with the same `run_id` is plausible but unverified.
- **F61-2** confirmed, Medium. `docker.rs:1189` splits each attach chunk with `.lines()` and has no carry-over buffer. Docker frames follow the process's writes, not line boundaries, so lines can split and a secret split across two frames escapes redaction. `HttpChatDriver` avoids this with `LineBuffer`.
- **F61-3** confirmed, Low. The `llm.rs:1170` `let _ = stream.emit(..)` is forced by the `Fn(String)` callback type.
- **F61-4** is the same defect as F59-4 and F60-1 (`docker.rs:622` `agent_io_dir`, plus `remove_dir_all` in `run_turn`).
- **F61-5** confirmed, Low. `llm.rs:880` returns at the first valid `submit_report`, so later tool calls in the same batch are dropped silently. The spec doesn't say what a mixed batch should do.
- **F62-1** confirmed, High, spec-gap (the code matches §9.5's lexical rule). The `.orrery/` part is the serious one. A stage allocation with an empty `write_scope` (the default) gets `WriteScope::anywhere()` (`engine.rs:1725-1727`), so MCP write tools can write `.orrery/roles/*.md` (the next run's system prompt), `project.toml` and checkpoints. AgentCli masks `.orrery`; MCP doesn't. The symlink part is Medium confidence, since it depends on whether a tool can create symlinks.
- **F62-2** confirmed, Medium, spec-gap. §9.5 checks only `path_args` keys that are *present*, so omitting the key skips the scope check.
- **F62-3** confirmed, High. `mcp.rs` has no timeout anywhere: not on the handshake, `list_tools` or `call_tool`. A stalled server hangs `build_engine`, turns, gates and hooks. This contradicts the Phase D goal that "MCP servers ... can't hang ... a run".
- **F62-4** confirmed, Medium. `mcp.rs:553` `while let Ok(..)` ends the forwarder on the first non-UTF-8 stderr line and drops the pipe, so the child's later stderr writes hit EPIPE/SIGPIPE. The line buffer is also unbounded. (The dropped `JoinHandle` only detaches the task and isn't a bug by itself.)
- **F62-5** confirmed, Low. `rmcp`'s `auth_header(String)` holds an unzeroized copy; this needs an `rmcp` API check before choosing a fix.
- **F62-6** confirmed, Medium. `tail_log` (`stream_log.rs:356`) hardcodes `.log.2`/`.log.1`/`.log`, ignoring `live_stream_max_files`, and uses `read_to_string`, so one non-UTF-8 byte fails `orrery logs`.
- **F62-7** confirmed, Low. This is a limit of the §9.12.5 design I wrote: the Error event appears on the *next* `emit`, so a failure on a run's last events is never shown. The fix is to emit from the writer thread.
- **F62-8** confirmed, Low. `LogFollower` can miss bytes written to the old inode between its read and its stat, the same window `tail -F` has.
- **F63-1** confirmed, downgraded to Medium. In `AwaitingOperator` with no task, `is_running()` (`app.rs:195`) shows `[Stop]`, and Interrupt only overwrites the status message with "Stopping…". It doesn't hang: Approve and Reject still start the run through the no-task branch. Footer `[Run]` is unreachable in that state.
- **F63-2** confirmed, High. An occupied slot's selector (`app.rs:858-866`) lists every agent plus `(remove)`, and choosing an agent calls `assign_agent`, which appends (`app.rs:631`). Replacing the orchestrator's agent always fails the 1-agent limit, and on other stages the agent lands in the next slot.
- **F63-3** confirmed, Medium. `drain_channels` (`bridge.rs:136`) has test callers only, so `run_tui` handles one input per loop and draws between inputs; under heavy stream traffic it falls behind and drops events. The `eng_sub` lag being ignored is real, but broadcast lag on engine events is rare.
- **F63-4** plausible, Medium, Confidence Low. Whether stale queued decisions can auto-answer a later pause depends on whether the engine drains `operator_rx` on entering `AwaitingOperator`; check `engine.rs` `enter_awaiting_operator`.
- **F63-5** accepted on the quoted evidence (`bridge.rs:580`), Medium: `load_initial_app` uses a length check instead of `reconcile` and silently drops load errors.
- **F63-6** confirmed, Medium (keyboard-first). `handle_footer_menu_key` (`app.rs:908`) walks onto the hidden Approve/Reject indices, so the highlight disappears for two key presses.
- **F63-7** accepted on the quoted evidence, Low: `slot_scroll` isn't reset when the header is selected.
- **F63-8** confirmed, Medium. `run_tui`'s `?` and `return Err` (`bridge.rs:640`, `:662`, `:689`) skip `controller.shutdown`, detaching a running engine.
- **F63-9** accepted, Low. The `err.to_string()` sent on `stream_tx` bypasses the hub redactor.
- **F63-10** confirmed, Low. The nested `_ => unreachable!()` is at `bridge.rs:382`.
- **F64-1** confirmed, Medium. `dispatch_status` (`commands.rs:850`) maps checkpoint statuses by index with no `reconcile`. This is the CLI twin of F63-5.
- **F64-2** confirmed by running it, raised to High. `orrery agent add-cli ... --command sh -c 'true'` exits 2 with `unexpected argument '-c' found` (`args.rs:203` lacks `allow_hyphen_values`). No AgentCli command with a flag can be registered from the CLI, including the §4.3 example.
- **F64-3** confirmed, Medium. `subscribe_stream()` (`commands.rs:735`) runs after `build_engine`, so its startup notices (unused server, hidden tools from Step 53) never reach `orrery run` stdout.
- **F64-4** to **F64-7** accepted on the quoted evidence, Low. In order: the `OsKeyringStore` probe bypasses `ctx.secrets`; stdin secret copies aren't zeroized and errors aren't redacted in `main`; the README layout omits `inference.toml` (confirmed against `README.md:95-99`); and there are CLI test gaps.

**Phase G totals:** 41 findings and 39 distinct defects (F60-1 and F61-4 duplicate F59-4): 5 High, 18 Medium, 16 Low. For Claude re-review: F63-4 (does the engine drain stale operator decisions?), F61-1's 409-on-resume claim, the symlink half of F62-1, F62-5 (what the rmcp `auth_header` API allows), and F59-2 (operator's call on reset semantics).
- **F59-2 settled** (operator, 2026-11-16): an operator decision resets `consecutive_violations` and `stage_ran_since_gate_failure`, as the code does now. No code change; the next phase writes this rule into the spec.

**Claude re-review (2026-11-16, two read-only reviewers; key claims spot-checked against the code):**
- **F63-4** confirmed, raised to High. `engine.rs:1105-1111` is the only reader of `operator_rx` and calls `recv()` with no drain. A double Approve or Reject on one pause leaves a stale decision in the 8-slot channel, and it answers the *next* pause (this step or a later one) with no operator input. That's a hidden operator decision, which Phase D was meant to prevent. Fix: drain with `try_recv` at the top of the awaiting path (the real guard), and clear `app.awaiting_operator` after a successful `try_send` (`bridge.rs:384`).
- **F61-1** resume half confirmed, stays Medium. `run_id` persists in the checkpoint, so the CLI container name repeats. The first `resume` after a cancelled AgentCli orchestrator turn fails with a Docker name conflict, and its force-remove (`llm.rs:1201`) cleans the leak, so a second `resume` works. The leaked container runs with its secret env until then. Fix: remove `name` before `run_to_completion`, and have `handle_cancel` clean up the orchestrator's CLI container. Stage-turn cancels are already cleaned by `end_stage`.
- **F62-1** confirmed on both halves, High. Nothing resolves write-tool paths on disk. An AgentCli agent with `--mount rw` can plant `src/escape -> /etc` (or `-> ../.orrery`, which gets around the container mask), and a later MCP write under scope `src` passes `contains`. Separately, an `anywhere()` scope can write `.orrery/`, which `resolve_stdio_command` (`mcp.rs:438-439`) treats as trusted for stdio commands, so planting an executable there is possible (the escalation needs a config pointing at it). Fix: always reject a first segment of `.orrery`, and canonicalize the longest existing ancestor under `project_root`, then check it against the root, `.orrery` and the scope prefix (still TOCTOU against a concurrent rw CLI stage; that's a design note).
- **F62-5** partly, Low. rmcp 3.9.0 `StreamableHttpClientTransportConfig` derives `Debug` with `pub auth_header: Option<String>` (checked), cloned per request. On the wire it's `bearer_auth`, which reqwest marks sensitive. There's no log path today; the `Debug` print is a latent hazard. Zeroizing isn't possible with rmcp 3.9. Fix: pass a `set_sensitive(true)` `AUTHORIZATION` header via `custom_headers` instead of `auth_header`, and update spec §9.5 to match.

**Revised Phase G totals:** 39 distinct defects: **6 High** (F59-1, F62-1, F62-3, F63-2, F63-4, F64-2), 17 Medium, 16 Low. No open questions remain.

### STEP 59: Audit — Engine and Checkpoints

* **Recommended Command:** `/goal`

Scope note: `engine.rs` drives every run and `checkpoint.rs` holds every resume point, so a slip here costs the most.

```text
[STEP_ID]
STEP 59: audit/59-engine-checkpoint.md (NEW) — Phase G audit of engine.rs and checkpoint.rs

[SCOPE_LOCK]
ALLOWED_FILES:
  - audit/59-engine-checkpoint.md (NEW)
READ_ONLY_REFS:
  - STEPPED_IMPLEMENTATION_PLAN.md "### Phase G audit contract" (binding: checks C1–C12 and the report format)
  - crates/orrery-core/src/engine.rs:1-2062 (non-test), 2063-5972 (tests, C11 only)
  - crates/orrery-core/src/checkpoint.rs:1-543 (non-test), 544-1266 (tests, C11 only)
  - ORRERY_TECHNICAL_SPECIFICATION.md:26-45 (§2), 356-381 (§5.5), 441-492 (§5.10), 718-778 (§9.3), 1006-1044 (§9.8), 1160-1192 (§9.11.2), 1225-1243 (§9.11.6, §9.11.7), 1309-1311 (§9.12.3)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No code, test, spec or plan edits. No live tests.

[PRE_GATE]
test ! -e audit/59-engine-checkpoint.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^### Phase G audit contract' STEPPED_IMPLEMENTATION_PLAN.md

[MUTATION_SPEC]
1. Write audit/59-engine-checkpoint.md per the Phase G audit contract, with `<N>` = 59 and area "Engine and Checkpoints".
2. Cover in the Coverage table: the engine state machine phases and transitions, retries and `max_step_retries`, escalation, the three validation modes, operator decisions (including §9.11.7), hooks (`on_run_start`, `on_step_pass`, `on_retry`, `on_halt`) and their `on_failure`, Loop mode cycles, interrupt and cancel, resume, `build_engine` (including §9.12.3), and checkpoint save/load atomicity and the stamped-report paths.
3. Run C1–C12 on both files.

[POST_GATE]
R=audit/59-engine-checkpoint.md && cargo fmt --all -- --check && test -s $R && for h in '# Audit: Engine and Checkpoints (Step 59)' '## Scope' '## Findings' '## Coverage' '## Checklist'; do grep -qxF "$h" $R || exit 1; done && test "$(grep -cE '^\| C([1-9]|1[0-2]) \|' $R)" -eq 12 && n=$(grep -cE '^### F59-[0-9]+: ' $R) && test "$(grep -cE '^- Severity: (Critical|High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Confidence: (High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Location: `[^`]+:[0-9]+`$' $R)" -eq "$n" && test "$(grep -cE '^- (Category|Spec|Evidence|Impact|Suggested fix): ' $R)" -eq "$((n*5))" && grep -oE '^- Location: `[^`]+:[0-9]+`$' $R | sed -E 's/^- Location: `([^`]+):([0-9]+)`$/\1 \2/' | while read -r p l; do test -f "$p" && test "$l" -ge 1 && test "$l" -le "$(wc -l < "$p")" || { echo "bad location $p:$l"; exit 1; }; done

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 60: Audit — Config, Security, Plan, Errors

* **Recommended Command:** `/goal`

Scope note: config validation is the first line of defense: secret patterns, tool names, paths, limits. `security.rs` owns the keychain and redaction.

```text
[STEP_ID]
STEP 60: audit/60-config-security-plan.md (NEW) — Phase G audit of config.rs, security.rs, plan.rs and lib.rs

[SCOPE_LOCK]
ALLOWED_FILES:
  - audit/60-config-security-plan.md (NEW)
READ_ONLY_REFS:
  - STEPPED_IMPLEMENTATION_PLAN.md "### Phase G audit contract" (binding)
  - crates/orrery-core/src/config.rs:1-2293, security.rs:1-233, plan.rs:1-222, lib.rs:1-63 (non-test); the rest of each file for C11 only
  - ORRERY_TECHNICAL_SPECIFICATION.md:125-295 (§4), 298-355 (§5.1–§5.4), 649-717 (§9.0–§9.2), 922-955 (§9.6a), 1160-1192 (§9.11.2), 1300-1304 (§9.12.1)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No code, test, spec or plan edits. No live tests.

[PRE_GATE]
test ! -e audit/60-config-security-plan.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^### Phase G audit contract' STEPPED_IMPLEMENTATION_PLAN.md

[MUTATION_SPEC]
1. Write audit/60-config-security-plan.md per the Phase G audit contract, with `<N>` = 60 and area "Config, Security, Plan, Errors".
2. Cover in the Coverage table: every §4 schema field (exists, has the spec'd default, is validated, and is actually read somewhere; use `grep -rn '<field>' crates/` to confirm it's read outside config.rs, and report a field nothing reads as `wiring`), global-vs-project catalog override by id, `resolve_safe_path`, the embedded-key pattern rejection, `[limits]` validation, the keychain headless fallback, `SecretRedactor`, plan parsing (marker, numbering, sha256) and the `Error` variants in §5.1.
3. Run C1–C12 on all four files.

[POST_GATE]
R=audit/60-config-security-plan.md && cargo fmt --all -- --check && test -s $R && for h in '# Audit: Config, Security, Plan, Errors (Step 60)' '## Scope' '## Findings' '## Coverage' '## Checklist'; do grep -qxF "$h" $R || exit 1; done && test "$(grep -cE '^\| C([1-9]|1[0-2]) \|' $R)" -eq 12 && n=$(grep -cE '^### F60-[0-9]+: ' $R) && test "$(grep -cE '^- Severity: (Critical|High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Confidence: (High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Location: `[^`]+:[0-9]+`$' $R)" -eq "$n" && test "$(grep -cE '^- (Category|Spec|Evidence|Impact|Suggested fix): ' $R)" -eq "$((n*5))" && grep -oE '^- Location: `[^`]+:[0-9]+`$' $R | sed -E 's/^- Location: `([^`]+):([0-9]+)`$/\1 \2/' | while read -r p l; do test -f "$p" && test "$l" -ge 1 && test "$l" -le "$(wc -l < "$p")" || { echo "bad location $p:$l"; exit 1; }; done

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 61: Audit — Docker and Agent Drivers

* **Recommended Command:** `/goal`

Scope note: these are the three agent backends (LocalModel, AgentCli, CloudApi) plus container and inference lifecycle: the code that talks to the outside world.

```text
[STEP_ID]
STEP 61: audit/61-docker-llm.md (NEW) — Phase G audit of docker.rs and llm.rs

[SCOPE_LOCK]
ALLOWED_FILES:
  - audit/61-docker-llm.md (NEW)
READ_ONLY_REFS:
  - STEPPED_IMPLEMENTATION_PLAN.md "### Phase G audit contract" (binding)
  - crates/orrery-core/src/docker.rs:1-1231, llm.rs:1-1240 (non-test); the rest of each file and crates/orrery-core/tests/live_docker.rs, live_llm.rs for C11 only
  - ORRERY_TECHNICAL_SPECIFICATION.md:391-417 (§5.7), 431-440 (§5.9), 844-921 (§9.6), 956-1005 (§9.7), 1193-1201 (§9.11.3), 1216-1224 (§9.11.5), 1283-1288 (§9.11.12), 1306-1307 (§9.12.2)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No code, test, spec or plan edits. No live tests, no docker commands.

[PRE_GATE]
test ! -e audit/61-docker-llm.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^### Phase G audit contract' STEPPED_IMPLEMENTATION_PLAN.md

[MUTATION_SPEC]
1. Write audit/61-docker-llm.md per the Phase G audit contract, with `<N>` = 61 and area "Docker and Agent Drivers".
2. Cover in the Coverage table: the `ContainerBackend` operations and `cli_container_name`, AgentCli mounts (`in/`, `out/`, the `.orrery` mask), memory/time limits and cleanup on every exit path (success, error, timeout, cancel), `secret_env` injection, `ResourceManager` (`max_concurrent_agents`, inference start/stop, `unload_after_stage`, `startup_timeout_secs`, External/Container/Compose lifecycles), `HttpChatDriver` (request shape, tool-call loop and `max_tool_calls`, idle timeout, context preflight, streaming, report parsing), and the Bearer header (§9.12.2).
3. Run C1–C12 on both files.

[POST_GATE]
R=audit/61-docker-llm.md && cargo fmt --all -- --check && test -s $R && for h in '# Audit: Docker and Agent Drivers (Step 61)' '## Scope' '## Findings' '## Coverage' '## Checklist'; do grep -qxF "$h" $R || exit 1; done && test "$(grep -cE '^\| C([1-9]|1[0-2]) \|' $R)" -eq 12 && n=$(grep -cE '^### F61-[0-9]+: ' $R) && test "$(grep -cE '^- Severity: (Critical|High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Confidence: (High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Location: `[^`]+:[0-9]+`$' $R)" -eq "$n" && test "$(grep -cE '^- (Category|Spec|Evidence|Impact|Suggested fix): ' $R)" -eq "$((n*5))" && grep -oE '^- Location: `[^`]+:[0-9]+`$' $R | sed -E 's/^- Location: `([^`]+):([0-9]+)`$/\1 \2/' | while read -r p l; do test -f "$p" && test "$l" -ge 1 && test "$l" -le "$(wc -l < "$p")" || { echo "bad location $p:$l"; exit 1; }; done

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 62: Audit — MCP Gatekeeper and Live Stream

* **Recommended Command:** `/goal`

Scope note: the gatekeeper is the security boundary for agent tool calls. The live stream is the only thing the operator sees, and it must redact.

```text
[STEP_ID]
STEP 62: audit/62-mcp-stream.md (NEW) — Phase G audit of mcp.rs and stream_log.rs

[SCOPE_LOCK]
ALLOWED_FILES:
  - audit/62-mcp-stream.md (NEW)
READ_ONLY_REFS:
  - STEPPED_IMPLEMENTATION_PLAN.md "### Phase G audit contract" (binding)
  - crates/orrery-core/src/mcp.rs:1-680, stream_log.rs:1-555 (non-test); the rest of each file and crates/orrery-core/tests/live_mcp.rs for C11 only
  - ORRERY_TECHNICAL_SPECIFICATION.md:382-390 (§5.6), 418-430 (§5.8), 779-843 (§9.4, §9.5), 1138-1159 (§9.11.1), 1202-1224 (§9.11.4, §9.11.5), 1309-1319 (§9.12.3–§9.12.5)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No code, test, spec or plan edits. No live tests.

[PRE_GATE]
test ! -e audit/62-mcp-stream.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^### Phase G audit contract' STEPPED_IMPLEMENTATION_PLAN.md

[MUTATION_SPEC]
1. Write audit/62-mcp-stream.md per the Phase G audit contract, with `<N>` = 62 and area "MCP Gatekeeper and Live Stream".
2. Cover in the Coverage table: `is_tool_allowed` and role patterns, `write_tools` path-argument checks against the project root and `write_scope` (try `..`, absolute paths, symlinks and missing arguments in your reasoning), Stdio and StreamableHttp transports (connect, handshake, timeouts, auth header), the stderr forwarder, `tools_for_role`, tool-result handling, and for the live stream: redaction of every event, the ring, the writer thread, rotation, `tail_log`, `LogFollower` and the write-error event.
3. Run C1–C12 on both files.

[POST_GATE]
R=audit/62-mcp-stream.md && cargo fmt --all -- --check && test -s $R && for h in '# Audit: MCP Gatekeeper and Live Stream (Step 62)' '## Scope' '## Findings' '## Coverage' '## Checklist'; do grep -qxF "$h" $R || exit 1; done && test "$(grep -cE '^\| C([1-9]|1[0-2]) \|' $R)" -eq 12 && n=$(grep -cE '^### F62-[0-9]+: ' $R) && test "$(grep -cE '^- Severity: (Critical|High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Confidence: (High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Location: `[^`]+:[0-9]+`$' $R)" -eq "$n" && test "$(grep -cE '^- (Category|Spec|Evidence|Impact|Suggested fix): ' $R)" -eq "$((n*5))" && grep -oE '^- Location: `[^`]+:[0-9]+`$' $R | sed -E 's/^- Location: `([^`]+):([0-9]+)`$/\1 \2/' | while read -r p l; do test -f "$p" && test "$l" -ge 1 && test "$l" -le "$(wc -l < "$p")" || { echo "bad location $p:$l"; exit 1; }; done

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 63: Audit — TUI

* **Recommended Command:** `/goal`

Scope note: the TUI must never block, never hide an operator decision, and be fully usable from the keyboard. Mouse support is an addition on top, not a substitute (operator, 2026-11-16).

```text
[STEP_ID]
STEP 63: audit/63-tui.md (NEW) — Phase G audit of orrery-tui (app, bridge, layout, widgets, theme)

[SCOPE_LOCK]
ALLOWED_FILES:
  - audit/63-tui.md (NEW)
READ_ONLY_REFS:
  - STEPPED_IMPLEMENTATION_PLAN.md "### Phase G audit contract" (binding)
  - crates/orrery-tui/src/app.rs:1-1157, bridge.rs:1-710, layout.rs:1-282, widgets.rs:1-886, theme.rs:1-84 (non-test); the rest of each file for C11 only
  - ORRERY_TECHNICAL_SPECIFICATION.md:493-580 (§6), 1045-1120 (§9.9), 1261-1288 (§9.11.9–§9.11.12), 1321-1323 (§9.12.6), 1332-1341 (§9.13.1)
  - README.md:73-97 (TUI keys)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No code, test, spec or plan edits. Don't launch the TUI.

[PRE_GATE]
test ! -e audit/63-tui.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^### Phase G audit contract' STEPPED_IMPLEMENTATION_PLAN.md

[MUTATION_SPEC]
1. Write audit/63-tui.md per the Phase G audit contract, with `<N>` = 63 and area "TUI".
2. Cover in the Coverage table, plus these extra rows:
   - every `AppAction` variant (`app.rs` `pub enum AppAction`): where `bridge.rs` handles it, and what it does when no run is active;
   - every `UiRegion` variant (`layout.rs`): the keyboard path that reaches the same action (focus area, keys); a region with no keyboard path is a `wiring` finding;
   - every row of the README TUI keys table: the `app.rs` handler that implements it; a mismatch is a `doc-drift` finding;
   - every modal: how it opens, how it closes, and that `Esc` always gets out;
   - the render path: nothing in the draw loop awaits engine work (§9.11.9).
3. Run C1–C12 on all five files.

[POST_GATE]
R=audit/63-tui.md && cargo fmt --all -- --check && test -s $R && for h in '# Audit: TUI (Step 63)' '## Scope' '## Findings' '## Coverage' '## Checklist'; do grep -qxF "$h" $R || exit 1; done && test "$(grep -cE '^\| C([1-9]|1[0-2]) \|' $R)" -eq 12 && n=$(grep -cE '^### F63-[0-9]+: ' $R) && test "$(grep -cE '^- Severity: (Critical|High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Confidence: (High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Location: `[^`]+:[0-9]+`$' $R)" -eq "$n" && test "$(grep -cE '^- (Category|Spec|Evidence|Impact|Suggested fix): ' $R)" -eq "$((n*5))" && grep -oE '^- Location: `[^`]+:[0-9]+`$' $R | sed -E 's/^- Location: `([^`]+):([0-9]+)`$/\1 \2/' | while read -r p l; do test -f "$p" && test "$l" -ge 1 && test "$l" -le "$(wc -l < "$p")" || { echo "bad location $p:$l"; exit 1; }; done && grep -q 'AppAction' $R && grep -q 'UiRegion' $R

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 64: Audit — CLI and README

* **Recommended Command:** `/goal`

Scope note: the CLI is the operator's scripting surface, and §7 is its contract. The README must match both.

```text
[STEP_ID]
STEP 64: audit/64-cli-readme.md (NEW) — Phase G audit of orrery-cli and README.md against the code

[SCOPE_LOCK]
ALLOWED_FILES:
  - audit/64-cli-readme.md (NEW)
READ_ONLY_REFS:
  - STEPPED_IMPLEMENTATION_PLAN.md "### Phase G audit contract" (binding)
  - crates/orrery-cli/src/args.rs:1-443, commands.rs:1-953, main.rs:1-73 (non-test); the rest of each file and crates/orrery-cli/tests/cli_smoke.rs for C11 only
  - README.md (whole file)
  - ORRERY_TECHNICAL_SPECIFICATION.md:581-644 (§7, §8), 1121-1134 (§9.10), 1244-1260 (§9.11.8), 1289-1296 (§9.11.13, §9.11.14), 1300-1328 (§9.12)
  - For README checks only: crates/orrery-core/src/config.rs (field names, defaults), crates/orrery-core/tests/live_*.rs (env var names)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No code, test, spec or plan edits. Don't run the orrery binary against a real project and don't run live tests. `cargo run -p orrery-cli --locked -- <cmd> --help` is allowed.

[PRE_GATE]
test ! -e audit/64-cli-readme.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q '^### Phase G audit contract' STEPPED_IMPLEMENTATION_PLAN.md

[MUTATION_SPEC]
1. Write audit/64-cli-readme.md per the Phase G audit contract, with `<N>` = 64 and area "CLI and README".
2. Cover in the Coverage table, plus these extra rows:
   - every §7 command row: the `args.rs` variant, every flag spelled as §7 spells it, and the `commands.rs` handler; a missing or misspelled flag is a `wiring` finding;
   - the exit codes (0, 1, 2, 3, 130): where each is produced, and that `run` returns 3 when a step awaits the operator;
   - every README command, flag, env var, path and default: the code that backs it; a mismatch is a `doc-drift` finding;
   - every README statement about platform, Docker or the keychain, checked against §9.12.1.
3. Run C1–C12 on the three CLI files.

[POST_GATE]
R=audit/64-cli-readme.md && cargo fmt --all -- --check && test -s $R && for h in '# Audit: CLI and README (Step 64)' '## Scope' '## Findings' '## Coverage' '## Checklist'; do grep -qxF "$h" $R || exit 1; done && test "$(grep -cE '^\| C([1-9]|1[0-2]) \|' $R)" -eq 12 && n=$(grep -cE '^### F64-[0-9]+: ' $R) && test "$(grep -cE '^- Severity: (Critical|High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Confidence: (High|Medium|Low)$' $R)" -eq "$n" && test "$(grep -cE '^- Location: `[^`]+:[0-9]+`$' $R)" -eq "$n" && test "$(grep -cE '^- (Category|Spec|Evidence|Impact|Suggested fix): ' $R)" -eq "$((n*5))" && grep -oE '^- Location: `[^`]+:[0-9]+`$' $R | sed -E 's/^- Location: `([^`]+):([0-9]+)`$/\1 \2/' | while read -r p l; do test -f "$p" && test "$l" -ge 1 && test "$l" -le "$(wc -l < "$p")" || { echo "bad location $p:$l"; exit 1; }; done

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase H — audit fixes (Steps 65–76)

**Phase goal (frozen until the phase ends):** fix every High and Medium defect the Phase G audit confirmed. That's 23 defects:
- the 6 High: F59-1, F62-1, F62-3, F63-2, F63-4, F64-2
- the 17 Medium

Each fix is exactly as Claude's validation notes and the re-review describe it, with nothing beyond them. The 16 Low findings stay parked for a later pass. Design: spec §9.14 (`ORRERY_TECHNICAL_SPECIFICATION.md:1343-1421`).

Written 2026-11-16 against `b816aa4`. Several files are edited by more than one step (config.rs: 65, 67, 75; mcp.rs: 66–68; engine.rs: 66, 67, 69–71, 76; bridge.rs: 73, 74; docker.rs: 71, 72). Later steps name functions as well as lines, because earlier steps shift line numbers.

**Decisions the contract leaves open** (settled here, not by the executor):
- Required `path_args` (F62-2): a write tool's listed path argument must be present. Omitting it is a security error, not a pass.
- MCP timeouts (F62-3): the handshake and `list_tools` use a fixed 60 s. Tool calls use the new `[limits] mcp_call_timeout_secs`, default 600 s, because gate tools may run long test suites.
- `orrery status` on an edited step (F64-1) fails with the reconcile error instead of showing stale statuses.
- `agent add-cli --command` (F64-2) must be the last option, since it takes everything after it.
- F63-1 is fixed in the bridge only: Interrupt with no task explains itself. The footer label logic in `app.rs` is unchanged.
- F59-2 (operator, 2026-11-16): operator decisions keep resetting the violation counters. Only the spec records it (§9.14.5).

### STEP 65: Config ID Integrity

* **Recommended Command:** `/goal`

Scope note: F59-4 / F60-1 / F61-4 (ids with path characters load from hand-edited files), F60-2 (file name vs id, leading-dot ids), F60-4 (`remove_role` deletes a shared rules file on a parse error).

```text
[STEP_ID]
STEP 65: Config ID Integrity

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1346-1351 (§9.14.1; binding)
  - crates/orrery-core/src/config.rs:645 (RoleConfig::validate), 768 (AgentConfig::validate), InferenceConfig::validate (~1050), 1512-1525 (validate_config_file_id), 1780-1812 (remove_role), 2100-2110 (sorted_toml_entries dot-file skip), 2129 (read_single_role), 2164 (read_roles_from_dir), 2173-2185 (read_agents_from_dir)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Signatures stay as they are. MCP `server_id` handling is unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q "id.starts_with('.')" crates/orrery-core/src/config.rs && grep -Pzq 'read_roles_from_dir\(&roles_dir\)\s*\.unwrap_or_default\(\)' crates/orrery-core/src/config.rs && sed -n 1346p $S | grep -q '^#### 9.14.1 '

[MUTATION_SPEC]
1. `validate_config_file_id`: after the existing checks, `if id.starts_with('.') { return Err(Error::PathTraversal { path: PathBuf::from(id) }); }`.
2. `AgentConfig::validate`, `RoleConfig::validate` and `InferenceConfig::validate`: the first statement is `validate_config_file_id(&self.agent_id, "agent_id")?;` / `(&self.role_id, "role_id")` / `(&self.inference_id, "inference_id")`. Keep each existing emptiness check after it.
3. `read_single_role(roles_dir, path)` and the loop in `read_agents_from_dir`: after parsing, compare `path.file_stem().and_then(|s| s.to_str())` with the id. On a mismatch (or a `None` stem), return `Error::Validation { field: "role_id" | "agent_id", message: format!("file '{stem}.toml' declares id '{id}'; the file must be named '{id}.toml'") }`.
4. `remove_role`: `Self::read_roles_from_dir(&roles_dir).unwrap_or_default()` becomes `Self::read_roles_from_dir(&roles_dir)?`.
5. Tests (in `config::tests`):
   - `validate_rejects_path_ids`: `AgentConfig`, `RoleConfig` and `InferenceConfig` with ids `"../x"`, `"a/b"` and `".hidden"` each fail `validate()` with `Error::PathTraversal`.
   - `load_rejects_mismatched_file_stem`: write `.orrery/roles/custom.toml` with `role_id = "reviewer"` (plus a rules file); `load_roles()` is `Err(Error::Validation { field: "role_id", .. })`. The same check for an agent file `agents/a1.toml` with `agent_id = "a2"`.
   - `load_rejects_hand_edited_traversal_id`: `agents/x.toml` with `agent_id = "../x"` makes `load_agents()` an `Err`.
   - `remove_role_keeps_rules_when_peer_unreadable`: two roles share `rules_file`, and a third role file is malformed TOML. `remove_role(first)` is `Err`, and the shared rules file still exists.
6. If an existing test writes a role or agent file whose name differs from its id, that test hits the bug this step fixes: rename the file in the test to `<id>.toml`. Don't weaken any assertion.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q "id.starts_with('.')" crates/orrery-core/src/config.rs && ! grep -Pzq 'read_roles_from_dir\(&roles_dir\)\s*\.unwrap_or_default\(\)' crates/orrery-core/src/config.rs && grep -q 'the file must be named' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(config::tests::validate_rejects_path_ids|config::tests::load_rejects_mismatched_file_stem|config::tests::load_rejects_hand_edited_traversal_id|config::tests::remove_role_keeps_rules_when_peer_unreadable): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 66: MCP Write-Scope Hardening

* **Recommended Command:** `/goal`

Scope note: F62-1 (High: MCP write tools can reach `.orrery/`, and symlinks escape a scope) and F62-2 (an omitted `path_args` key skips the check).

```text
[STEP_ID]
STEP 66: MCP Write-Scope Hardening

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1352-1361 (§9.14.2; binding), 802-843 (§9.5)
  - crates/orrery-core/src/mcp.rs:36-91 (WriteScope, normalize_path_segments), 122-127 (McpGatekeeper fields; it derives Default), 204-260 (execute_for_role)
  - crates/orrery-core/src/engine.rs:1993 (build_engine `let mut gatekeeper = McpGatekeeper::new();`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `scopes_overlap` and `normalize_path_segments` keep their behavior. No change to `WriteScope::new`.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'fn resolve_write_path' crates/orrery-core/src/mcp.rs && grep -q 'if let Some(val) = args.get(arg_name)' crates/orrery-core/src/mcp.rs && sed -n 1352p $S | grep -q '^#### 9.14.2 '

[MUTATION_SPEC]
1. `WriteScope::contains`: after normalizing, `if path_segs.first() == Some(&".orrery") { return false; }`, before the empty-prefix check.
2. `execute_for_role`, write-tool loop: replace `if let Some(val) = args.get(arg_name)` with `let Some(val) = args.get(arg_name) else { return Err(Error::Security { message: format!("write tool '{server_id}:{tool}' is missing path argument '{arg_name}'") }); };`, then the existing string and `contains` checks.
3. `McpGatekeeper` gains `project_root: Option<PathBuf>` and `pub fn set_project_root(&mut self, root: PathBuf) { self.project_root = Some(root); }`.
4. Add `pub fn resolve_write_path(root: &Path, rel: &str) -> Result<PathBuf>` per §9.14.2: `fs::canonicalize(root)`, then walk `normalize_path_segments(rel)` (None → `Error::Security`). For each segment: `symlink_metadata(&next)` `Ok` → `fs::canonicalize(&next)` (an error → `Error::Security { message: format!("path '{rel}' contains an unresolvable entry") }`); `Err(NotFound)` → push that segment and the rest lexically and stop; another `Err` → `Error::Io`.
5. In `execute_for_role`, after `scope.contains(path_str)` passes and when `self.project_root` is `Some(root)`: `let canonical_root = fs::canonicalize(root)` (`Error::Io`); `let resolved = resolve_write_path(root, path_str)?;`. Fail with the §9.14.2 message unless `resolved.starts_with(&canonical_root)`, `!resolved.starts_with(canonical_root.join(".orrery"))`, and, when `!scope.prefixes().is_empty()`, some prefix has `resolved.starts_with(resolve_write_path(root, prefix)?)`.
6. engine.rs `build_engine`: right after `let mut gatekeeper = McpGatekeeper::new();`, add `gatekeeper.set_project_root(store.project_root().to_path_buf());`.
7. Tests in `mcp::tests` (symlink tests under `#[cfg(unix)]`, using `std::os::unix::fs::symlink` and `tempfile::TempDir`):
   - `write_scope_rejects_orrery_first_segment`: `anywhere().contains(".orrery/project.toml")` and `contains("./.orrery/x")` are false; `contains("src/.orrery/x")` is true.
   - `execute_rejects_missing_path_arg`: a write tool `fs:write_file` with `path_args = ["path"]`, called with `{}`, returns `Error::Security`.
   - `execute_rejects_symlink_escape`: root `r` with `r/src/` and `r/src/escape -> <second TempDir>`; `set_project_root(r)`; scope `["src"]`; `{"path": "src/escape/x"}` → `Error::Security`; `{"path": "src/ok.txt"}` → `Ok`.
   - `execute_rejects_symlink_into_orrery`: `r/.orrery/` exists and `r/src/o -> r/.orrery`; scope `anywhere()`; `{"path": "src/o/x"}` → `Error::Security`.
   Use `MockMcpTransport::new("fs").with_tool("write_file", json!({}))` and a role that allows `fs:write_file`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn resolve_write_path' crates/orrery-core/src/mcp.rs && grep -q 'pub fn set_project_root' crates/orrery-core/src/mcp.rs && grep -q 'is missing path argument' crates/orrery-core/src/mcp.rs && grep -q 'gatekeeper.set_project_root(store.project_root().to_path_buf());' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(mcp::tests::write_scope_rejects_orrery_first_segment|mcp::tests::execute_rejects_missing_path_arg|mcp::tests::execute_rejects_symlink_escape|mcp::tests::execute_rejects_symlink_into_orrery): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 67: MCP Timeouts

* **Recommended Command:** `/goal`

Scope note: F62-3 (High): there's no timeout anywhere in `mcp.rs`, so a stalled MCP server hangs `build_engine`, turns, gates and hooks.

```text
[STEP_ID]
STEP 67: MCP Timeouts

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
  - crates/orrery-core/src/config.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1362-1367 (§9.14.3; binding), 1160-1192 (§9.11.2)
  - crates/orrery-core/src/config.rs:152-205 (RuntimeLimits, Default, validate), the `RuntimeLimits {` literals in config.rs tests (~3915-3970)
  - crates/orrery-core/src/mcp.rs: `RmcpTransport::connect_with_stream` (the two `rmcp::ServiceExt::serve(..).await` calls), `impl McpTransport for RmcpTransport` (`list_tools`, `call_tool`), `McpGatekeeper` and `execute_for_role` (after Step 66)
  - crates/orrery-core/src/engine.rs build_engine (the `set_project_root` line Step 66 added)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other limit changes. Timeouts aren't applied in `MockMcpTransport`.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn resolve_write_path' crates/orrery-core/src/mcp.rs && ! grep -q 'mcp_call_timeout_secs' crates/orrery-core/src/config.rs && sed -n 1362p $S | grep -q '^#### 9.14.3 '

[MUTATION_SPEC]
1. config.rs `RuntimeLimits`: add `pub mcp_call_timeout_secs: u64` after `model_idle_timeout_secs`, default `600`. In `validate`, right after the `model_idle_timeout_secs` check: `1..=86_400`, otherwise `Error::Validation { field: "limits.mcp_call_timeout_secs", message: "mcp_call_timeout_secs must be in 1..=86_400" }`. Add the field to every `RuntimeLimits { .. }` literal in the tests.
2. mcp.rs: `pub const MCP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);`. Wrap both `rmcp::ServiceExt::serve(..)` awaits and the body of `RmcpTransport::list_tools` in `tokio::time::timeout(MCP_CONNECT_TIMEOUT, ..)`. On `Elapsed`, return `Error::Mcp { server_id, source: Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "MCP handshake timed out after 60s")) }` (`"list_tools timed out after 60s"` for `list_tools`).
3. `McpGatekeeper` gains `call_timeout: Option<std::time::Duration>` and `pub fn set_call_timeout(&mut self, timeout: std::time::Duration)`. `execute_for_role`'s final `transport.call_tool(tool, args).await` runs inside `tokio::time::timeout(t, ..)` when `Some(t)`. Elapsed → `Error::Mcp { server_id: server_id.to_string(), source: Box::new(io::Error::new(TimedOut, format!("tool '{server_id}:{tool}' timed out after {}s", t.as_secs()))) }`.
4. engine.rs `build_engine`: next to `set_project_root`, add `gatekeeper.set_call_timeout(std::time::Duration::from_secs(project.limits.mcp_call_timeout_secs));`.
5. Tests:
   - `config::tests::limits_mcp_call_timeout_bounds`: the default is 600; 0 and 86_401 fail with field `limits.mcp_call_timeout_secs`; 1 and 86_400 pass.
   - `mcp::tests::execute_times_out_slow_tool` (`#[tokio::test]`): a test-only `struct SlowTransport` implementing `McpTransport` whose `call_tool` sleeps 30 s; `set_call_timeout(Duration::from_millis(50))`; `execute_for_role` returns `Error::Mcp` within 5 s (wrap the call in a 5 s `tokio::time::timeout` and assert the inner `Err`).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub mcp_call_timeout_secs: u64' crates/orrery-core/src/config.rs && grep -q 'pub const MCP_CONNECT_TIMEOUT' crates/orrery-core/src/mcp.rs && grep -q 'pub fn set_call_timeout' crates/orrery-core/src/mcp.rs && grep -Pzq 'set_call_timeout\(\s*std::time::Duration::from_secs\(\s*project\.limits\.mcp_call_timeout_secs,?\s*\),?\s*\)' crates/orrery-core/src/engine.rs && ! grep -q 'rustfmt::skip' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(config::tests::limits_mcp_call_timeout_bounds|mcp::tests::execute_times_out_slow_tool): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 68: MCP Stderr Bytes and Log Tail

* **Recommended Command:** `/goal`

Scope note: F62-4 (invalid UTF-8 on an MCP server's stderr ends the forwarder and closes the pipe; lines are unbounded) and F62-6 (`tail_log` ignores `live_stream_max_files` and fails on non-UTF-8).

```text
[STEP_ID]
STEP 68: MCP Stderr Bytes and Log Tail

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
  - crates/orrery-core/src/stream_log.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1368-1378 (§9.14.4; binding), 1138-1159 (§9.11.1)
  - crates/orrery-core/src/mcp.rs `spawn_stderr_forwarder` (~542-558) and the test `stderr_forwarder_emits_redacted_notices`
  - crates/orrery-core/src/stream_log.rs `tail_log` (~355-376), `rotated_log_path`
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `spawn_stderr_forwarder` and `tail_log` keep their signatures and return types. The existing forwarder test stays unchanged and passing.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'while let Ok(Some(line)) = lines.next_line().await' crates/orrery-core/src/mcp.rs && grep -q 'rotated_log_path(log_dir, 2),' crates/orrery-core/src/stream_log.rs && sed -n 1368p $S | grep -q '^#### 9.14.4 '

[MUTATION_SPEC]
1. mcp.rs `spawn_stderr_forwarder`: replace the `BufReader::lines` loop with `use tokio::io::AsyncReadExt; let mut chunk = [0u8; 4096]; let mut pending: Vec<u8> = Vec::new();`. Loop on `stderr.read(&mut chunk).await`:
   - `Ok(0)` → emit the non-empty remainder and stop;
   - `Ok(n)` → extend `pending`, then while it holds a `b'\n'`, drain through it and emit the line (without `\n` and a trailing `\r`);
   - if `pending.len() >= 4096`, drain and emit 4096 bytes;
   - `Err(_)` → stop.

   Emitting means `let text = String::from_utf8_lossy(&bytes); if !text.trim().is_empty() { let _ = stream.emit(StreamEvent::notice(format!("[mcp:{server_id}] {text}"))); }`. The `let _ =` is spec'd: the forwarder has no caller to report to.
2. stream_log.rs `tail_log`: build the candidate list as `(1..=9).rev().map(|i| rotated_log_path(log_dir, i))` followed by `log_dir.join(LOG_FILE)`. Read each with `fs::read`, skipping `NotFound` as now, and take `String::from_utf8_lossy(&bytes).lines()`.
3. Tests:
   - `mcp::tests::stderr_forwarder_survives_invalid_utf8`: input `&b"ok\n\xff\xfebad\nafter\n"[..]` gives 3 notices: `"[mcp:fs] ok"`, `"[mcp:fs] \u{FFFD}\u{FFFD}bad"`, `"[mcp:fs] after"`.
   - `mcp::tests::stderr_forwarder_caps_long_lines`: 5000 × `b'a'` then `b"\n"` gives 2 notices, with 4096 and 904 `a`s after the `"[mcp:fs] "` prefix.
   - `stream_log::tests::tail_log_reads_high_rotations`: files `.log.4` = `"a\n"`, `.log.1` = `"b\n"`, `.log` = `"c\n"` → `tail_log(dir, 10)` is `["a", "b", "c"]`.
   - `stream_log::tests::tail_log_tolerates_invalid_utf8`: `.log` = `b"x\xffy\n"` → `Ok(["x\u{FFFD}y"])`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'lines.next_line().await' crates/orrery-core/src/mcp.rs && ! grep -q 'rotated_log_path(log_dir, 2),' crates/orrery-core/src/stream_log.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(mcp::tests::stderr_forwarder_survives_invalid_utf8|mcp::tests::stderr_forwarder_caps_long_lines|mcp::tests::stderr_forwarder_emits_redacted_notices|stream_log::tests::tail_log_reads_high_rotations|stream_log::tests::tail_log_tolerates_invalid_utf8): test$')" -eq 5

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 69: Engine Exit Shutdown and Stale Decisions

* **Recommended Command:** `/goal`

Scope note: F59-1 (High: inference servers Orrery started are never stopped) and the engine half of F63-4 (High: a stale queued decision answers the next pause).

```text
[STEP_ID]
STEP 69: Engine Exit Shutdown and Stale Decisions

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1379-1383 (§9.14.5; binding)
  - crates/orrery-core/src/engine.rs:346 (`pub async fn run`), 1068-1112 (`enter_awaiting_operator`, the only `operator_rx` reader), 2097-2190 (`test_context`), 4308-4317 (`set_max_concurrency`: rebuilding `ctx.resources`)
  - crates/orrery-core/src/docker.rs:289-326 (`InferenceCall`, `MockInferenceControl::calls`; it records `Stop(inference_id)`), 944 (`ResourceManager::shutdown`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other engine behavior changes. `run`'s signature is unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'async fn run_steps' crates/orrery-core/src/engine.rs && ! grep -q 'rx.try_recv().is_ok()' crates/orrery-core/src/engine.rs && sed -n 1379p $S | grep -q '^#### 9.14.5 '

[MUTATION_SPEC]
1. Rename the current `pub async fn run` body to `async fn run_steps(&self, opts: RunOptions, cancel: CancellationToken) -> Result<RunOutcome>`. The new `pub async fn run(&self, opts, cancel)` does `let result = self.run_steps(opts, cancel).await; if let Err(err) = self.ctx.resources.shutdown().await { self.ctx.stream.emit(StreamEvent::notice(format!("inference shutdown failed: {err}")))?; } result`.
2. `enter_awaiting_operator`: as its first statement, `{ let mut rx = self.operator_rx.lock().await; while rx.try_recv().is_ok() {} }`.
3. Tests (in `engine::tests`):
   - `run_stops_started_inference_on_exit` (`#[tokio::test]`): `test_context` with the `"router"` inference changed to `lifecycle: InferenceLifecycle::Container { name: "llama-test".into() }`, `mode: InferenceMode::Single`, `unload_after_stage: false`. Rebuild `ctx.resources` as `set_max_concurrency` does, keeping an `Arc<MockInferenceControl>` handle. Script one passing step and run with `wait_for_operator: false`; then `handle.calls()` contains `InferenceCall::Stop("router".into())`.
   - `stale_operator_decision_is_drained` (`#[tokio::test]`): send `OperatorDecision::Approve` on `engine.operator_sender()` *before* the run; script the orchestrator to `Escalate` on step 1; spawn `run` with `wait_for_operator: true`. Wait (timeout 5 s) for `EngineEvent::AwaitingOperator` on `engine.subscribe()`, then assert the run is still pending after 200 ms (`tokio::time::timeout(200ms, &mut handle)` is `Err`). Send `Reject { feedback: "x".into() }`, then cancel and await the handle.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'async fn run_steps' crates/orrery-core/src/engine.rs && grep -q 'while rx.try_recv().is_ok() {}' crates/orrery-core/src/engine.rs && grep -q 'inference shutdown failed: {err}' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(engine::tests::run_stops_started_inference_on_exit|engine::tests::stale_operator_decision_is_drained): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 70: Fresh Step Reports and Keychain Notices

* **Recommended Command:** `/goal`

Scope note: F59-5 (after `reset --step`, old reports feed the new prompts) and F59-3 (an unreadable keychain entry silently skips redaction).

```text
[STEP_ID]
STEP 70: Fresh Step Reports and Keychain Notices

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
  - crates/orrery-core/src/checkpoint.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1384-1387 (§9.14.6; binding)
  - crates/orrery-core/src/checkpoint.rs:422-470 (CheckpointManager: save_report, load_reports, delete_reports)
  - crates/orrery-core/src/engine.rs `run_steps` (after Step 69): the fresh-start `else` branch that sets `StepStatus::InProgress` and `state.round = 0`; `build_engine` (~1947-1990: the agent and server secret loops, the hub creation, `referenced_mcp_servers`); the test helper `init_project_with_agents`
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Report file naming and the `delete_reports` behavior are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'async fn run_steps' crates/orrery-core/src/engine.rs && ! grep -q 'pub fn clear_step' crates/orrery-core/src/checkpoint.rs && sed -n 1384p $S | grep -q '^#### 9.14.6 '

[MUTATION_SPEC]
1. checkpoint.rs: `pub fn clear_step(&self, step_number: usize) -> Result<()>` removes `self.root.join(format!("step_{step_number:03}"))` with `fs::remove_dir_all` when it exists. Use the same directory expression `save_report` uses; `NotFound` → `Ok`, other errors → `Error::Io`.
2. engine.rs fresh-start branch: before the `StepRunState` is built, `self.ctx.checkpoints.clear_step(step.number)?;`.
3. engine.rs `build_engine`: `let mut keychain_notices: Vec<String> = Vec::new();`.
   - Agent loop: only for agents whose id appears in some `project.stages[*].agent_ids`, `match backends.secrets.get(service, account) { Ok(secret) => redactor.register(&secret)?, Err(err) => keychain_notices.push(format!("keychain entry {service}/{account} for agent '{}' could not be read ({err}); it will not be redacted", agent.agent_id)) }`.
   - Server loop: only for servers in `referenced_mcp_servers(&project, &roles)` (compute it once before both loops and reuse it for the connect loop), with `"MCP server '<id>'"` in the message.
   - After the hub is created, emit each as `StreamEvent::notice(..)` with `?`.
4. Tests:
   - `checkpoint::tests::clear_step_removes_reports`: save two reports for step 2, then `clear_step(2)`; `load_reports(2, None)` is empty, and `clear_step(9)` is `Ok`.
   - `engine::tests::fresh_step_start_clears_old_reports`: before the run, `save_report` a stale report for step 1 with summary `"stale-from-before-reset"`. Run one passing step; no report from `load_reports(1, None)` has that summary.
   - `engine::tests::build_engine_warns_on_unreadable_keychain`: from `init_project_with_agents(dir, true)`, add a `CloudApi` agent `cloud-missing` (keychain `orrery.test.missing` / `testuser`, endpoint `https://api.example.com/v1/chat/completions`) and assign it to stage 1, with no secret stored. `build_engine` is `Ok`, and `recent(50)` has a `SystemNotice` containing `"orrery.test.missing/testuser for agent 'cloud-missing' could not be read"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn clear_step' crates/orrery-core/src/checkpoint.rs && grep -q 'checkpoints.clear_step(step.number)?' crates/orrery-core/src/engine.rs && grep -q 'it will not be redacted' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(checkpoint::tests::clear_step_removes_reports|engine::tests::fresh_step_start_clears_old_reports|engine::tests::build_engine_warns_on_unreadable_keychain): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 71: CLI Container Cleanup

* **Recommended Command:** `/goal`

Scope note: F61-1: cancelling during an AgentCli orchestrator turn leaks the container (secret env included) and its `agent_io` dir, and the first resume fails with a name conflict.

```text
[STEP_ID]
STEP 71: CLI Container Cleanup

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
  - crates/orrery-core/src/docker.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1388-1392 (§9.14.7; binding)
  - crates/orrery-core/src/llm.rs `CliContainerDriver::run_turn` (~1073-1230: `let name = cli_container_name(..)`, the `for _attempt in 0..2` loop)
  - crates/orrery-core/src/docker.rs `ResourceManager::end_stage` (~920-942: the remove + agent_io cleanup pattern to copy), `agent_io_dir` (~622), `MockContainerBackend` and `MockCall`
  - crates/orrery-core/src/engine.rs `handle_cancel` (~993-1012)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `end_stage`, `begin_stage` and `run_to_completion` stay as they are.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'pub async fn remove_cli_agent' crates/orrery-core/src/docker.rs && sed -n 1388p $S | grep -q '^#### 9.14.7 '

[MUTATION_SPEC]
1. llm.rs `run_turn`: right after `let name = cli_container_name(..);`, `self.backend.remove(&name).await?;` (backends return `Ok` for a missing container).
2. docker.rs `impl ResourceManager`: `pub async fn remove_cli_agent(&self, run_id: &uuid::Uuid, agent_id: &str) -> Result<()>` calls `self.backend.remove(&cli_container_name(run_id, agent_id)).await?`, then removes `agent_io_dir(&self.project_root, run_id, agent_id)` with `fs::remove_dir_all` when it exists (`Error::Io` on failure).
3. engine.rs `handle_cancel`: when the `current_lease` `take()` yields `None`, for each id in `self.ctx.project.stages.first().map(|s| s.agent_ids.clone()).unwrap_or_default()` whose agent (`self.find_agent(id)?`) has `AgentBackend::AgentCli { .. }`, call `self.ctx.resources.remove_cli_agent(&state.run_id, id).await?;`.
4. Tests:
   - `docker::tests::remove_cli_agent_removes_container_and_io`: create `agent_io_dir(root, &run_id, "cli-orch")/in`; `remove_cli_agent` → the mock backend recorded `MockCall::Remove(..)` of `cli_container_name(&run_id, "cli-orch")`, and the dir is gone.
   - `llm::tests::cli_driver_removes_stale_container_first`: with `MockContainerBackend`, the first recorded call naming that container is `MockCall::Remove(name)`, before `MockCall::RunToCompletion { name, .. }`.
   - `engine::tests::cancel_without_lease_removes_cli_orchestrator`: `test_context` whose stage-0 agent is changed to `AgentBackend::AgentCli` (image `busybox@sha256:` plus 64 zeros, command `["sh"]`, `WorkspaceMount::None`, 256 MB, 60 s, no keychain or secret_env), with `ctx.resources` rebuilt around a kept `Arc<MockContainerBackend>`; call `handle_cancel(&mut state, 0, &step)` directly with no lease open; the backend recorded `MockCall::Remove(cli_container_name(&state.run_id, <stage-0 agent id>))`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub async fn remove_cli_agent' crates/orrery-core/src/docker.rs && grep -q 'remove_cli_agent(&state.run_id' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(docker::tests::remove_cli_agent_removes_container_and_io|llm::tests::cli_driver_removes_stale_container_first|engine::tests::cancel_without_lease_removes_cli_orchestrator): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 72: Container Output Line Splitter

* **Recommended Command:** `/goal`

Scope note: F61-2: container output is split into lines per Docker frame, so lines break, and a secret split across two frames escapes redaction.

```text
[STEP_ID]
STEP 72: Container Output Line Splitter

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/docker.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1393-1398 (§9.14.8; binding)
  - crates/orrery-core/src/docker.rs `BollardContainerBackend::run_to_completion` (~1145-1230; the `reader` async block with `text.lines()`)
  - crates/orrery-core/src/llm.rs `LineBuffer` (~635: the UTF-8 boundary logic to mirror; read only)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The `ContainerBackend` trait and the `on_output` callback type are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'for line in text.lines()' crates/orrery-core/src/docker.rs && ! grep -q 'struct LineSplitter' crates/orrery-core/src/docker.rs && sed -n 1393p $S | grep -q '^#### 9.14.8 '

[MUTATION_SPEC]
1. docker.rs: `const MAX_OUTPUT_LINE_BYTES: usize = 4096;` and `#[derive(Default)] pub(crate) struct LineSplitter { pending: Vec<u8> }` with:
   - `pub(crate) fn push(&mut self, bytes: &[u8]) -> Vec<String>`: extend `pending`; while it holds `b'\n'`, drain through it and push `String::from_utf8_lossy` of the line without `\n` and a trailing `\r`. Then, while `pending.len() >= MAX_OUTPUT_LINE_BYTES`, cut at `match std::str::from_utf8(&pending[..MAX_OUTPUT_LINE_BYTES]) { Ok(_) => MAX_OUTPUT_LINE_BYTES, Err(e) if e.valid_up_to() > 0 => e.valid_up_to(), Err(_) => MAX_OUTPUT_LINE_BYTES }` and push that piece.
   - `pub(crate) fn finish(&mut self) -> Option<String>`: the non-empty remainder, lossy-decoded, leaving `pending` empty.
2. `run_to_completion`'s `reader`: `let mut out_lines = LineSplitter::default(); let mut err_lines = LineSplitter::default();`. For each chunk, pick the splitter by `matches!(chunk, LogOutput::StdErr { .. })` (import `bollard::container::LogOutput` as the existing code needs) and pass `chunk.as_ref()`; call `on_output` for every returned line. After the stream ends, call `finish()` on both.
3. Tests in `docker::tests`:
   - `line_splitter_joins_across_chunks`: push `b"hel"` → `[]`; push `b"lo\r\nwor"` → `["hello"]`; `finish()` → `Some("wor")`.
   - `line_splitter_keeps_utf8_across_chunks`: push `b"caf\xc3"` → `[]`; push `b"\xa9\n"` → `["café"]`.
   - `line_splitter_caps_long_lines`: push 4095 × `b'a'` followed by `"é"` (2 bytes, 4097 bytes total, no newline) → `["a"×4095]`, cut at the char boundary before `é`; then push `b"\n"` → `["é"]`. (Two pushes: the cap only applies to `pending` with no newline left in it.)
   - `line_splitter_keeps_secret_whole`: push `b"token sk-abc"` then `b"def123\n"` → exactly `["token sk-abcdef123"]`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub(crate) struct LineSplitter' crates/orrery-core/src/docker.rs && ! grep -q 'for line in text.lines()' crates/orrery-core/src/docker.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(docker::tests::line_splitter_joins_across_chunks|docker::tests::line_splitter_keeps_utf8_across_chunks|docker::tests::line_splitter_caps_long_lines|docker::tests::line_splitter_keeps_secret_whole): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 73: TUI Decisions, Interrupt and Shutdown on Error

* **Recommended Command:** `/goal`

Scope note: The TUI half of F63-4 (Approve/Reject stay live after sending), F63-1 (`[Stop]` with no task overwrites the message) and F63-8 (a terminal error skips `controller.shutdown`).

```text
[STEP_ID]
STEP 73: TUI Decisions, Interrupt and Shutdown on Error

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1399-1403 (§9.14.9; binding), 1261-1272 (§9.11.9, §9.11.10)
  - crates/orrery-tui/src/bridge.rs:372-375 (`AppAction::Interrupt`), 376-392 (Approve/Reject running branch), 601-710 (`run_tui`), 1136 (test `approve_reaches_operator_channel`), 1529-1560 (setup of `approve_without_running_engine_resumes`: an `AwaitingOperator` checkpoint with no task)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `EngineController` method signatures and channel sizes are unchanged. Don't change `app.rs` or `widgets.rs`.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'app.status_message = Some("Stopping…".into());' crates/orrery-tui/src/bridge.rs && ! grep -q 'let loop_result' crates/orrery-tui/src/bridge.rs && sed -n 1399p $S | grep -q '^#### 9.14.9 '

[MUTATION_SPEC]
1. Approve/Reject while `self.has_running_task()`: match `tx.try_send(decision)`: `Ok(())` → `app.awaiting_operator = false; app.status = RunStatus::Running;`; `Err(Full(_))` → the existing message; `Err(Closed(_))` → `app.status_message = Some("engine stopped; decision not sent".into())`.
2. `AppAction::Interrupt`: when `!self.has_running_task()`, set `app.status_message` to `"no run in progress; Approve or Reject the waiting step"` if `app.awaiting_operator`, else `"no run in progress"`, and return without cancelling. Otherwise keep the current behavior.
3. `run_tui`: wrap everything from `loop {` to the loop's end in `let loop_result: Result<()> = async { loop { … } Ok(()) }.await;`, so the existing `?`s and `return Err(..)`s inside become the block's. Then `controller.shutdown(&mut app).await;` and `loop_result`. Setup before the loop (terminal enter, sizes) keeps its `?`s.
4. Tests (in `bridge::tests`):
   - `approve_while_running_clears_awaiting`: as `approve_reaches_operator_channel`; after `handle(&AppAction::Approve, ..)`, `app.awaiting_operator` is `false` and `app.status == RunStatus::Running`.
   - `interrupt_without_task_keeps_awaiting_state`: as the setup of `approve_without_running_engine_resumes` (checkpoint `AwaitingOperator`, `load_initial_app`, no task); `handle(&AppAction::Interrupt, ..)` leaves `app.status == RunStatus::AwaitingOperator` and `app.awaiting_operator == true`, with the waiting-step message.
   `run_tui` needs a real terminal, so item 3 is checked by POST_GATE greps, not by a test.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'let loop_result: Result<()> = async' crates/orrery-tui/src/bridge.rs && grep -q 'no run in progress; Approve or Reject the waiting step' crates/orrery-tui/src/bridge.rs && test "$(grep -c 'controller.shutdown(&mut app).await;' crates/orrery-tui/src/bridge.rs)" -ge 1 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(bridge::tests::approve_while_running_clears_awaiting|bridge::tests::interrupt_without_task_keeps_awaiting_state): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 74: TUI Drain and Checkpoint Reconcile

* **Recommended Command:** `/goal`

Scope note: F63-3 (`drain_channels` is never called, and engine lag is silent) and F63-5 (TUI startup and ingest compare step counts instead of reconciling, and drop load errors).

```text
[STEP_ID]
STEP 74: TUI Drain and Checkpoint Reconcile

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1404-1412 (§9.14.10; binding)
  - crates/orrery-tui/src/bridge.rs: `drain_channels` (~136), the run task's `eng_sub.recv()` arm (`RecvError::Lagged(_) => {}`), `handle(AppAction::IngestPlan(..))` (~339-364), `load_initial_app` (~572-600), `run_tui` (after Step 73)
  - crates/orrery-core/src/checkpoint.rs:283-375 (`ReconcileOutcome`, `WorkflowCheckpoint::reconcile`; it returns `Err(Error::Engine)` with message "step edited; run `orrery reset --step N`" for an edited passed or active step)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `App` fields and `step()` are unchanged. Don't save the reconciled checkpoint (the TUI only views it).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'let loop_result: Result<()> = async' crates/orrery-tui/src/bridge.rs && ! grep -q 'fn load_step_statuses' crates/orrery-tui/src/bridge.rs && sed -n 1404p $S | grep -q '^#### 9.14.10 '

[MUTATION_SPEC]
1. `run_tui`: right after `let effects = step(&mut app, &regions, input);`, call `controller.drain_channels(&mut app, &regions); needs_redraw = true;`.
2. The run task's `eng_sub` arm: `Err(RecvError::Lagged(n)) => { let _ = stream_tx.try_send(StreamEvent::notice(format!("{n} engine events dropped; statuses may be stale"))); }`. The `let _ =` is spec'd, matching the stream-drop handling beside it.
3. `fn load_step_statuses(store: &ConfigStore, plan: &ImplementationPlan, app: &mut App) -> Vec<StepStatus>`: `CheckpointManager::new(store.orrery_dir().join("checkpoints")).load_state()`.
   - `Ok(Some(mut cp))` → `match cp.reconcile(plan) { Ok(_) => { set the AwaitingOperator fields as today when cp.step_statuses.get(cp.current_step_index) == Some(&StepStatus::AwaitingOperator); cp.step_statuses } Err(e) => { app.status_message = Some(e.to_string()); vec![Pending; plan.steps.len()] } }`
   - `Ok(None)` → all `Pending`.
   - `Err(e)` → all `Pending`, with `status_message = e.to_string()`.
4. `load_initial_app`: use it. Replace the `if let Ok(..) && let Ok(..)` with a `match` that sets `app.status_message = Some(format!("plan not loaded: {e}"))` on either error.
5. `IngestPlan`: after the plan parses and saves, `app.step_statuses = load_step_statuses(&self.store, &plan, app);` in place of the all-`Pending` reset.
6. Tests (in `bridge::tests`):
   - `initial_load_reconciles_appended_step`: a checkpoint for the 3-step plan with step 1 `Passed`; append a 4th step to the plan file; `load_initial_app` → statuses `[Passed, Pending, Pending, Pending]`.
   - `initial_load_reports_edited_step`: the same checkpoint; edit step 1's body → statuses all `Pending`, and `status_message` contains `"step edited"`.
   - `ingest_reconciles_checkpoint`: the same as the first, through `handle(&AppAction::IngestPlan(path), ..)`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn load_step_statuses' crates/orrery-tui/src/bridge.rs && grep -q 'controller.drain_channels(&mut app, &regions);' crates/orrery-tui/src/bridge.rs && grep -q 'engine events dropped; statuses may be stale' crates/orrery-tui/src/bridge.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(bridge::tests::initial_load_reconciles_appended_step|bridge::tests::initial_load_reports_edited_step|bridge::tests::ingest_reconciles_checkpoint): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 75: TUI Slot Replace and Footer Keys

* **Recommended Command:** `/goal`

Scope note: F63-2 (High: picking an agent for an occupied slot appends it) and F63-6 (footer Left/Right walk onto the hidden Approve/Reject).

```text
[STEP_ID]
STEP 75: TUI Slot Replace and Footer Keys

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/app.rs
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1413-1417 (§9.14.11; binding)
  - crates/orrery-core/src/config.rs `ProjectConfig::assign_agent` / `remove_agent` (~521-560: the clone, validate_staging, commit pattern)
  - crates/orrery-tui/src/app.rs: `apply_selector_choice` agent branch (~607-645), `open_stage_canvas_selector` (~841-875), `handle_footer_menu_key` (~905-960), `approve_visible` (~224)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No change to `assign_agent`, `remove_agent`, the selector options or `widgets.rs`.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && ! grep -q 'pub fn replace_agent' crates/orrery-core/src/config.rs && sed -n 1413p $S | grep -q '^#### 9.14.11 '

[MUTATION_SPEC]
1. config.rs: `pub fn replace_agent(&mut self, index: usize, slot: usize, agent_id: impl Into<String>) -> Result<()>`. An out-of-range `index` gets the same error as `assign_agent`. An out-of-range `slot` → `Error::Validation { field: format!("stages[{index}].agent_ids"), message: format!("slot {slot} out of bounds (len {})", ..) }`. Otherwise clone, set `next.stages[index].agent_ids[slot] = agent_id.into()`, `next.validate_staging()?`, then `*self = next`.
2. app.rs `apply_selector_choice`, agent branch, not `REMOVE_OPTION`: if `slot < self.config.stages[stage].agent_ids.len()`, call `replace_agent(stage, slot, option.clone())`; on `Ok` → `refresh_warnings()` and `vec![AppAction::AssignAgent { stage, agent_id: option }]`; on `Err` → `status_message`. Otherwise keep `assign_agent`.
3. `handle_footer_menu_key`: first, `if !self.approve_visible() && (1..=2).contains(&self.footer_index) { self.footer_index = 0; }`. `Right`: increment, and while hidden and on 1 or 2, increment again (stop at 6). `Left`: decrement, and while hidden and on 1 or 2, decrement again (stop at 0).
4. Tests:
   - `config::tests::replace_agent_swaps_slot`: stage 1 `[a, b]` → `replace_agent(1, 0, "c")` gives `[c, b]`; slot 5 → `Err(Validation)`; replacing stage 0's agent keeps the 1-agent limit satisfied.
   - `app::tests::selector_replaces_occupied_slot`: stage 0 holds one agent and the roles/agents list has a second; open the selector on slot 1 (the agent slot), choose the second agent → `config.stages[0].agent_ids == [second]` and the actions are `[AssignAgent { stage: 0, agent_id: second }]`.
   - `app::tests::footer_nav_skips_hidden_buttons`: `awaiting_operator = false`, focus `FooterMenu`, index 0, `Right` → 3, `Left` → 0; with `awaiting_operator = true`, `Right` from 0 → 1.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn replace_agent' crates/orrery-core/src/config.rs && grep -q 'replace_agent(stage, slot' crates/orrery-tui/src/app.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(config::tests::replace_agent_swaps_slot): test$')" -eq 1 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^(app::tests::selector_replaces_occupied_slot|app::tests::footer_nav_skips_hidden_buttons): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---
### STEP 76: CLI: add-cli Hyphens, Status Reconcile, Startup Notices

* **Recommended Command:** `/goal`

Scope note: F64-2 (High: `agent add-cli --command sh -c ..` exits 2 on `-c`), F64-1 (`status` doesn't reconcile the checkpoint) and F64-3 (`run` misses the engine's startup notices, and lag is silent).

```text
[STEP_ID]
STEP 76: CLI: add-cli Hyphens, Status Reconcile, Startup Notices

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/args.rs
  - crates/orrery-cli/src/commands.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1418-1421 (§9.14.12; binding), 581-620 (§7)
  - crates/orrery-cli/src/args.rs:180-215 (`AgentCmd::AddCli`, `#[arg(long, required = true, num_args = 1..)] command`), the args tests module
  - crates/orrery-cli/src/commands.rs:731-780 (`dispatch_run`), 827-870 (`dispatch_status`), the test `logs_tail_shows_redacted_lines` (CLI test harness pattern)
  - crates/orrery-core/src/engine.rs:342-344 (`subscribe_stream`: add the accessor next to it)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Only the `recent_stream` accessor is added to engine.rs. No other flag changes.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'num_args = 1..)]' crates/orrery-cli/src/args.rs && ! grep -q 'pub fn recent_stream' crates/orrery-core/src/engine.rs && sed -n 1418p $S | grep -q '^#### 9.14.12 '

[MUTATION_SPEC]
1. args.rs `AddCli.command`: `#[arg(long, required = true, num_args = 1.., allow_hyphen_values = true, help = "Container command and its arguments; must be the last option")]`.
2. engine.rs: `pub fn recent_stream(&self, limit: usize) -> Result<Vec<StreamEvent>> { self.ctx.stream.recent(limit) }`, next to `subscribe_stream`.
3. commands.rs `dispatch_run`: right after `let mut stream_rx = engine.subscribe_stream();`, `for ev in engine.recent_stream(usize::MAX)? { write_stdout_line(ctx, &format_stream_event(&ev))?; }`. In the `select!`, `Err(RecvError::Lagged(n)) => { write_stdout_line(ctx, &format!("[SystemNotice] -: {n} stream events dropped"))?; }`.
4. commands.rs `dispatch_status`: `Some(mut cp) => { cp.reconcile(&plan)?; … }`, so an edited step makes `status` fail with the reconcile error.
5. Tests:
   - in args.rs tests, `add_cli_accepts_hyphenated_command`: parse `["orrery","agent","add-cli","--id","c1","--name","C1","--image","busybox@sha256:<64 zeros>","--mount","ro","--command","sh","-c","true"]` → `command == ["sh","-c","true"]` and mount `ro`;
   - in commands.rs tests, `status_reports_edited_step`: a checkpoint with step 1 `Passed`, then step 1's plan body edited → `dispatch` of `status` is `Err`, and the message contains `"step edited"`;
   - in commands.rs tests, `run_prints_startup_notices`: a project with an MCP server that no role references (as `build_engine_skips_unused_server` sets up in engine tests), using the CLI test backends → captured stdout contains `"is not used by this workflow"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'allow_hyphen_values = true, help = "Container command and its arguments; must be the last option"' crates/orrery-cli/src/args.rs && grep -q 'pub fn recent_stream' crates/orrery-core/src/engine.rs && grep -q 'stream events dropped' crates/orrery-cli/src/commands.rs && grep -q 'cp.reconcile(&plan)?' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-cli --bins --locked -- --list 2>/dev/null | grep -cE '^(args::tests::add_cli_accepts_hyphenated_command|commands::tests::status_reports_edited_step|commands::tests::run_prints_startup_notices): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase I — parked Low tidy-ups (Steps 77–79)

**Phase goal (frozen until the phase ends):** fix the four cheap Low items the operator picked from the Phase H Parked list (2026-11-17): the `LineSplitter` cap gap, the MCP stderr UTF-8 cut, the `remove_role` deletion order, and the §7 `add-cli` synopsis. Minimal fixes only. Design: spec §9.15 (`ORRERY_TECHNICAL_SPECIFICATION.md:1423-1433`).

Written 2026-11-17 against `119aa59`. The §7 synopsis fix is a spec-only change, made by Claude in the planning commit (spec line 598 now ends with `--command <ARG>...`), so it has no step. Each remaining step edits one file.

### STEP 77: Line Splitter Caps Every Line

* **Recommended Command:** `/goal`

Scope note: Parked (Step 72 validation): a line whose `\n` arrives in the same chunk is drained before the cap, so it can exceed `MAX_OUTPUT_LINE_BYTES`.

```text
[STEP_ID]
STEP 77: Line Splitter Caps Every Line

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/docker.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1426-1427 (§9.15.1; binding), 1393-1398 (§9.14.8)
  - crates/orrery-core/src/docker.rs:1052-1091 (`MAX_OUTPUT_LINE_BYTES`, `LineSplitter`, `push`, `finish`), 2584-2620 (the four `line_splitter_*` tests)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `run_to_completion` and the four existing `line_splitter_*` tests are unchanged. `push` and `finish` keep their signatures.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub(crate) struct LineSplitter' crates/orrery-core/src/docker.rs && ! grep -q 'fn char_cut' crates/orrery-core/src/docker.rs && sed -n 1426p $S | grep -q '^#### 9.15.1 '

[MUTATION_SPEC]
1. docker.rs, a private free function next to `LineSplitter`: `fn char_cut(bytes: &[u8]) -> usize` (caller guarantees `bytes.len() >= MAX_OUTPUT_LINE_BYTES`) returning `match std::str::from_utf8(&bytes[..MAX_OUTPUT_LINE_BYTES]) { Ok(_) => MAX_OUTPUT_LINE_BYTES, Err(e) if e.valid_up_to() > 0 => e.valid_up_to(), Err(_) => MAX_OUTPUT_LINE_BYTES }`.
2. A private free function `fn push_capped(lines: &mut Vec<String>, raw: &[u8])`: `let mut rest = raw; while rest.len() > MAX_OUTPUT_LINE_BYTES { let cut = char_cut(rest); lines.push(String::from_utf8_lossy(&rest[..cut]).into_owned()); rest = &rest[cut..]; } lines.push(String::from_utf8_lossy(rest).into_owned());`. Note `>`, not `>=`: a piece of exactly the cap is pushed whole, so there is never an empty trailing piece, and an empty line still yields one `""`.
3. `push`, newline loop: after stripping `\n` and a trailing `\r`, call `push_capped(&mut lines, &raw)` instead of pushing the lossy line directly.
4. `push`, pending loop: keep `while self.pending.len() >= MAX_OUTPUT_LINE_BYTES`, but compute the cut with `let cut = char_cut(&self.pending);`.
5. Tests in `docker::tests`:
   - `line_splitter_caps_newline_terminated_lines`: one `push` of 4095 × `b'a'` + `"é"` (2 bytes) + `b"\n"` → `["a"×4095, "é"]`.
   - `line_splitter_exact_multiple_has_no_empty_piece`: one `push` of 8192 × `b'a'` + `b"\n"` → `["a"×4096, "a"×4096]` (exactly two items).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn char_cut(bytes: &\[u8\]) -> usize' crates/orrery-core/src/docker.rs && grep -q 'fn push_capped(lines: &mut Vec<String>, raw: &\[u8\])' crates/orrery-core/src/docker.rs && grep -q 'while rest.len() > MAX_OUTPUT_LINE_BYTES' crates/orrery-core/src/docker.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^docker::tests::line_splitter_[a-z0-9_]+: test$')" -eq 6 && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(docker::tests::line_splitter_caps_newline_terminated_lines|docker::tests::line_splitter_exact_multiple_has_no_empty_piece): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 78: MCP Stderr Uses the Line Splitter

* **Recommended Command:** `/goal`

Scope note: Parked (Step 68 validation): `spawn_stderr_forwarder`'s `pending.drain(..4096)` can split a multi-byte UTF-8 character, which then shows as U+FFFD on both sides.

```text
[STEP_ID]
STEP 78: MCP Stderr Uses the Line Splitter

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1429-1430 (§9.15.2; binding), 1368-1378 (§9.14.4)
  - crates/orrery-core/src/mcp.rs:648-690 (`spawn_stderr_forwarder`), 1422-1500 (tests `stderr_forwarder_emits_redacted_notices`, `stderr_forwarder_survives_invalid_utf8`, `stderr_forwarder_caps_long_lines`)
  - crates/orrery-core/src/docker.rs: `LineSplitter` (`pub(crate)`, `Default`, `push(&mut self, &[u8]) -> Vec<String>`, `finish(&mut self) -> Option<String>`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES (docker.rs is read-only here). The forwarder's signature, the `[mcp:<id>] ` prefix and blank-line suppression are unchanged. The three existing `stderr_forwarder_*` tests are unchanged and must still pass.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pending.drain(..4096)' crates/orrery-core/src/mcp.rs && grep -q 'fn push_capped' crates/orrery-core/src/docker.rs && sed -n 1429p $S | grep -q '^#### 9.15.2 '

[MUTATION_SPEC]
1. `spawn_stderr_forwarder`'s task body becomes:
   - `use tokio::io::AsyncReadExt; let mut stderr = stderr; let mut chunk = [0u8; 4096]; let mut lines = crate::docker::LineSplitter::default();`
   - `let emit = |text: &str| { if !text.trim().is_empty() { let _ = stream.emit(StreamEvent::notice(format!("[mcp:{server_id}] {text}"))); } };`
   - `while let Ok(n) = stderr.read(&mut chunk).await { if n == 0 { break; } for line in lines.push(&chunk[..n]) { emit(&line); } }`
   - after the loop (EOF or read error alike): `if let Some(rest) = lines.finish() { emit(&rest); }`
   The `pending` vector, the manual newline loop and `pending.drain(..4096)` are removed.
2. Test in `mcp::tests`: `stderr_forwarder_keeps_utf8_at_cut` (`#[tokio::test]`, hub set up as in `stderr_forwarder_caps_long_lines`): input `std::io::Cursor::new(..)` of 4095 × `b'a'` + `"é"` + `b"\n"` → `hub.recent(10)` texts are exactly `[format!("[mcp:fs] {}", "a".repeat(4095)), "[mcp:fs] é"]`, and no event text contains `'\u{FFFD}'`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'crate::docker::LineSplitter::default()' crates/orrery-core/src/mcp.rs && ! grep -q 'pending.drain(..4096)' crates/orrery-core/src/mcp.rs && grep -q 'lines.finish()' crates/orrery-core/src/mcp.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(mcp::tests::stderr_forwarder_keeps_utf8_at_cut|mcp::tests::stderr_forwarder_emits_redacted_notices|mcp::tests::stderr_forwarder_survives_invalid_utf8|mcp::tests::stderr_forwarder_caps_long_lines): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 79: remove_role Checks Peers First

* **Recommended Command:** `/goal`

Scope note: Parked (Step 65 validation): `remove_role` deletes the role's TOML before the fallible peer scan, so an unreadable peer file makes the command fail with the role already gone.

```text
[STEP_ID]
STEP 79: remove_role Checks Peers First

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1432-1433 (§9.15.3; binding), 1346-1351 (§9.14.1)
  - crates/orrery-core/src/config.rs:1823-1860 (`ConfigStore::remove_role`), 4190-4220 (test `remove_role_keeps_rules_when_peer_unreadable`: setup pattern)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Only `remove_role`'s body and the two new tests change. The not-found error and the `validate_config_file_id` call are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'fn remove_role_keeps_rules_when_peer_unreadable' crates/orrery-core/src/config.rs && ! grep -q 'let orphaned_rules' crates/orrery-core/src/config.rs && sed -n 1432p $S | grep -q '^#### 9.15.3 '

[MUTATION_SPEC]
1. `remove_role`, after `let role = Self::read_single_role(&roles_dir, &role_toml_path)?;` and before any deletion:
   `let orphaned_rules = match resolve_safe_within_dir(&roles_dir, Path::new(&role.rules_file), "rules_file") { Ok(rules_path) if rules_path.exists() => { let still_used = Self::read_roles_from_dir(&roles_dir)?.iter().any(|other| other.role_id != role.role_id && other.rules_file == role.rules_file); (!still_used).then_some(rules_path) } _ => None };`
   The `other.role_id != role.role_id` filter is required: the role's own TOML still exists during the scan.
2. Then `fs::remove_file(&role_toml_path)` (same `Error::Io` mapping as now), then `if let Some(rules_path) = orphaned_rules { fs::remove_file(&rules_path).map_err(|source| Error::Io { path: rules_path, source })?; }`. The old post-delete `if let Ok(rules_path) = … && rules_path.exists()` block is removed.
3. Tests in `config::tests`:
   - `remove_role_keeps_toml_when_peer_unreadable`: the setup of `remove_role_keeps_rules_when_peer_unreadable` (two roles sharing `shared.md`, plus `broken.toml` containing `role_id = [\n`); `remove_role("role-one")` is `Err`, and both `roles/role-one.toml` and `roles/shared.md` still exist.
   - `remove_role_deletes_own_rules_after_peer_scan`: `init_project`, then `save_role` of `solo` (rules `solo.md`) and `other` (rules `other.md`); `remove_role("solo")` is `Ok`; `roles/solo.toml` and `roles/solo.md` are gone; `roles/other.toml` and `roles/other.md` still exist.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'let orphaned_rules' crates/orrery-core/src/config.rs && grep -q 'other.role_id != role.role_id' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(config::tests::remove_role_keeps_toml_when_peer_unreadable|config::tests::remove_role_deletes_own_rules_after_peer_scan|config::tests::remove_role_keeps_rules_when_peer_unreadable): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase J — secret-handling Lows (Steps 80–83)

**Phase goal (frozen until the phase ends):** fix the three Phase G Lows that touch secret handling, picked by the operator on 2026-11-17:
- F63-9: the TUI shows run errors without redacting them.
- F64-5: the CLI prints run errors without redacting them, and its secret input makes a second plaintext copy.
- F62-5: the MCP HTTP bearer token goes into rmcp as a plain `String`.

Minimal fixes only. Design: spec §9.16 (`ORRERY_TECHNICAL_SPECIFICATION.md:1435-1450`).

Written 2026-11-17 against `2639c13`. The other 13 Phase G Lows stay parked in the validation notes. F63-9's non-secret parts (the build-failure stream notice and clearing `status_message`) aren't in scope.

### STEP 80: Redacted Run Errors in the CLI

* **Recommended Command:** `/goal`

Scope note: F64-5, error half: `main` prints `render_error(&err)` without redaction, so a run error carrying a registered secret reaches stderr in plaintext.

```text
[STEP_ID]
STEP 80: Redacted Run Errors in the CLI

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1438-1440 (§9.16.1; binding)
  - crates/orrery-core/src/engine.rs:342-350 (`subscribe_stream`, `recent_stream`: add `redact` next to them)
  - crates/orrery-core/src/security.rs:158-190 (`SecretRedactor::redact(&self, &str) -> Result<String>`), crates/orrery-core/src/stream_log.rs:316 (`LiveStreamHub::redactor`)
  - crates/orrery-cli/src/commands.rs:37-59 (`render_error`, `exit_code_for`, `write_stdout_line`), 731-786 (`dispatch_run`), 2655-2720 (`push_one_step_pass`, `setup_runnable_project`), 3115-3125 (`InMemorySecretStore::set` usage)
  - crates/orrery-cli/src/main.rs (read-only: its `Err` arm stays as is)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Only `redact` is added to engine.rs. Only `dispatch_run`'s handling of `outcome_res` and one new test change in commands.rs. Errors returned before `build_engine` succeeds keep propagating with `?`.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn recent_stream' crates/orrery-core/src/engine.rs && ! grep -q 'pub fn redact(&self, text: &str) -> String' crates/orrery-core/src/engine.rs && grep -q 'match outcome_res? {' crates/orrery-cli/src/commands.rs && sed -n 1438p $S | grep -q '^#### 9.16.1 '

[MUTATION_SPEC]
1. engine.rs, next to `recent_stream`: `pub fn redact(&self, text: &str) -> String { self.ctx.stream.redactor().redact(text).unwrap_or_else(|_| "[error text withheld: redactor unavailable]".to_string()) }`.
2. commands.rs `dispatch_run`: replace `match outcome_res? {` with `let outcome = match outcome_res { Ok(outcome) => outcome, Err(err) => { let text = engine.redact(&render_error(&err)); writeln!(ctx.err, "{text}").map_err(|source| Error::Io { path: PathBuf::from("<stderr>"), source })?; return Ok(exit_code_for(&err)); } }; match outcome {` (the three outcome arms are unchanged).
3. Tests:
   - `engine::tests::redact_masks_registered_secret`: `test_context`, register `SecretValue::new("sk-test-leak-123")` on `ctx.stream.redactor()`, build the engine; `engine.redact("echo sk-test-leak-123 done")` contains `"[REDACTED]"` and not `"sk-test-leak-123"`.
   - `commands::tests::run_error_is_redacted`: `setup_runnable_project` with a one-step plan and `secrets` holding `orrery.test` / `testuser` = `sk-test-leak-123`. Then, through `ConfigStore::new(dir, None)`: `save_agent` a `CloudApi` agent `cloud-orch` (endpoint `https://api.example.com/v1/chat/completions`, model `cloud-model`, temperature 0.0, 8192 context, keychain `orrery.test` / `testuser`), and `load_project` → `replace_agent(0, 0, "cloud-orch")` → `save_project`. Push `driver.push("cloud-orch", Err(Error::Agent { agent_id: "cloud-orch".into(), message: "upstream echoed sk-test-leak-123".into() }))`. `dispatch(Command::Run { r#loop: false, cycles: None, step: None }, ..)` is `Ok(1)`; the captured stderr contains `"[REDACTED]"` and does not contain `"sk-test-leak-123"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn redact(&self, text: &str) -> String' crates/orrery-core/src/engine.rs && grep -q 'error text withheld: redactor unavailable' crates/orrery-core/src/engine.rs && grep -q 'engine.redact(&render_error(&err))' crates/orrery-cli/src/commands.rs && ! grep -q 'match outcome_res? {' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::redact_masks_registered_secret: test$')" -eq 1 && test "$(cargo test -p orrery-cli --bins --locked -- --list 2>/dev/null | grep -cE '^commands::tests::run_error_is_redacted: test$')" -eq 1

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 81: Redacted Run Errors in the TUI

* **Recommended Command:** `/goal`

Scope note: F63-9, secret half: the run task sends `err.to_string()` straight to `stream_tx`, bypassing the hub's redactor, and `apply_task_result` puts the same text in `status_message`.

```text
[STEP_ID]
STEP 81: Redacted Run Errors in the TUI

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1442-1443 (§9.16.2; binding)
  - crates/orrery-tui/src/bridge.rs:102 (`task: Option<JoinHandle<Result<RunOutcome>>>`), 145-164 (`apply_task_result`), 166-218 (`poll_finished`, `shutdown`), 228-336 (`start_run` and the run task), 865-919 (`setup_project_dir`, `make_mock_backends`)
  - crates/orrery-core/src/engine.rs: `WorkflowEngine::redact(&self, text: &str) -> String` (added by Step 80)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The select loop, `drain_remaining`, and the build-failure behavior (no stream notice) are unchanged. `status_message` is not cleared anywhere new.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'pub fn redact(&self, text: &str) -> String' crates/orrery-core/src/engine.rs && grep -q 'task: Option<JoinHandle<Result<RunOutcome>>>' crates/orrery-tui/src/bridge.rs && sed -n 1442p $S | grep -q '^#### 9.16.2 '

[MUTATION_SPEC]
1. `EngineController.task`: `Option<JoinHandle<std::result::Result<RunOutcome, String>>>`.
2. The run task: `let backends = (make_backends)().map_err(|e| e.to_string())?; let engine = build_engine(&store, backends).await.map_err(|e| e.to_string())?;`. After `drain_remaining(..)`: `let res = res.map_err(|err| engine.redact(&err.to_string()));` then `if let Err(ref text) = res { let _ = stream_tx.try_send(StreamEvent { kind: StreamEventKind::Error, text: text.clone(), ..StreamEvent::notice("") }); }` and return `res`.
3. `apply_task_result(app, res: std::result::Result<std::result::Result<RunOutcome, String>, String>) -> bool`: the `Ok(Err(text))` arm sets `app.status_message = Some(text)`; the other arms are unchanged. `poll_finished` and `shutdown` keep calling it as now.
4. Test in `bridge::tests`: `run_error_is_redacted_in_tui` (`#[tokio::test]`): `setup_project_dir(ValidationMode::Auto)`; `store.save_agent` a `CloudApi` agent `cloud-orch` (endpoint `https://api.example.com/v1/chat/completions`, model `cloud-model`, temperature 0.0, 8192 context, keychain `orrery.test` / `testuser`); `app.config.replace_agent(0, 0, "cloud-orch")` then `store.save_project(&app.config)`. The backends factory returns `make_mock_backends(driver)` with `secrets` replaced by an `InMemorySecretStore` holding `orrery.test` / `testuser` = `sk-test-leak-123`. `driver.push("cloud-orch", Err(Error::Agent { agent_id: "cloud-orch".into(), message: "upstream echoed sk-test-leak-123".into() }))`. `handle(&AppAction::Run, ..)`, then loop `poll_finished` (5 s timeout, 20 ms sleeps) until it returns `true`. Then `app.status == RunStatus::Error`, `app.status_message` contains `"[REDACTED]"` and not `"sk-test-leak-123"`, and no `app.stream` event text contains `"sk-test-leak-123"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'engine.redact(&err.to_string())' crates/orrery-tui/src/bridge.rs && grep -q 'JoinHandle<std::result::Result<RunOutcome, String>>' crates/orrery-tui/src/bridge.rs && ! grep -q 'text: err.to_string(),' crates/orrery-tui/src/bridge.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^bridge::tests::run_error_is_redacted_in_tui: test$')" -eq 1

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 82: MCP Bearer Header

* **Recommended Command:** `/goal`

Scope note: F62-5 (re-reviewed): `cfg.auth_header(secret.expose())` keeps the token in rmcp's `Debug`-deriving config as a plain `Option<String>`, cloned per request, and never checks it is a valid header value.

```text
[STEP_ID]
STEP 82: MCP Bearer Header

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/mcp.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1445-1446 (§9.16.3; binding)
  - crates/orrery-core/src/mcp.rs:604-628 (`McpTransportConfig::StreamableHttp` arm of `connect_with_stream`)
  - crates/orrery-core/src/llm.rs:611-633 (`auth_header_value`: the `Zeroizing` + `set_sensitive` pattern to copy)
  - rmcp 3.9.0 `StreamableHttpClientTransportConfig::custom_headers(HashMap<HeaderName, HeaderValue>)`; `http` 1.5.0 is the only `http` in Cargo.lock, so `reqwest::header` types are the same types
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No Cargo.toml change. The both-or-neither keychain check and the connect timeouts are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && grep -q 'cfg = cfg.auth_header(secret.expose());' crates/orrery-core/src/mcp.rs && ! grep -q 'fn mcp_bearer_headers' crates/orrery-core/src/mcp.rs && sed -n 1445p $S | grep -q '^#### 9.16.3 '

[MUTATION_SPEC]
1. mcp.rs, a free function: `pub(crate) fn mcp_bearer_headers(secret: &SecretValue) -> Result<std::collections::HashMap<reqwest::header::HeaderName, reqwest::header::HeaderValue>>`: `let bearer = zeroize::Zeroizing::new(format!("Bearer {}", secret.expose())); let mut value = reqwest::header::HeaderValue::from_str(&bearer).map_err(|_| Error::Validation { field: "keychain".to_string(), message: "MCP bearer token is not a valid HTTP header value".to_string() })?; value.set_sensitive(true); Ok(std::collections::HashMap::from([(reqwest::header::AUTHORIZATION, value)]))`.
2. The `(Some(svc), Some(acc))` arm: `let secret = secrets.get(svc, acc)?; cfg = cfg.custom_headers(mcp_bearer_headers(&secret)?);`. No `auth_header` call remains.
3. Tests in `mcp::tests`:
   - `mcp_bearer_headers_are_sensitive`: `mcp_bearer_headers(&SecretValue::new("tok-123"))` has exactly one entry, under `AUTHORIZATION`; its value `to_str()` is `"Bearer tok-123"`, `is_sensitive()` is true, and `format!("{value:?}")` does not contain `"tok-123"`.
   - `mcp_bearer_headers_reject_invalid`: `mcp_bearer_headers(&SecretValue::new("bad\ntoken"))` is `Err(Error::Validation { .. })`, and its `to_string()` does not contain `"bad"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub(crate) fn mcp_bearer_headers' crates/orrery-core/src/mcp.rs && grep -q 'cfg.custom_headers(mcp_bearer_headers(&secret)?)' crates/orrery-core/src/mcp.rs && ! grep -q 'auth_header(' crates/orrery-core/src/mcp.rs && grep -q 'set_sensitive(true)' crates/orrery-core/src/mcp.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(mcp::tests::mcp_bearer_headers_are_sensitive|mcp::tests::mcp_bearer_headers_reject_invalid): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 83: Secret Input Without Copies

* **Recommended Command:** `/goal`

Scope note: F64-5, copy half: `trim_end_matches(..).to_string()` allocates a second plaintext copy of the secret and drops the original without zeroizing it.

```text
[STEP_ID]
STEP 83: Secret Input Without Copies

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/main.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1448-1449 (§9.16.4; binding)
  - crates/orrery-cli/src/main.rs:15-37 (`read_secret_from_stdin`, `read_secret_input`)
  - crates/orrery-cli/src/commands.rs:690-700 (the caller moves the `String` into `SecretValue::new`, which zeroizes on drop)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `main`'s body and both functions' signatures are unchanged. No new dependency.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test "$(grep -c "trim_end_matches(\['\\\\r', '\\\\n'\]).to_string()" crates/orrery-cli/src/main.rs)" -eq 2 && ! grep -q 'fn trim_line_end_in_place' crates/orrery-cli/src/main.rs && sed -n 1448p $S | grep -q '^#### 9.16.4 '

[MUTATION_SPEC]
1. main.rs: `fn trim_line_end_in_place(mut s: String) -> String { let len = s.trim_end_matches(['\r', '\n']).len(); s.truncate(len); s }`.
2. `read_secret_from_stdin` returns `Ok(trim_line_end_in_place(line))`; `read_secret_input`'s TTY branch returns `Ok(trim_line_end_in_place(value))`. No `.to_string()` copy of the secret remains.
3. A `#[cfg(test)] mod tests` at the end of main.rs with `trim_line_end_keeps_allocation`: `let mut s = String::with_capacity(32); s.push_str("tok\r\n"); let ptr = s.as_ptr(); let out = trim_line_end_in_place(s);` → `out == "tok"` and `out.as_ptr() == ptr`; and `trim_line_end_in_place("plain".to_string()) == "plain"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn trim_line_end_in_place(mut s: String) -> String' crates/orrery-cli/src/main.rs && ! grep -q "trim_end_matches(\['\\\\r', '\\\\n'\]).to_string()" crates/orrery-cli/src/main.rs && test "$(cargo test -p orrery-cli --bins --locked -- --list 2>/dev/null | grep -cE '^tests::trim_line_end_keeps_allocation: test$')" -eq 1

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---


## Phase K — chrono to jiff, then GitHub CI (Steps 84–87)

**Phase goal (frozen until the phase ends):** drop `chrono` from Orrery's own code in favor of `jiff` (RUSTSEC-2026-0543: chrono is unmaintained), and add the project's first GitHub Actions workflow so the fmt, clippy and test gates run on every push and pull request.

Written 2026-11-18 against `96b5270`. Facts checked while planning:
- `chrono` is used only in `orrery-core` (`checkpoint.rs`, `stream_log.rs`, `llm.rs`, `engine.rs`). `orrery-tui` and `orrery-cli` have no direct use.
- `rmcp 3.9` still depends on `chrono` (`cargo tree -i chrono`). Step 84 removes only our direct dependency. The RUSTSEC-2026-0543 ignore in `deny.toml` stays until rmcp drops chrono; Step 85 rewords it.
- `jiff::Timestamp` serializes through `Display` as RFC 3339 with a `Z` suffix, the same shape chrono wrote (`2026-11-13T18:00:00Z`, or nine fractional digits). Step 84 pins that with tests instead of assuming it, because checkpoints and the live stream log written by older builds must still load.
- The live-backend tests sit behind the `live-backends` feature (`crates/orrery-core/tests/live_*.rs`, `required-features`), so `cargo test --workspace --locked` already skips them. CI needs no exclusion list and no secrets.
- `dbus-secret-service-keyring-store` needs `libdbus-1-dev` and `pkg-config` on the build host, so the CI runner must install them (Step 86).
- Out of scope, by operator choice (2026-11-18): CI jobs for `cargo deny`, `cargo llvm-cov` and aarch64. Those gates stay in each step's `POST_GATE`.

### STEP 84: Swap chrono for jiff in orrery-core

* **Recommended Command:** `/goal`

Scope note: the timestamp type is shared by four files, so none of them compiles alone. This is one mechanical step over four source files. Splitting it would leave a tree that doesn't build.

```text
[STEP_ID]
STEP 84: orrery-core uses jiff::Timestamp instead of chrono::DateTime<Utc>; chrono is no longer a direct dependency

[SCOPE_LOCK]
ALLOWED_FILES:
  - Cargo.toml
  - Cargo.lock
  - crates/orrery-core/Cargo.toml
  - crates/orrery-core/src/checkpoint.rs
  - crates/orrery-core/src/stream_log.rs
  - crates/orrery-core/src/llm.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - crates/orrery-core/src/checkpoint.rs:215-218, 255-258 (`StampedReport.recorded_at`, `HookResult.recorded_at`)
  - crates/orrery-core/src/checkpoint.rs:556-562 (`mod tests` imports), 852, 929, 984, 1286 (`Utc.with_ymd_and_hms(..)` in tests)
  - crates/orrery-core/src/stream_log.rs:9, 31, 42 (`StreamEvent.timestamp`, `StreamEvent::notice`)
  - crates/orrery-core/src/llm.rs:8, 1246 (imports)
  - crates/orrery-core/src/engine.rs:6, 2138 (imports)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Field names, field order, `serde` attributes and every public signature other than the timestamp type stay the same. No new dependency besides `jiff`. orrery-tui and orrery-cli are not edited (they have no direct chrono use).

[PRE_GATE]
grep -q '^chrono = ' Cargo.toml && ! grep -q '^jiff' Cargo.toml && test "$(grep -rlE 'use chrono|chrono::' crates | wc -l)" -eq 4 && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. Cargo.toml (workspace `[workspace.dependencies]`): remove the `chrono = ...` line. Add, in alphabetical position (after `futures-util`): `jiff = { version = "1.0.1", default-features = false, features = ["std", "serde"] }`. Why: only UTC instants are used, so no time-zone database is needed; `std` gives `Timestamp::now()`.
2. crates/orrery-core/Cargo.toml: replace `chrono.workspace = true` with `jiff.workspace = true`, kept in alphabetical order (after `futures-util.workspace = true`, before `keyring-core.workspace = true`).
3. checkpoint.rs: `recorded_at: chrono::DateTime<chrono::Utc>` becomes `recorded_at: jiff::Timestamp` in both structs (lines 217 and 257). In `mod tests`, replace `use chrono::{TimeZone, Utc};` with `use jiff::Timestamp;`. Replace each `Utc.with_ymd_and_hms(2026, 11, 13, H, M, 0).unwrap()` (4 sites) with `"2026-11-13THH:MM:00Z".parse::<Timestamp>().unwrap()` using the same hour and minute (18:00, 19:00, 19:30, 19:30).
4. stream_log.rs: `use chrono::{DateTime, Utc};` becomes `use jiff::Timestamp;`; `timestamp: DateTime<Utc>` becomes `timestamp: Timestamp`; every `Utc::now()` becomes `Timestamp::now()` (non-test code at line 42 and 251). In `mod tests`, `use chrono::Utc;` becomes `use jiff::Timestamp;` and its `Utc::now()` becomes `Timestamp::now()`.
5. llm.rs and engine.rs: `use chrono::Utc;` becomes `use jiff::Timestamp;` at the top of each file and in each `mod tests`; every `Utc::now()` becomes `Timestamp::now()`. No other edits. Keep the import blocks in rustfmt order.
6. Tests (all use `serde_json`, already a dependency):
   - checkpoint.rs `mod tests`, `stamped_report_reads_legacy_timestamp`: build a `StampedReport { step_number: 7, plan_step_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(), round: 1, stage_index: 0, role_id: "orchestrator".to_string(), agent_id: "local-orch".to_string(), recorded_at: Timestamp::UNIX_EPOCH, report: base_report(ReportStatus::Dispatch) }`. For each text in `["2026-11-13T18:00:00Z", "2026-11-13T10:00:00.123456789Z"]`: `let mut value = serde_json::to_value(&stamped).unwrap(); value["recorded_at"] = serde_json::json!(text);` then `let decoded: StampedReport = serde_json::from_value(value).unwrap();`. Assert `decoded.recorded_at == text.parse::<Timestamp>().unwrap()` and `serde_json::to_value(&decoded).unwrap()["recorded_at"] == text`. Why: pins that timestamps written by the chrono-era builds still load and re-serialize unchanged.
   - checkpoint.rs `mod tests`, `hook_result_reads_legacy_timestamp`: the same loop over a `HookResult { event: HookEvent::StepStart, step_number: 2, tool: "git:status".to_string(), success: true, output: "clean".to_string(), recorded_at: Timestamp::UNIX_EPOCH }`.
   - stream_log.rs `mod tests`, `stream_event_reads_legacy_timestamp`: the same loop over `StreamEvent::notice("hello")`, using the `"timestamp"` key.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -rnE 'use chrono|chrono::|^chrono' crates Cargo.toml && grep -q '^jiff = ' Cargo.toml && grep -q '^jiff.workspace = true' crates/orrery-core/Cargo.toml && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(checkpoint::tests::stamped_report_reads_legacy_timestamp|checkpoint::tests::hook_result_reads_legacy_timestamp|stream_log::tests::stream_event_reads_legacy_timestamp): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 85: Spec and deny.toml Follow Step 84

* **Recommended Command:** `/goal`

Scope note: docs and config only. The advisory ignore stays because `rmcp 3.9` still pulls in chrono.

```text
[STEP_ID]
STEP 85: the spec names jiff instead of chrono, and the deny.toml ignore says chrono now arrives only through rmcp

[SCOPE_LOCK]
ALLOWED_FILES:
  - ORRERY_TECHNICAL_SPECIFICATION.md
  - deny.toml
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:96 (dependency table row for `chrono`)
  - ORRERY_TECHNICAL_SPECIFICATION.md:737, 752 (`recorded_at: chrono::DateTime<chrono::Utc>`)
  - ORRERY_TECHNICAL_SPECIFICATION.md:782 (`StreamEvent`, `timestamp: DateTime<Utc>`)
  - ORRERY_TECHNICAL_SPECIFICATION.md:1318 (`timestamp: Utc::now()`)
  - deny.toml:4-9 (the `ignore` entry)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The RUSTSEC-2026-0543 entry is NOT removed: `cargo deny check` fails without it while rmcp depends on chrono. No other spec line changes.

[PRE_GATE]
grep -q 'jiff = ' Cargo.toml && test "$(grep -cE 'chrono|Utc' ORRERY_TECHNICAL_SPECIFICATION.md)" -eq 5 && grep -q 'RUSTSEC-2026-0543' deny.toml

[MUTATION_SPEC]
1. Spec line 96: replace the `chrono` table row with `| `jiff` | `jiff = { version = "1.0.1", default-features = false, features = ["std", "serde"] }` | Timestamps: `jiff::Timestamp` (UTC instant, RFC 3339 with `Z` on the wire). Replaced chrono in Step 84 | Step 7 |`. Keep the row's position and column count.
2. Spec lines 737 and 752: `chrono::DateTime<chrono::Utc>` becomes `jiff::Timestamp`.
3. Spec line 782: `timestamp: DateTime<Utc>` becomes `timestamp: Timestamp`. Spec line 1318: `Utc::now()` becomes `Timestamp::now()`.
4. deny.toml: replace the comment and `reason` of the RUSTSEC-2026-0543 entry. Comment: `# chrono is unmaintained (all versions; no safe upgrade). orrery-core no longer uses it (Step 84), but rmcp 3.9 still depends on it, so it stays in the tree. Revisit when rmcp drops chrono. Accepted 2026-11-17, narrowed 2026-11-18.` Reason: `"chrono unmaintained; transitive via rmcp 3.9 only, no safe upgrade"`. The `id` stays.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && test "$(grep -cE 'chrono|Utc' ORRERY_TECHNICAL_SPECIFICATION.md)" -eq 1 && grep -q 'Replaced chrono in Step 84' ORRERY_TECHNICAL_SPECIFICATION.md && grep -q 'transitive via rmcp 3.9 only' deny.toml && grep -q 'RUSTSEC-2026-0543' deny.toml

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 86: GitHub Actions CI Workflow

* **Recommended Command:** `/goal`

Scope note: the repo has no `.github/` directory. One workflow, one job. `deny`, `llvm-cov` and aarch64 are out of scope by operator choice.

```text
[STEP_ID]
STEP 86: .github/workflows/ci.yml runs fmt, clippy and tests on every push to main and every pull request

[SCOPE_LOCK]
ALLOWED_FILES:
  - .github/workflows/ci.yml (NEW)
READ_ONLY_REFS:
  - Cargo.toml:5 (`rust-version = "1.99.1"`)
  - README.md:5-13 (Requirements: libdbus-1-dev and pkg-config)
  - crates/orrery-core/Cargo.toml:`[[test]]` tables (the live tests need the `live-backends` feature, so a default test run skips them)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No secrets, no deploy or release job, no third-party actions: `actions/checkout` is the only `uses:`. No `cargo deny`, `cargo llvm-cov` or aarch64 job.

[PRE_GATE]
test ! -e .github && grep -q '^jiff = ' Cargo.toml && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. Create `.github/workflows/ci.yml` with `name: CI`.
2. `on:` `push` with `branches: [main]`, and `pull_request`.
3. Top-level `permissions: { contents: read }` and `env: { CARGO_TERM_COLOR: always }`.
4. Top-level `concurrency: { group: ci-${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: true }`.
5. One job, `gates`, `runs-on: ubuntu-latest`, `timeout-minutes: 30`. Steps, in order:
   a. `uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`
   b. Install system packages: `sudo apt-get update && sudo apt-get install -y --no-install-recommends libdbus-1-dev pkg-config`. Why: `dbus-secret-service-keyring-store` links libdbus.
   c. Install the toolchain from `rust-version`: `rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy && rustup default 1.99.1`. Why: runners ship rustup, so no third-party action is needed.
   d. `cargo fmt --all -- --check`
   e. `cargo clippy --workspace --all-targets --locked -- -D warnings`
   f. `cargo test --workspace --locked`
   Give each of d–f its own `name:`. No `continue-on-error` anywhere.
6. Do not add a build cache. Why: it would need a third-party action; revisit if CI time hurts.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && python3 -c "import sys,yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && grep -q 'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1' .github/workflows/ci.yml && grep -q 'libdbus-1-dev' .github/workflows/ci.yml && grep -q 'cargo clippy --workspace --all-targets --locked -- -D warnings' .github/workflows/ci.yml && grep -q 'cargo test --workspace --locked' .github/workflows/ci.yml && test "$(grep -c 'uses:' .github/workflows/ci.yml)" -eq 1 && ! grep -q 'continue-on-error' .github/workflows/ci.yml

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 87: README CI Section

* **Recommended Command:** `/goal`

Scope note: the README must state only what has been verified. The section says what the workflow runs; the first green run on GitHub is the operator's check (`[USER] CI-1`).

```text
[STEP_ID]
STEP 87: the README has a CI section naming what the workflow runs and how to run the same gates locally

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md
READ_ONLY_REFS:
  - README.md:135-152 (`## Live-backend tests`; the new section goes directly above it)
  - .github/workflows/ci.yml (the three gate commands)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other README section changes.

[PRE_GATE]
test -f .github/workflows/ci.yml && grep -q '^## Live-backend tests' README.md && ! grep -q '^## CI' README.md

[MUTATION_SPEC]
1. README.md: add `## CI` directly above `## Live-backend tests`. Content: one sentence that `.github/workflows/ci.yml` runs on every push to `main` and every pull request; the three gates as a `bash` code block (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`); one sentence that CI runs on Ubuntu x86_64 with Rust 1.99.1 and installs `libdbus-1-dev` and `pkg-config`; one sentence that the live-backend tests are not run in CI because they need a feature flag and real backends. State nothing about `cargo deny`, coverage or aarch64 in CI.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && grep -q '^## CI' README.md && grep -q 'libdbus-1-dev' README.md

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### User Step: CI-1

After Step 86 is committed, the operator pushes a branch and opens a pull request. The check is that the `CI` workflow's `gates` job goes green. This is the only place the workflow itself runs; the local gates prove the commands, not the YAML on GitHub's runner.

---

## Phase L — configured tool arguments for hooks and gates (Steps 88–92, User Step NEB-1)

**Phase goal (frozen until the phase ends):** let hooks and gates send fixed, configured arguments next to the `orrery` object, so Orrery can drive Nebulous MCP tools that need them (`git_commit_documented` needs `summary`, and only commits with `dry_run: false`). Existing `project.toml` files load unchanged. Done when User Step NEB-1 shows the `on_step_pass` hook committing `.tracker/` with `dry_run: false`, and not committing with `dry_run` left out. Design: spec §9.17 (`ORRERY_TECHNICAL_SPECIFICATION.md:1451-1472`).

Written 2026-11-18 against `b1a58bc`. Facts checked while planning:
- `toml 1.1.5` round-trips a `serde_json::Map<String, serde_json::Value>` field (inline tables, arrays, nested tables, ints, floats, bools), and a config with no `args` key loads with an empty map. It fails to serialize a JSON `null` (`UnsupportedType(Some("unit"))`), which is why `validate_tool_args` rejects `null` before `save_project` writes anything (`save_project` runs `validate_staging` first, `config.rs:1727-1736`).
- Gate args go in a parallel `gate_args` map keyed by tool ref, so `gate_tools = ["server:tool", …]` keeps its shape.
- Nebulous `git_commit_documented` (`Nebulous/crates/backend_core/src/mcp/mod.rs:2235-2290`) deserializes its args without `deny_unknown_fields`, so the extra `orrery` key is ignored. `dry_run` defaults to `true`; `summary` must be non-empty.
- `neb mcp` picks its project from the registry's **active** project first and the working directory second (`resolve_active_storage`, same file, 171-189). User Step NEB-1 switches the active project first so the commit can't land in the wrong repo.
- Adding `HookConfig.args` breaks every `HookConfig { .. }` literal: 1 in `config.rs` tests, 17 in `engine.rs` tests, 2 in `commands.rs`, 1 in `widgets.rs`. Step 88 fixes them with one added line each.

### STEP 88: Hook and Gate Args in Config

* **Recommended Command:** `/goal`

Scope note: four source files, but `engine.rs`, `commands.rs` and `widgets.rs` only get one `args: Default::default(),` line per existing `HookConfig` literal so the workspace compiles. All logic is in `config.rs`.

```text
[STEP_ID]
STEP 88: HookConfig gains args and ProjectConfig gains gate_args (JSON objects, default empty); validation rejects the reserved 'orrery' key, null values, and gate_args keys not in gate_tools

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
  - crates/orrery-core/src/engine.rs (test literals only)
  - crates/orrery-cli/src/commands.rs (two literals only)
  - crates/orrery-tui/src/widgets.rs (one test literal only)
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1454-1458 (§9.17.1; binding)
  - crates/orrery-core/src/config.rs:1-10 (imports), 60-73 (`HookConfig`, `HookConfig::validate`), 123-138 (`HooksConfig::validate` field names), 224-243 (`ProjectConfig`), 245-273 (`default_template`), 374-378 (`gate_tools` loop in `validate_staging`), 1328-1346 (`validate_concrete_tool_ref`), 2995-3020 (`hooks_and_inference_validation_edges`)
  - crates/orrery-core/src/config.rs:1727-1736 (`save_project` runs `validate_staging` before `toml::to_string_pretty`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `gate_tools` stays `Vec<String>`. No change to `tool`, `on_failure`, their serde attributes, or `#[serde(deny_unknown_fields)]`. No engine logic change (Step 89). No CLI logic change (Step 90): `commands.rs:282` gets `args: Default::default(),` only.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1454p $S | grep -q '^#### 9.17.1 ' && ! grep -q 'pub gate_args' crates/orrery-core/src/config.rs && ! grep -q 'fn validate_tool_args' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs imports: add `use std::collections::BTreeMap;` in rustfmt order (first `std` line, before `use std::ffi::OsStr;`).
2. config.rs `HookConfig`: after `on_failure`, add
   `#[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]`
   `pub args: serde_json::Map<String, serde_json::Value>,`
   Keep the existing derives (`serde_json::Map` and `Value` implement `Eq`).
3. config.rs, directly after `validate_concrete_tool_ref`: add
   `pub fn validate_tool_args(args: &serde_json::Map<String, serde_json::Value>, field: &str) -> Result<()>`
   - `args.contains_key("orrery")` → `Err(Error::Validation { field: field.to_string(), message: "'orrery' is reserved for Orrery's call context".to_string() })`.
   - any value in `args` holds a JSON null at any depth → `Err(Error::Validation { field: field.to_string(), message: "null is not allowed (project.toml can't store it)".to_string() })`. Use a private recursive helper `fn json_contains_null(value: &serde_json::Value) -> bool` (`Null` → true; `Array` → any element; `Object` → any value; else false).
   - otherwise `Ok(())`.
   Why null: `toml` can't serialize unit, so `save_project` would fail after the edit with an unhelpful error.
4. config.rs `HookConfig::validate(&self, field)`: after the `validate_concrete_tool_ref` call add
   `let args_field = format!("{}.args", field.strip_suffix(".tool").unwrap_or(field));`
   `validate_tool_args(&self.args, &args_field)?;`
   So `"hooks.on_step_pass.tool"` reports args errors as `"hooks.on_step_pass.args"`.
5. config.rs `ProjectConfig`: directly after `gate_tools`, add
   `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`
   `pub gate_args: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,`
   `default_template`: add `gate_args: BTreeMap::new(),` after `gate_tools: Vec::new(),`.
6. config.rs `validate_staging`: directly after the `gate_tools` loop, add
   `for (key, args) in &self.gate_args {`
   `    let field = format!("gate_args.\"{key}\"");`
   `    if !self.gate_tools.iter().any(|t| t == key) { return Err(Error::Validation { field, message: format!("no gate_tools entry '{key}'") }); }`
   `    validate_tool_args(args, &field)?;`
   `}`
7. Every other `HookConfig { .. }` struct literal in the workspace gets `args: Default::default(),` as its last field and nothing else: `config.rs` test at ~3001, all 19 in `engine.rs` `mod tests` (`grep -n 'HookConfig {' crates/orrery-core/src/engine.rs`), `commands.rs` ~282 and ~1606, `widgets.rs` ~1256.
8. Tests in config.rs `mod tests` (build JSON maps with `serde_json::json!({..}).as_object().unwrap().clone()`):
   - `hook_and_gate_args_round_trip_toml`: `let mut cfg = ProjectConfig::default_template("args-proj");` set `cfg.hooks.on_step_pass = Some(HookConfig { tool: "neb:git_commit_documented".to_string(), on_failure: HookFailure::Halt, args: <{"summary": "orrery step pass", "dry_run": false}> })`, `cfg.gate_tools = vec!["neb:gate_run".to_string()]`, `cfg.gate_args.insert("neb:gate_run".to_string(), <{"profile": "ci", "paths": ["src"]}>)`. Assert `cfg.validate_staging()` is `Ok`. `let text = toml::to_string_pretty(&cfg).unwrap(); let back: ProjectConfig = toml::from_str(&text).unwrap();` assert `back == cfg`.
   - `hook_without_args_loads_empty_and_omits_key`: `let hook: HookConfig = toml::from_str("tool = \"git:commit\"\n").unwrap();` assert `hook.args.is_empty()`; assert `!toml::to_string(&hook).unwrap().contains("args")`. Then `let cfg = ProjectConfig::default_template("p");` assert `cfg.gate_args.is_empty()` and `!toml::to_string_pretty(&cfg).unwrap().contains("gate_args")`.
   - `tool_args_reject_reserved_orrery_key`: hook on `on_step_pass` with tool `"neb:git_commit_documented"` and args `{"orrery": {}}` → `validate_staging()` is `Err(Error::Validation { field, .. })` with `field == "hooks.on_step_pass.args"`. Separately, a fresh template with `gate_tools = ["neb:gate_run"]` and `gate_args["neb:gate_run"] = {"orrery": 1}` → `field == "gate_args.\"neb:gate_run\""`.
   - `tool_args_reject_null`: hook on `on_step_start` (tool `"git:status"`) with args `{"a": {"b": [1, null]}}` → `Err(Error::Validation { field, message })` with `field == "hooks.on_step_start.args"` and `message.contains("null")`.
   - `gate_args_key_must_be_gate_tool`: template with empty `gate_tools` and `gate_args["neb:gate_run"] = {}` → `Err(Error::Validation { field, .. })` with `field == "gate_args.\"neb:gate_run\""`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn validate_tool_args(args: &serde_json::Map<String, serde_json::Value>, field: &str) -> Result<()>' crates/orrery-core/src/config.rs && grep -q 'pub gate_args: BTreeMap<String, serde_json::Map<String, serde_json::Value>>' crates/orrery-core/src/config.rs && grep -q 'pub args: serde_json::Map<String, serde_json::Value>' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(hook_and_gate_args_round_trip_toml|hook_without_args_loads_empty_and_omits_key|tool_args_reject_reserved_orrery_key|tool_args_reject_null|gate_args_key_must_be_gate_tool): test$')" -eq 5

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 89: Merge Configured Args into Hook and Gate Calls

* **Recommended Command:** `/goal`

Scope note: one file. The `orrery` object's contents don't change; only where it sits.

```text
[STEP_ID]
STEP 89: a pure pub fn merge_tool_args builds call args from configured args plus the orrery object, and run_hook / run_gates use it

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1460-1462 (§9.17.2; binding)
  - crates/orrery-core/src/engine.rs:1292-1330 (`run_hook`, its `let args = serde_json::json!({ "orrery": .. })`)
  - crates/orrery-core/src/engine.rs:1392-1432 (`run_gates`, its `args` and the per-tool loop passing `args.clone()`)
  - crates/orrery-core/src/engine.rs:1945 (`pub fn referenced_mcp_servers`: the new fn goes directly above it)
  - crates/orrery-core/src/engine.rs:2166 (`test_context`), 2358 (`script_passing_step`), 3427-3470 (`gate_failure_blocks_pass`: gate test setup), 4052-4096 (`step_pass_hook_runs_after_pass`: hook test setup)
  - crates/orrery-core/src/mcp.rs:391 (`MockMcpTransport::calls() -> Vec<(String, serde_json::Value)>`)
  - crates/orrery-core/src/config.rs (`HookConfig.args`, `ProjectConfig.gate_args`, from Step 88)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The keys and values inside the `orrery` object stay exactly as they are. No change to hook role, gate role, `WriteScope`, success/failure handling or event emission.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1460p $S | grep -q '^#### 9.17.2 ' && grep -q 'pub gate_args: BTreeMap' crates/orrery-core/src/config.rs && ! grep -q 'pub fn merge_tool_args' crates/orrery-core/src/engine.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. engine.rs, directly above `pub fn referenced_mcp_servers`:
   `pub fn merge_tool_args(configured: &serde_json::Map<String, serde_json::Value>, orrery: serde_json::Value) -> serde_json::Value {`
   `    let mut merged = configured.clone();`
   `    merged.insert("orrery".to_string(), orrery);`
   `    serde_json::Value::Object(merged)`
   `}`
   Let rustfmt wrap the signature; no `#[rustfmt::skip]`.
   Why insert last: config validation already rejects an `orrery` key; inserting last means Orrery's object still wins if a config bypassed validation.
2. `run_hook`: replace `let args = serde_json::json!({ "orrery": { <fields> } });` with `let orrery = serde_json::json!({ <the same fields> });` then `let args = merge_tool_args(&hook_cfg.args, orrery);`.
3. `run_gates`: replace `let args = serde_json::json!({ "orrery": { <fields> } });` with `let orrery = serde_json::json!({ <the same fields, "event": "gate"> });` and `let no_args = serde_json::Map::new();`. Inside the `for (idx, tool_ref)` loop, before `execute_for_role`: `let args = merge_tool_args(self.ctx.project.gate_args.get(tool_ref).unwrap_or(&no_args), orrery.clone());` and pass `args` (not `args.clone()`).
4. Tests in engine.rs `mod tests` (maps via `serde_json::json!({..}).as_object().unwrap().clone()`):
   - `merge_tool_args_adds_orrery_and_keeps_configured`: configured `{"summary": "s", "dry_run": false}`, orrery `json!({"event": "step_pass"})` → result `== json!({"summary": "s", "dry_run": false, "orrery": {"event": "step_pass"}})`.
   - `merge_tool_args_empty_config_is_orrery_only`: `serde_json::Map::new()`, orrery `json!({"event": "gate"})` → `== json!({"orrery": {"event": "gate"}})`.
   - `merge_tool_args_orrery_wins_over_configured`: configured `{"orrery": "x", "a": 1}`, orrery `json!({"event": "halt"})` → `== json!({"a": 1, "orrery": {"event": "halt"}})`.
   - `step_pass_hook_sends_configured_args`: copy `step_pass_hook_runs_after_pass`'s setup with `HookConfig { tool: "git:tag".to_string(), on_failure: HookFailure::Halt, args: <{"summary": "orrery step pass", "dry_run": false}> }`. After the run (`RunOutcome::Completed { steps_passed: 1 }`): `calls.len() == 1`, `calls[0].1["summary"] == "orrery step pass"`, `calls[0].1["dry_run"] == false`, `calls[0].1["orrery"]["event"] == "step_pass"`.
   - `gate_sends_per_tool_args`: `test_context` with orch/imp/val as in `gate_failure_blocks_pass`; `ctx.project.gate_tools = vec!["cargo:test".to_string(), "cargo:clippy".to_string()]`; `ctx.project.gate_args.insert("cargo:test".to_string(), <{"profile": "ci"}>)`; transport `MockMcpTransport::new("cargo")` `.with_tool("test", json!({"content": [{"type": "text", "text": "ok"}]}))` `.with_tool("clippy", json!({"content": [{"type": "text", "text": "clean"}]}))`, registered on a `McpGatekeeper` as in that test; `script_passing_step(&mock, "orch", "imp", "val")`. Run → `RunOutcome::Completed { steps_passed: 1 }`. `calls.len() == 2`; `calls[0].0 == "test"`, `calls[0].1["profile"] == "ci"`, `calls[0].1["orrery"]["event"] == "gate"`; `calls[1].0 == "clippy"`, `calls[1].1.get("profile").is_none()`, `calls[1].1["orrery"]["event"] == "gate"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn merge_tool_args(' crates/orrery-core/src/engine.rs && grep -q 'merge_tool_args(&hook_cfg.args, orrery)' crates/orrery-core/src/engine.rs && grep -q 'gate_args.get(tool_ref)' crates/orrery-core/src/engine.rs && test "$(grep -c 'let orrery = serde_json::json!({' crates/orrery-core/src/engine.rs)" -eq 2 && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(merge_tool_args_adds_orrery_and_keeps_configured|merge_tool_args_empty_config_is_orrery_only|merge_tool_args_orrery_wins_over_configured|step_pass_hook_sends_configured_args|gate_sends_per_tool_args): test$')" -eq 5

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 90: CLI --args for Hooks and gate args

* **Recommended Command:** `/goal`

Scope note: two files. Reserved-key and null checks are not repeated in the CLI: `save_project` runs `validate_staging` (Step 88), so a bad value fails before anything is written.

```text
[STEP_ID]
STEP 90: orrery hook set takes --args <JSON>, orrery gate args <SERVER:TOOL> <JSON> sets per-gate args, gate set drops stale gate_args and gate clear clears them

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/args.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1464-1467 (§9.17.3; binding)
  - crates/orrery-cli/src/args.rs:384-391 (`GateCmd`), 428-442 (`HookCmd`), 448-638 (`every_spec_row_parses` rows array, `hook` rows at ~612-623)
  - crates/orrery-cli/src/commands.rs:251-303 (`dispatch_gate`, `dispatch_hook`), 305-313 (`edit_project`: load, mutate, `save_project`), 1510-1620 (`gate_set_and_clear`, `hook_set_and_clear`: test setup to copy)
  - crates/orrery-core/src/config.rs:1727-1736 (`save_project` → `validate_staging`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `GateCmd::Set { tools }` and `GateCmd::Clear` keep their argv shape. No change to other subcommands.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1464p $S | grep -q '^#### 9.17.3 ' && grep -q 'pub fn merge_tool_args' crates/orrery-core/src/engine.rs && ! grep -q 'fn parse_tool_args' crates/orrery-cli/src/commands.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. args.rs `HookCmd::Set`: after `on_failure`, add `#[arg(long)] args: Option<String>,`.
2. args.rs `GateCmd`: add variant `Args { tool: String, json: String },` between `Set` and `Clear`. Subcommand name is `args` (clap's default kebab-case of the variant).
3. args.rs `every_spec_row_parses` rows: add `&["orrery", "hook", "set", "pass", "neb:git_commit_documented", "--args", "{\"dry_run\":false}"]` and `&["orrery", "gate", "args", "fs:test", "{\"profile\":\"ci\"}"]`.
4. commands.rs: private `fn parse_tool_args(text: &str, field: &str) -> Result<serde_json::Map<String, serde_json::Value>>`: `serde_json::from_str::<serde_json::Value>(text)`; parse error → `Error::Validation { field: field.to_string(), message: format!("invalid JSON: {e}") }`; `Value::Object(map)` → `Ok(map)`; any other value → `Error::Validation { field: field.to_string(), message: "expected a JSON object".to_string() }`.
5. commands.rs `dispatch_hook` `HookCmd::Set`: destructure `args`; `let args = match args { Some(text) => parse_tool_args(&text, "--args")?, None => serde_json::Map::new() };` and use it in the `HookConfig` literal (replacing Step 88's `args: Default::default()` there).
6. commands.rs `dispatch_gate`:
   - `Set { tools }`: inside `edit_project`, `cfg.gate_args.retain(|key, _| tools.contains(key));` before `cfg.gate_tools = tools;`.
   - `Clear`: also `cfg.gate_args.clear();`.
   - new `Args { tool, json }`: `let args = parse_tool_args(&json, "gate args")?;` then `edit_project`: if `!cfg.gate_tools.contains(&tool)` → `Err(Error::Validation { field: "gate args".to_string(), message: format!("'{tool}' is not in gate_tools; run 'orrery gate set' first") })`; if `args.is_empty()` → `cfg.gate_args.remove(&tool)`, else `cfg.gate_args.insert(tool.clone(), args)`. Print `Cleared gate args for <tool>` or `Set gate args for <tool>` with `write_stdout_line`. Return `Ok(0)`.
7. Existing tests: `hook_set_and_clear`'s `HookCmd::Set { .. }` literal gets `args: None`. No other existing test changes.
8. New tests in commands.rs `mod tests` (each starts with `TempDir`, `test_ctx()` and `Command::Init` as `hook_set_and_clear` does):
   - `hook_set_with_args_saves_them`: `HookCmd::Set { event: HookEventArg::Pass, tool: "neb:git_commit_documented".to_string(), on_failure: HookFailureArg::Halt, args: Some(r#"{"summary":"orrery step pass","dry_run":false}"#.to_string()) }` → code 0; loaded `cfg.hooks.on_step_pass.unwrap().args == json!({"summary": "orrery step pass", "dry_run": false}).as_object().unwrap().clone()`.
   - `hook_set_rejects_bad_args`: `args: Some(r#"{"orrery":{}}"#)` → `dispatch` returns `Err(Error::Validation { field, .. })` with `field == "hooks.on_step_pass.args"`, and the loaded config's `on_step_pass` is `None`. Then `args: Some("[1]")` → `Err(Error::Validation { field, .. })` with `field == "--args"`.
   - `gate_args_set_replace_and_clear`: `gate set fs:test fs:lint`; `GateCmd::Args { tool: "fs:test", json: r#"{"profile":"ci"}"# }` → code 0, `cfg.gate_args["fs:test"]["profile"] == "ci"`; `Args { tool: "fs:missing", json: "{}" }` → `Err(Error::Validation { field, .. })` with `field == "gate args"`; `Args { tool: "fs:test", json: "{}" }` → `gate_args` empty; set `{"profile":"ci"}` again, then `gate set fs:lint` → `gate_args` empty; set `fs:lint` args `{"x":1}`, then `GateCmd::Clear` → `gate_tools` and `gate_args` both empty.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'fn parse_tool_args(' crates/orrery-cli/src/commands.rs && grep -q 'gate_args.retain' crates/orrery-cli/src/commands.rs && grep -q 'Args { tool: String, json: String }' crates/orrery-cli/src/args.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(hook_set_with_args_saves_them|hook_set_rejects_bad_args|gate_args_set_replace_and_clear): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 91: Live MCP Call with Merged Args

* **Recommended Command:** `/goal`

Scope note: one test file. Written and compiled, never run by the agent (Rules: live-backend tests). The operator runs it in User Step NEB-1.

```text
[STEP_ID]
STEP 91: tests/live_mcp.rs gains live_stdio_call_with_merged_args, which calls ORRERY_LIVE_MCP_TOOL with merge_tool_args(ORRERY_LIVE_MCP_ARGS, orrery object) and asserts the result is not an error

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/tests/live_mcp.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1469-1470 (§9.17.4; binding)
  - crates/orrery-core/tests/live_mcp.rs:1-41 (`live_stdio_list_and_call`: env parsing and connect)
  - crates/orrery-core/src/mcp.rs:139-146 (`tool_result_is_error`, `tool_result_text`)
  - crates/orrery-core/src/engine.rs (`merge_tool_args`, from Step 89)
  - crates/orrery-core/Cargo.toml:40-42 (`live_mcp` needs `live-backends`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `live_stdio_list_and_call` keeps its behavior. Don't run the live test.

[PRE_GATE]
grep -q 'pub fn merge_tool_args' crates/orrery-core/src/engine.rs && ! grep -q 'live_stdio_call_with_merged_args' crates/orrery-core/tests/live_mcp.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. live_mcp.rs: move the `ORRERY_LIVE_MCP_CMD` parsing and `McpServerConfig` building into `fn live_server() -> McpServerConfig` (same `expect` messages, `server_id: "live"`), and use it from `live_stdio_list_and_call`. Why: both tests connect the same way.
2. New `#[tokio::test] async fn live_stdio_call_with_merged_args()`:
   - `let tool = std::env::var("ORRERY_LIVE_MCP_TOOL").expect("ORRERY_LIVE_MCP_TOOL must name a tool on the ORRERY_LIVE_MCP_CMD server");`
   - `let raw = std::env::var("ORRERY_LIVE_MCP_ARGS").unwrap_or_else(|_| "{}".to_string());` parse with `serde_json::from_str::<serde_json::Value>`, `.expect("ORRERY_LIVE_MCP_ARGS must be JSON")`, then `.as_object().cloned().expect("ORRERY_LIVE_MCP_ARGS must be a JSON object")`.
   - `let orrery = json!({ "project_name": "live-test", "run_id": "00000000-0000-0000-0000-000000000000", "step_number": 1, "step_title": "live merged args", "plan_step_sha256": "0".repeat(64), "event": "step_pass" });`
   - `let args = orrery_core::engine::merge_tool_args(&configured, orrery);`
   - connect with `RmcpTransport::connect(&live_server(), &InMemorySecretStore::new())`, then `let val = transport.call_tool(&tool, args).await.unwrap_or_else(|err| panic!("transport error calling '{tool}': {err}"));`
   - `assert!(!tool_result_is_error(&val), "'{tool}' returned isError: {}", tool_result_text(&val));`
3. Update the `use orrery_core::mcp::{..}` line to also import `tool_result_text`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy -p orrery-core --all-targets --locked --features live-backends -- -D warnings && cargo test --workspace --locked && grep -q 'async fn live_stdio_call_with_merged_args' crates/orrery-core/tests/live_mcp.rs && grep -q 'ORRERY_LIVE_MCP_TOOL' crates/orrery-core/tests/live_mcp.rs && grep -q 'merge_tool_args(&configured, orrery)' crates/orrery-core/tests/live_mcp.rs && grep -q '!tool_result_is_error(&val)' crates/orrery-core/tests/live_mcp.rs

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 92: README Hook and Gate Args

* **Recommended Command:** `/goal`

Scope note: docs only. The README states only what Steps 88–91 built; it makes no claim about Nebulous behavior beyond the tool's own argument names.

```text
[STEP_ID]
STEP 92: the README documents hook and gate args, the reserved orrery key, the new CLI forms, and the ORRERY_LIVE_MCP_TOOL / ORRERY_LIVE_MCP_ARGS env vars

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1451-1472 (§9.17; binding)
  - README.md:117-124 (project layout bullets; the `project.toml` bullet), 147-172 (`## Live-backend tests`, env var list)
  - crates/orrery-cli/src/args.rs (`HookCmd::Set`, `GateCmd::Args`, from Step 90)
  - crates/orrery-core/tests/live_mcp.rs (`live_stdio_call_with_merged_args`, from Step 91)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other README section changes.

[PRE_GATE]
grep -q 'live_stdio_call_with_merged_args' crates/orrery-core/tests/live_mcp.rs && grep -q '^## Live-backend tests' README.md && ! grep -q '^### Hook and gate arguments' README.md

[MUTATION_SPEC]
1. README.md: in the `project.toml` bullet, change ``gate_tools`` to ``gate_tools` (with optional `[gate_args]`)` and ``[hooks]`` to ``[hooks]` (each with optional `args`)`.
2. README.md: add `### Hook and gate arguments` directly after the project layout bullet list (before `## Security`). Content:
   - One sentence: each hook and gate is called with its configured `args` plus an `orrery` object (`project_name`, `run_id`, `step_number`, `step_title`, `plan_step_sha256`, `event`); `orrery` is reserved, and `args` can't contain `null`.
   - A `toml` block:
     `[hooks.on_step_pass]` / `tool = "neb:git_commit_documented"` / `on_failure = "Halt"` / `args = { summary = "orrery step pass", dry_run = false }`, then `[gate_args."neb:gate_run"]` / `profile = "ci"`.
   - A `bash` block: `orrery hook set pass neb:git_commit_documented --args '{"summary":"orrery step pass","dry_run":false}'` and `orrery gate args neb:gate_run '{"profile":"ci"}'`.
   - One sentence: `orrery gate args <tool> '{}'` removes a gate's args; `gate set` drops args for tools no longer listed; `gate clear` clears them.
   Use `neb:gate_run` only as an example name; state it's an example.
3. README.md env var list: directly after the `ORRERY_LIVE_MCP_CMD` bullet, add one bullet naming `ORRERY_LIVE_MCP_TOOL` (tool name on that server) and `ORRERY_LIVE_MCP_ARGS` (JSON object of args, default `{}`), used by `live_stdio_call_with_merged_args` in `live_mcp`. Say the test adds the `orrery` object itself and asserts the result is not an error. Match the existing bullets' format (backticked names, then a colon).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && grep -q '^### Hook and gate arguments' README.md && grep -q 'ORRERY_LIVE_MCP_TOOL' README.md && grep -q 'orrery gate args' README.md && grep -q -- '--args' README.md

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### User Step: NEB-1

After Step 92 is committed. This is the phase's acceptance check. `neb` must be built from Nebulous `bc00dd3` (STEP 40A) or later: older builds have no `dry_run` default and commit on every `git_commit_documented` call, so step 2 would commit. Rebuild with `/srv/devshare/Nebulous/scripts/install.sh --user` if `~/.local/bin/neb` predates 2026-11-18 11:48. Use a scratch project, not a real one: the hook makes a real git commit.

1. Make a scratch git repo with a Nebulous tracker and make it neb's **active** project (`neb mcp` uses the registry's active project before its working directory):

   ```bash
   mkdir -p ~/scratch/orrery-neb && cd ~/scratch/orrery-neb && git init && neb init
   ```

   ```bash
   neb project switch ~/scratch/orrery-neb
   ```

2. Live test, safe form (`dry_run` left out, so neb only reports). From the Orrery repo:

   ```bash
   ORRERY_LIVE_MCP_CMD="neb mcp" ORRERY_LIVE_MCP_TOOL=git_commit_documented ORRERY_LIVE_MCP_ARGS='{"summary":"orrery live test"}' cargo test -p orrery-core --locked --features orrery-core/live-backends --test live_mcp live_stdio_call_with_merged_args
   ```

   Expect: passes; `git -C ~/scratch/orrery-neb log --oneline` shows no new commit.

3. Hook, commit form. Needs your llama.cpp server running; the hook fires only when a step passes, so this is a real (one-step) agent run.

   a. Install Orrery from this repo. It lands in `~/.cargo/bin`, which is on your PATH:

   ```bash
   cargo install --path /srv/devshare/Orrery/crates/orrery-cli --locked
   ```

   ```bash
   command -v orrery
   ```

   b. Set up Orrery in the scratch repo. `init` writes the default orchestrator, coder and validator roles. Put your server's URL and model ID in the two `<...>` spots:

   ```bash
   cd ~/scratch/orrery-neb && orrery init --name orrery-neb
   ```

   ```bash
   orrery inference add --id local --endpoint <your llama.cpp URL>
   ```

   ```bash
   orrery agent add-local-model --id local-agent --name "Local Agent" --inference local --model <model id>
   ```

   ```bash
   orrery workflow assign 0 local-agent && orrery workflow assign 1 local-agent && orrery workflow assign 2 local-agent
   ```

   c. A one-step plan that changes no files, so a local model can pass it without write tools:

   ```bash
   printf '## Step 1: Acknowledge\n\nNo files change. The implementer reports Complete; the validator confirms.\n' > PLAN.md
   ```

   ```bash
   orrery ingest PLAN.md --marker "## Step "
   ```

   d. Register neb and the hook:

   ```bash
   orrery mcp add-stdio --id neb --command neb --arg mcp
   ```

   ```bash
   orrery hook set pass neb:git_commit_documented --args '{"summary":"orrery step pass","dry_run":false}'
   ```

   e. Make a `.tracker/` change, run one step, and look:

   ```bash
   neb task "orrery hook check 1"
   ```

   ```bash
   orrery run
   ```

   ```bash
   git log -1 --stat
   ```

   Expect: `orrery run` ends with the step `Passed`, and the last commit is from `orrery step pass` and touches only `.tracker/`. If the hook fails, `on_failure = "Halt"` leaves the step awaiting the operator, and `orrery logs --tail 50` shows the tool's error.

4. Hook, no-commit form: `orrery hook set pass neb:git_commit_documented --args '{"summary":"orrery step pass"}'`, then `orrery reset --step 1`, `neb task "orrery hook check 2"` and `orrery run` again. Expect: no new commit; the hook result in `orrery status` / the live log shows `"dry_run": true`.

Report back: pass/fail for 2–4, plus `git log --oneline -3` from the scratch repo.

### Parked (Phase L)

All three picked for Phase M (2026-11-18): items 1 → Step 98, 2 → Step 97, 3 → Step 97.

- `crates/orrery-core/src/config.rs:1376` — `#[rustfmt::skip]` on `validate_tool_args` exists only so Step 88's POST_GATE grep could match the signature on one line (planning error: the signature is 129 chars). — Remove the attribute and run `cargo fmt --all`; Steps 89–90 markers no longer grep full signatures.
- `crates/orrery-cli/src/args.rs:390` — comment `// Args { tool: String, json: String }` exists only so Step 90's POST_GATE grep matched (rustfmt splits the variant over lines; planning error). — Delete the comment.
- `crates/orrery-cli/src/commands.rs` (`dispatch_run`) — `orrery run` exits 0 with no output when the current step is already `AwaitingOperator` (seen in NEB-1). — Print `awaiting operator: <reason>; run orrery approve or orrery reject` before returning.

---

## Phase M — role protocol, skills and parked fixes (Steps 93–99, User Step LIVE-3)

**Phase goal (frozen until the phase ends):** every agent's system prompt carries Orrery's report protocol, generated from the code that enforces it; `init` writes role rules that describe scope and behavior; roles can pull in named markdown skills from `.orrery/skills/` (or the global catalog); and the three Phase L parked items are fixed. Done when User Step LIVE-3 passes a one-step plan on the local Qwen3.8-27B from a fresh `orrery init` with no hand-edited rules. Design: spec §9.18 (`ORRERY_TECHNICAL_SPECIFICATION.md:1474-1517`).

Written 2026-11-18 against `71e9b6a`. Facts checked while planning:
- An agent's system prompt today is `# Role: <name>`, the rules file and one reporting line (`llm.rs:52-72`). Nothing tells it its allowed statuses, the `Dispatch` fields or the pass rule; those live only in `checkpoint.rs:50-152`. In NEB-1 the 27B failed until hand-written rules spelled them out, then passed first try twice.
- Skills are `.md` fragments (operator choice, 2026-11-18), not MCP prompts.
- Adding `RoleConfig.skills` breaks 48 `RoleConfig { .. }` literals in 11 files. Step 95 is that mechanical change alone.
- `orrery run` returns exit 3 on `RunOutcome::AwaitingOperator` but prints nothing (`commands.rs:841-845`); hit three times in NEB-1.

### STEP 93: Protocol Contract Text from the Enforcing Code

* **Recommended Command:** `/goal`

Scope note: two files, pure functions only. Nothing calls the new prompt builder until Step 94.

```text
[STEP_ID]
STEP 93: checkpoint.rs exposes allowed_statuses / requires_feedback (validate_for_role uses them), and llm.rs gains protocol_contract and build_agent_system_prompt

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/checkpoint.rs
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1477-1491 (§9.18.1; binding, including the exact contract text)
  - crates/orrery-core/src/checkpoint.rs:12 (`MAX_SUMMARY_BYTES`), 14-23 (`ReportStatus`), 50-152 (`validate_for_role`)
  - crates/orrery-core/src/llm.rs:12-16 (imports), 52-72 (`build_system_prompt`), 1277 (`sample_local_agent`), 1485-1525 (system prompt tests)
  - crates/orrery-core/src/config.rs (`RoleKind`, `ValidationMode`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `validate_for_role`'s accepted reports and every error message stay exactly as they are. `build_system_prompt` keeps its signature and output.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1477p $S | grep -q '^#### 9.18.1 ' && ! grep -q 'fn allowed_statuses' crates/orrery-core/src/checkpoint.rs && ! grep -q 'fn protocol_contract' crates/orrery-core/src/llm.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. checkpoint.rs, above `impl AgentReport`: `pub fn allowed_statuses(kind: RoleKind) -> &'static [ReportStatus]` and `pub fn requires_feedback(status: ReportStatus) -> bool`, per §9.18.1.
2. checkpoint.rs `validate_for_role`: the status check becomes `let allowed = allowed_statuses(role.kind).contains(&self.status);`; the feedback check becomes `if requires_feedback(self.status) && self.feedback...`. Same messages.
3. llm.rs: `pub fn protocol_contract(kind: RoleKind, stage_index: usize, stage_count: usize, mode: ValidationMode) -> String`, producing §9.18.1's text line for line (lines joined with `\n`, one blank line after the heading). Build the status list from `allowed_statuses(kind)` and the feedback lines from `requires_feedback`, never from literals, so the text can't drift from the checks. `GateAndOperator` gets rule list 9; every other mode gets list 10.
4. llm.rs: `pub fn build_agent_system_prompt(role: &RoleConfig, rules_markdown: &str, backend: &AgentBackend, stage_index: usize, stage_count: usize, mode: ValidationMode) -> String` per §9.18.1.
5. llm.rs imports: add `allowed_statuses`, `requires_feedback`, `MAX_SUMMARY_BYTES`, `ReportStatus` from `crate::checkpoint` and `ValidationMode` from `crate::config`, as needed.
6. Tests, checkpoint.rs `mod tests`:
   - `allowed_statuses_match_role_kinds`: the three slices equal §9.18.1's lists; `requires_feedback` is true for exactly `Denied`, `Blocked`, `Escalate` (loop over all 7 variants).
7. Tests, llm.rs `mod tests`:
   - `contract_lists_orchestrator_statuses_and_dispatch_rules`: `protocol_contract(RoleKind::Orchestrator, 0, 3, ValidationMode::Agents)` starts with `"# Orrery protocol\n\n"`, contains ``"Allowed statuses: `Dispatch`, `StepPassed`, `Escalate`."``, ``"`Escalate` requires `feedback` saying why."``, `"Stages run from 0 to 2."`, `"It never means the work has happened."`, and ``"no Implementer stage ran after it → `StepPassed`."``.
   - `contract_gate_mode_passes_without_validator`: same call with `ValidationMode::GateAndOperator` contains `"Orrery then runs the gate tools and asks the operator."` and does not contain `` "`Denied`" ``.
   - `contract_for_implementer_and_validator`: Implementer at stage 1 of 3 contains ``"Allowed statuses: `Complete`, `Blocked`."``, ``"`Blocked` requires `feedback`"``, ``"Never include `target_stage` or `allocations`."``, and does not contain `"Decide with the first rule"`. Validator at stage 2 contains ``"Allowed statuses: `Confirmed`, `Denied`."``.
   - `agent_system_prompt_puts_contract_before_rules`: `build_agent_system_prompt(&role, "Write clean Rust.", &sample_local_agent("coder-1").backend, 1, 3, ValidationMode::Agents)` for an Implementer role: the index of `"# Orrery protocol"` is less than the index of `"Write clean Rust."`, and it still contains `"submit_report"`. With rules `"   "`, the output contains no `"\n\n\n"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn allowed_statuses(' crates/orrery-core/src/checkpoint.rs && grep -q 'pub fn requires_feedback(' crates/orrery-core/src/checkpoint.rs && grep -q 'pub fn protocol_contract(' crates/orrery-core/src/llm.rs && grep -q 'pub fn build_agent_system_prompt(' crates/orrery-core/src/llm.rs && grep -q 'allowed_statuses(role.kind)' crates/orrery-core/src/checkpoint.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(checkpoint::tests::allowed_statuses_match_role_kinds|llm::tests::(contract_lists_orchestrator_statuses_and_dispatch_rules|contract_gate_mode_passes_without_validator|contract_for_implementer_and_validator|agent_system_prompt_puts_contract_before_rules)): test$')" -eq 5

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 94: Engine Sends the Protocol Contract

* **Recommended Command:** `/goal`

Scope note: one file, two call sites.

```text
[STEP_ID]
STEP 94: run_orchestrator_turn and run_stage_turn build system prompts with build_agent_system_prompt, so every agent gets the protocol contract

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1493-1494 (§9.18.2; binding)
  - crates/orrery-core/src/engine.rs:1659-1690 (`run_orchestrator_turn`, `build_system_prompt` call), 1712-1785 (`run_stage_turn`, `build_system_prompt` call in the allocation loop)
  - crates/orrery-core/src/config.rs (`ProjectConfig::effective_validation_mode(&self, roles: &[RoleConfig]) -> ValidationMode`)
  - crates/orrery-core/src/llm.rs (`build_agent_system_prompt`, from Step 93; `MockAgentDriver::invocations()` at ~229)
  - crates/orrery-core/src/engine.rs:2166 (`test_context`), 2283 (`attach_mock_driver`), 2358 (`script_passing_step`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. User prompts, tools, write scope and report handling don't change.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1493p $S | grep -q '^#### 9.18.2 ' && grep -q 'pub fn build_agent_system_prompt(' crates/orrery-core/src/llm.rs && ! grep -q 'build_agent_system_prompt' crates/orrery-core/src/engine.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. `run_orchestrator_turn`: replace `build_system_prompt(&role, rules, &agent.backend)` with `build_agent_system_prompt(&role, rules, &agent.backend, 0, self.ctx.project.stages.len(), self.ctx.project.effective_validation_mode(&self.ctx.roles))`.
2. `run_stage_turn`: the same replacement in the allocation loop, with `stage_index` in place of `0`. Compute the mode once before the loop.
3. Imports: add `build_agent_system_prompt` to the `crate::llm` import. Remove `build_system_prompt` from it only if nothing else in the non-test code uses it.
4. Tests in engine.rs `mod tests`:
   - `agents_receive_protocol_contract`: `test_context` with orch/imp/val, `attach_mock_driver`, `script_passing_step(&mock, "orch", "imp", "val")`, run → `RunOutcome::Completed { steps_passed: 1 }`. From `mock.invocations()`: the first one with `stage_index == 0` has a `system_prompt` containing ``"Allowed statuses: `Dispatch`, `StepPassed`, `Escalate`."``; the first with `stage_index == 1` contains ``"Allowed statuses: `Complete`, `Blocked`."``; the first with `stage_index == 2` contains ``"Allowed statuses: `Confirmed`, `Denied`."``. Every invocation's `system_prompt` contains `"# Orrery protocol"`.
   - `orchestrator_contract_follows_validation_mode`: same setup but `ctx.project.validation_mode = ValidationMode::GateAndOperator`, with stages orch/imp only (no validator) and a script that passes in that mode (Dispatch to imp, imp Complete, orch StepPassed; the run then awaits the operator). The first orchestrator invocation's `system_prompt` contains `"Orrery then runs the gate tools and asks the operator."`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && test "$(grep -c 'build_agent_system_prompt(' crates/orrery-core/src/engine.rs)" -ge 2 && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^engine::tests::(agents_receive_protocol_contract|orchestrator_contract_follows_validation_mode): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 95: RoleConfig Skills Field

* **Recommended Command:** `/goal`

Scope note: eleven files, but only `config.rs` gets logic. Everywhere else is one added `skills: Vec::new(),` line per existing `RoleConfig { .. }` literal so the workspace compiles. Splitting it would leave a tree that doesn't build.

```text
[STEP_ID]
STEP 95: RoleConfig gains skills (Vec<String>, default empty, omitted when empty); validate checks names and rejects duplicates

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
  - crates/orrery-core/src/checkpoint.rs (literals only)
  - crates/orrery-core/src/engine.rs (literals only)
  - crates/orrery-core/src/llm.rs (literals only)
  - crates/orrery-core/src/mcp.rs (literals only)
  - crates/orrery-core/tests/live_docker.rs (literals only)
  - crates/orrery-core/tests/live_llm.rs (literals only)
  - crates/orrery-cli/src/commands.rs (literals only)
  - crates/orrery-tui/src/app.rs (literals only)
  - crates/orrery-tui/src/bridge.rs (literals only)
  - crates/orrery-tui/src/widgets.rs (literals only)
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1496-1498 (§9.18.3; binding)
  - crates/orrery-core/src/config.rs:648-657 (`RoleConfig`), 702-721 (`RoleConfig::validate`), 1598-1616 (`validate_config_file_id`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other field or serde attribute changes. In the "(literals only)" files the only change is `skills: Vec::new(),` as the last field of each `RoleConfig { .. }` literal (`grep -rn 'RoleConfig {' crates`). No skill loading (Step 96), no CLI flag (Step 97).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1496p $S | grep -q '^#### 9.18.3 ' && grep -q 'build_agent_system_prompt(' crates/orrery-core/src/engine.rs && ! grep -q 'pub skills: Vec<String>' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs `RoleConfig`: after `allowed_mcp_tools`, add `#[serde(default, skip_serializing_if = "Vec::is_empty")]` / `pub skills: Vec<String>,`.
2. config.rs `RoleConfig::validate`: after the `allowed_mcp_tools` loop, for `(idx, name)` in `self.skills`: `validate_config_file_id(name, &format!("skills[{idx}]"))?;` and if `self.skills[..idx].contains(name)` → `Err(Error::Validation { field: format!("skills[{idx}]"), message: format!("duplicate skill '{name}'") })`.
3. Every `RoleConfig { .. }` literal in the 11 files: add `skills: Vec::new(),` as the last field. This includes the 3 in `default_roles_with_rules`.
4. Tests in config.rs `mod tests`:
   - `role_skills_default_empty_and_omitted`: `toml::from_str::<RoleConfig>("role_id = \"coder\"\ndisplay_name = \"Coder\"\nkind = \"Implementer\"\nrules_file = \"coder.md\"\n")` → `skills.is_empty()`; `toml::to_string(&role)` does not contain `"skills"`.
   - `role_skills_round_trip_and_validate`: a role with `skills: vec!["rust-style".to_string(), "neb-tracker".to_string()]` validates `Ok`, and survives `toml::to_string` → `toml::from_str` unchanged.
   - `role_skills_reject_bad_names`: `skills: vec!["../x".to_string()]` → `Err(Error::PathTraversal { .. })`; `vec!["a".to_string(), "a".to_string()]` → `Err(Error::Validation { field, message })` with `field == "skills[1]"` and `message == "duplicate skill 'a'"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy -p orrery-core --all-targets --locked --features live-backends -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub skills: Vec<String>' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(role_skills_default_empty_and_omitted|role_skills_round_trip_and_validate|role_skills_reject_bad_names): test$')" -eq 3

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 96: Skill Files Load into Role Rules

* **Recommended Command:** `/goal`

Scope note: two files. `config.rs` finds and reads a skill; `engine.rs` appends skills to each role's rules when the engine is built.

```text
[STEP_ID]
STEP 96: ConfigStore::load_skill reads .orrery/skills/<name>.md (project over global), and build_engine appends each role's skills to its rules via compose_role_rules

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
  - crates/orrery-core/src/engine.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1500-1504 (§9.18.4; binding)
  - crates/orrery-core/src/config.rs:1598-1616 (`validate_config_file_id`), 1676 (`ConfigStore::new(project_root, xdg_override)`), 1713-1725 (`global_dir`), 1801-1842 (`load_role_rules`: project-then-global pattern to copy), 1845-1860 (`save_role`)
  - crates/orrery-core/src/engine.rs:1982-1990 (`build_engine` role_rules loop), 5391 (`init_project_with_agents`), 5463-5481 (`build_engine_from_initialized_project`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `load_role_rules` doesn't change. A role with no skills gets exactly its rules text, as today.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1500p $S | grep -q '^#### 9.18.4 ' && grep -q 'pub skills: Vec<String>' crates/orrery-core/src/config.rs && ! grep -q 'fn load_skill' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs `impl ConfigStore`, after `load_role_rules`: `pub fn load_skill(&self, name: &str) -> Result<String>` per §9.18.4. Project path `self.orrery_dir().join("skills").join(format!("{name}.md"))`; global path `global_dir.join("skills").join(format!("{name}.md"))`. Read with `fs::read_to_string`, mapping errors other than not-found to `Error::Io { path, source }`.
2. engine.rs, next to `merge_tool_args`: `pub fn compose_role_rules(rules_markdown: &str, skills: &[(String, String)]) -> String` per §9.18.4.
3. engine.rs `build_engine`: in the role loop, `let skills = role.skills.iter().map(|name| Ok((name.clone(), store.load_skill(name)?))).collect::<Result<Vec<_>>>()?;` then `role_rules.insert(role.role_id.clone(), compose_role_rules(&rules, &skills));`.
4. Tests in config.rs `mod tests`:
   - `load_skill_project_overrides_global`: `TempDir` project and `TempDir` global, with `ConfigStore::new(project, Some(global_dir_path))` set up so `global_dir()` returns `<global>/orrery` (create that dir). Write `<global>/orrery/skills/style.md` = `"global style"` and `<global>/orrery/skills/only-global.md` = `"g"`; write `<project>/.orrery/skills/style.md` = `"project style"`. `load_skill("style") == "project style"`; `load_skill("only-global") == "g"`.
   - `load_skill_missing_or_bad_name`: `load_skill("nope")` → `Err(Error::Validation { field, message })` with `field == "skills"` and `message.contains("skill 'nope' not found")`; `load_skill("../x")` → `Err(Error::PathTraversal { .. })`.
5. Tests in engine.rs `mod tests`:
   - `compose_role_rules_appends_skills`: `compose_role_rules("Rules.", &[("a".into(), "  Body A\n".into()), ("b".into(), "Body B".into())]) == "Rules.\n\n# Skill: a\n\nBody A\n\n# Skill: b\n\nBody B"`; with no skills it returns `"Rules."` unchanged.
   - `build_engine_includes_role_skills`: `init_project_with_agents(dir.path(), true)`; write `.orrery/skills/house-style.md` = `"Keep functions short."`; load the `coder` role with `store.load_roles()`, set `skills = vec!["house-style".into()]`, and save it with `store.save_role(&role, &store.load_role_rules("coder").unwrap())`. `build_engine` → `engine.ctx.role_rules["coder"]` contains `"# Skill: house-style\n\nKeep functions short."`. Then set the skill to `"missing"`, save, and `build_engine` → `Err(Error::Validation { .. })`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn load_skill(' crates/orrery-core/src/config.rs && grep -q 'pub fn compose_role_rules(' crates/orrery-core/src/engine.rs && grep -q 'store.load_skill(' crates/orrery-core/src/engine.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^(config::tests::(load_skill_project_overrides_global|load_skill_missing_or_bad_name)|engine::tests::(compose_role_rules_appends_skills|build_engine_includes_role_skills)): test$')" -eq 4

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 97: CLI role add --skill and the Awaiting-Operator Message

* **Recommended Command:** `/goal`

Scope note: two files. Also removes the Phase L parked comment at `args.rs:390`.

```text
[STEP_ID]
STEP 97: orrery role add takes --skill <NAME> (repeatable); run/resume print the awaiting-operator reason and next commands; the stray Args comment is removed

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/args.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1506-1509 (§9.18.5; binding)
  - crates/orrery-cli/src/args.rs:323-335 (`RoleCmd::Add`), 390 (`// Args { tool: String, json: String }`), 455-650 (`every_spec_row_parses`; `role add` row at ~568-584)
  - crates/orrery-cli/src/commands.rs:553-575 (`dispatch_role` `RoleCmd::Add`), 779-845 (`dispatch_run`: `engine.redact`, the final `match outcome`), 1069-1080 (`test_ctx`), 1049 (`SharedVec::as_string`), 2509 (`role_add_reads_rules_file`), 2959 (`escalate_report`), 2978 (`setup_runnable_project`, returns `(ctx, out, err)`), 3161-3200 (`escalation_exits_3_then_approve_resume_completes`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Exit codes don't change. Only the `AwaitingOperator` arm prints; other outcomes stay silent.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1506p $S | grep -q '^#### 9.18.5 ' && grep -q 'pub fn load_skill(' crates/orrery-core/src/config.rs && grep -q '// Args { tool: String, json: String }' crates/orrery-cli/src/args.rs && ! grep -q 'Awaiting operator for step' crates/orrery-cli/src/commands.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. args.rs `RoleCmd::Add`: after `allow_tools`, add `#[arg(long = "skill")] skills: Vec<String>,`.
2. args.rs: delete the line `// Args { tool: String, json: String }`.
3. args.rs `every_spec_row_parses`: append `"--skill", "house-style"` to the existing `role add` row.
4. commands.rs `dispatch_role` `RoleCmd::Add`: destructure `skills` and set `skills` in the `RoleConfig` literal (replacing Step 95's `skills: Vec::new()` there). `save_role` validates it.
5. commands.rs `dispatch_run`: the `RunOutcome::AwaitingOperator { step_number, reason }` arm writes, with `write_stdout_line`, `format!("Awaiting operator for step {step_number}: {}", engine.redact(&reason))` and then `"Run 'orrery approve' or 'orrery reject --feedback <TEXT>', then 'orrery resume'."`, then returns `Ok(3)`. If `resume` uses a separate function with its own outcome match, apply the same there.
6. Existing test literals of `RoleCmd::Add { .. }` get `skills: Vec::new()`.
7. Tests in commands.rs `mod tests`:
   - `role_add_with_skills_saves_them`: copy `role_add_reads_rules_file`'s setup, add `skills: vec!["house-style".to_string()]` → the loaded role's `skills == vec!["house-style"]`. Then a second add with `skills: vec!["../x".to_string()]` → `Err(Error::PathTraversal { .. })`.
   - `awaiting_operator_run_prints_reason_and_next_steps`: as `escalation_exits_3_then_approve_resume_completes`, keep the `out` handle from `setup_runnable_project`. First `Command::Run` → code 3, and `out.as_string()` contains `"Awaiting operator for step 1: "` and `"Run 'orrery approve' or 'orrery reject --feedback <TEXT>', then 'orrery resume'."`. `out.clear()`, run again (still awaiting) → code 3, and `out.as_string()` again contains `"Awaiting operator for step 1: "`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q '// Args { tool: String, json: String }' crates/orrery-cli/src/args.rs && grep -q 'Awaiting operator for step' crates/orrery-cli/src/commands.rs && grep -q 'long = "skill"' crates/orrery-cli/src/args.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(role_add_with_skills_saves_them|awaiting_operator_run_prints_reason_and_next_steps): test$')" -eq 2

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 98: Default Role Rules Describe Behavior

* **Recommended Command:** `/goal`

Scope note: one file. The text comes from the NEB-1 scratch rules that worked, minus everything the generated contract now covers. Also removes the Phase L parked `#[rustfmt::skip]`.

```text
[STEP_ID]
STEP 98: orrery init writes orchestrator, coder and validator rules that describe scope and behavior (no protocol), and validate_tool_args loses its #[rustfmt::skip]

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1511-1513 (§9.18.6; binding)
  - crates/orrery-core/src/config.rs:1376 (`#[rustfmt::skip]` above `validate_tool_args`), 1736 (`init_project`), 2375-2410 (`default_roles_with_rules`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. Role ids, names, kinds, rules file names and empty tool lists stay as they are. `validate_tool_args` changes only by losing the attribute and being reformatted.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && sed -n 1511p $S | grep -q '^#### 9.18.6 ' && grep -q 'Awaiting operator for step' crates/orrery-cli/src/commands.rs && grep -q 'Coordinate stage execution' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs: delete `#[rustfmt::skip]` above `validate_tool_args`; `cargo fmt --all` wraps the signature.
2. config.rs `default_roles_with_rules`: replace the three rules strings with exactly this text (each ends with one `\n`; a raw string or `concat!` is a free choice):
   Orchestrator:
     # Orchestrator Role Rules
     (blank)
     You coordinate the stages. You never do the work yourself.
     (blank)
     - Give each allocation clear, self-contained instructions: what to do, which files or areas, and what "done" looks like.
     - Keep each allocation to one concern. Split bigger work across allocations or rounds.
     - When a Validator reports `Denied`, pass its feedback on word for word.
     - If you can't tell what the plan step wants, `Escalate` and say what is unclear. Don't guess.
   Coder:
     # Implementation Role Rules
     (blank)
     You do only what "# Your assignment" says, inside its write scope.
     (blank)
     - Don't fix, improve or reformat anything you weren't asked to change.
     - If the assignment says no files change, change nothing.
     - If you need a tool you don't have, or the assignment is unclear, report `Blocked` and say what you need. Don't guess.
     - Your summary says what you changed, or that you changed nothing.
   Validator:
     # Validator Role Rules
     (blank)
     You check the Implementer's work against the plan step. You never change anything.
     (blank)
     - Compare the plan step and "# Your assignment" with the Implementer's latest report in "# Prior reports", and with whatever your tools let you inspect.
     - Confirm only what you can check. When the plan step changes no files, the Implementer's report that it changed nothing is enough.
     - If the Implementer's report is missing, `Blocked`, or contradicts the plan step, report `Denied`.
     - Your feedback names exactly what is wrong, so the Implementer can fix it.
3. Test in config.rs `mod tests`, `default_role_rules_describe_behavior_only`: for each `(role, text)` in `default_roles_with_rules()`: `text` does not contain `"Allowed statuses"` or `"target_stage"`, and ends with `"\n"`. The orchestrator text contains `"You never do the work yourself."`, the coder text `"Don't guess."`, the validator text `"You never change anything."`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'rustfmt::skip' crates/orrery-core/src/config.rs && ! grep -q 'Coordinate stage execution' crates/orrery-core/src/config.rs && grep -q 'You never do the work yourself.' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::default_role_rules_describe_behavior_only: test$')" -eq 1

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 99: README Roles, Protocol and Skills

* **Recommended Command:** `/goal`

Scope note: docs only. States only what Steps 93–98 built.

```text
[STEP_ID]
STEP 99: the README explains the generated protocol section, what role rules are for, skills, role add --skill and the awaiting-operator message

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1474-1517 (§9.18; binding)
  - README.md:98-124 (`## Project layout`: tree and bullets), 125-145 (`### Hook and gate arguments`: the new section goes directly after it)
  - crates/orrery-core/src/config.rs (`default_roles_with_rules`, from Step 98)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No other section changes.

[PRE_GATE]
grep -q 'You never do the work yourself.' crates/orrery-core/src/config.rs && grep -q '^### Hook and gate arguments' README.md && ! grep -q '^### Roles, protocol and skills' README.md

[MUTATION_SPEC]
1. Project layout tree: add `├── skills/<name>.md` under `<project_root>/.orrery/` (after `roles/`) and under `~/.config/orrery/` (after `roles/`).
2. The `roles/<role_id>.toml` bullet: add "and optional `skills` (skill names)".
3. New `### Roles, protocol and skills` directly after the "Hook and gate arguments" section:
   - One sentence: every agent's system prompt starts with an "Orrery protocol" section that Orrery generates from its report checks (allowed statuses for the role, required fields, and for the orchestrator the dispatch and pass rules), so rules files never repeat it.
   - One sentence: the rules file (`roles/<role_id>.md`) holds the role's scope and behavior; `orrery init` writes starting rules for the orchestrator, coder and validator.
   - One sentence: a skill is a markdown file `skills/<name>.md` (project, or the global catalog; project wins); a role lists skills by name, and each is appended to its rules under `# Skill: <name>`. A missing skill fails the run before any agent starts.
   - A `toml` block: a role TOML with `skills = ["house-style"]`; a `bash` block: `orrery role add --id reviewer --name Reviewer --kind validator --rules-file reviewer.md --skill house-style`.
4. Quickstart step 8's comment, or one line below its block: when a step waits for an operator, `orrery run` and `orrery resume` print the reason and the next commands, and exit with code 3.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && grep -q '^### Roles, protocol and skills' README.md && grep -q 'skills/<name>.md' README.md && grep -q -- '--skill house-style' README.md && grep -q 'Orrery protocol' README.md

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### User Step: LIVE-3

After Step 99 is committed. Proves the generated protocol plus the new default rules are enough for a local model, with no hand-written rules. Needs `llama-qwen38-27b` running on 8085 and `neb` from Nebulous `bc00dd3` or later.

1. Reinstall Orrery from this repo:

   ```bash
   cargo install --path /srv/devshare/Orrery/crates/orrery-cli --locked
   ```

2. Fresh scratch repo, set up with defaults only:

   ```bash
   mkdir -p ~/scratch/orrery-live3 && cd ~/scratch/orrery-live3 && git init && neb init && neb project switch ~/scratch/orrery-live3
   ```

   ```bash
   orrery init --name orrery-live3 && orrery inference add --id local --endpoint http://127.0.0.1:8085
   ```

   ```bash
   orrery agent add-local-model --id local-agent --name "Local Agent" --inference local --model qwen3.8-27b
   ```

   ```bash
   orrery workflow assign 0 local-agent && orrery workflow assign 1 local-agent && orrery workflow assign 2 local-agent
   ```

   ```bash
   printf '## Step 1: Acknowledge\n\nThis step changes no files.\n' > PLAN.md && orrery ingest PLAN.md --marker "## Step "
   ```

   ```bash
   orrery mcp add-stdio --id neb --command neb --arg mcp && orrery hook set pass neb:git_commit_documented --args '{"summary":"orrery step pass","dry_run":false}'
   ```

3. Don't edit anything under `.orrery/roles/`. Run:

   ```bash
   neb task "live3 check" && orrery run
   ```

   ```bash
   orrery status && git log -1 --stat
   ```

   Expect: step 1 `Passed`; the last commit is `orrery step pass` and touches only `.tracker/`.

4. Skill check: add a skill to the validator, reset, run, and check the skill reached the prompt (the reasoning stream quotes it, or `orrery logs` shows it):

   ```bash
   mkdir -p .orrery/skills && printf 'Start every summary with the word LIVE3.\n' > .orrery/skills/live3-mark.md
   ```

   Add `skills = ["live3-mark"]` to `.orrery/roles/validator.toml`, then:

   ```bash
   orrery reset --step 1 && orrery run
   ```

   Expect: the validator's `Confirmed` summary starts with `LIVE3`.

Report back: pass/fail for 3 and 4, plus `orrery status`.

### Parked (Phase M)

Migrated to neb BUG-0002 (2026-11-18). Kept for history; do not add here.

- `crates/orrery-cli/src/commands.rs:1145` (and every `ConfigStore::new(dir.path(), None)` / `dispatch` test) — CLI tests resolve the global catalog through `dirs::config_dir()`, so any real `~/.config/orrery/agents/*.toml` leaks into `load_agents` and fails `agent_add_each_backend_and_list_json` (6 ≠ 3), `agent_show_and_remove` and `agent_add_with_embedded_secret_exits_1`. Seen during the Step 99 check; it passed with `XDG_CONFIG_HOME` pointed at an empty dir — tests should pass an explicit temp `xdg_override` (or the CLI context should carry one) so they never read the operator's catalog.

---

## Phase N — config safety, global profiles and the gate-failure rule (Steps 100–110, User Step LOOP-1)

**Phase goal (frozen until the phase ends):** a project's configuration is never silently overwritten, every project setting has a CLI, and a machine-wide setup can be written into a new project in one command: `init` refuses a configured project (BUG-0013); TUI saves merge into the file on disk (BUG-0015); the Agents-mode protocol routes a failed gate back to the implementer (BUG-0014); `role skills`, `config set` and `config show` (with each catalog entry's source) cover what was hand-edited (TASK-0007); `init --profile` applies a global profile saved with `profile save`. Loop behavior itself doesn't change. Done when User Step LOOP-1 passes. Design: spec §9.19 (`ORRERY_TECHNICAL_SPECIFICATION.md:1519-1590`).

Written 2026-11-18 against `014aeb1`. Facts checked while planning:
- `init_project` (`config.rs:1749`) writes `project.toml` and all three default roles with no existence check. A trial refusal broke no existing test (no test re-inits the same directory).
- The TUI loads `ProjectConfig` once (`bridge.rs:633`) and six actions save that whole copy (`bridge.rs:338-344`, `366`).
- Orrery notes reach the orchestrator once (`engine.rs:601`, `std::mem::take`), so a "gate failed" note is always new on the turn it appears. The orchestrator test at `llm.rs:3680` asserts no rule number, so renumbering breaks nothing.
- No CLI today for `validation_mode`, `max_step_retries`, `max_concurrent_agents`, `[limits]`, or an existing role's `skills` (the orrery-setup skill hand-edits all of them).
- CLI tests use `test_config_home()` (`commands.rs`, BUG-0002), a per-process path shared by every test. Tests that write a profile must use their own `config_home` under their `TempDir`, or they leak into other tests.

### STEP 100: init Refuses a Configured Project

* **Recommended Command:** `/goal`

Advances: BUG-0013.

```text
[STEP_ID]
STEP 100: ConfigStore::init_project returns a Validation error, before writing anything, when .orrery/project.toml already exists

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1522-1523 (§9.19.1; binding)
  - crates/orrery-core/src/config.rs:1749-1766 (`init_project`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No `--force`, no new CLI flag. The rest of `init_project` is unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.1 ' $S && ! grep -q 'never overwrites a configured project' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs `init_project`: as its first statement after `let project_name = name.into();`, add:
   `let existing = self.orrery_dir().join("project.toml");`
   `if existing.exists() { return Err(Error::Validation { field: "init".to_string(), message: format!("{} already exists; orrery init never overwrites a configured project. Move .orrery/ aside to start over.", existing.display()) }); }`
2. Test in config.rs `mod tests`, `init_project_refuses_existing_project`: `TempDir` project and `TempDir` global; `ConfigStore::new(project, Some(global))`; `init_project("t")` ok. Load the project, set `max_step_retries = 7`, `save_project`; write `.orrery/roles/coder.md` = `"custom rules\n"`. Second `init_project("t")` → `Err(Error::Validation { field, message })` with `field == "init"` and `message.contains("never overwrites a configured project")`. Then `load_project().max_step_retries == 7` and `.orrery/roles/coder.md` reads `"custom rules\n"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'never overwrites a configured project' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::init_project_refuses_existing_project: test$')" -eq 1

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 101: TUI Saves Merge into the File on Disk

* **Recommended Command:** `/goal`

Advances: BUG-0015.

```text
[STEP_ID]
STEP 101: the TUI reloads project.toml before each config save and writes back only the fields the action changed (merge_action_into), so CLI and hand edits survive

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1525-1532 (§9.19.2; binding)
  - crates/orrery-tui/src/bridge.rs:336-372 (`EngineController::handle`), 871-911 (`setup_project_dir`), 1314 (`config_edit_persists`)
  - crates/orrery-tui/src/app.rs:101-115 (`AppAction`), 188 (`pub fn refresh_warnings`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `app.rs` is not touched: the App still edits `app.config` first and emits the action. The failed-ingest paths that restore `step_marker` from disk stay as they are.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.2 ' $S && grep -q 'never overwrites a configured project' crates/orrery-core/src/config.rs && ! grep -q 'fn merge_action_into(' crates/orrery-tui/src/bridge.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. bridge.rs, a free function above `impl EngineController`: `pub fn merge_action_into(disk: &mut ProjectConfig, edited: &ProjectConfig, action: &AppAction)` with one `match action`:
   - `AppAction::SetMode(_)` → `disk.execution_mode = edited.execution_mode;`
   - `AppAction::SetCycles(_)` → `disk.cycle_count = edited.cycle_count;`
   - `AppAction::SetStageCount(_) | AppAction::SetStageRole { .. } | AppAction::AssignAgent { .. } | AppAction::UnassignAgent { .. }` → `disk.stages = edited.stages.clone();`
   - `AppAction::IngestPlan(_)` → `disk.plan_path = edited.plan_path.clone(); disk.step_marker = edited.step_marker.clone();`
   - `_ => {}`
2. `EngineController::handle`: in the six-action arm and in the successful `IngestPlan` path, replace `self.store.save_project(&app.config)?;` with:
   `let mut disk = self.store.load_project()?;`
   `merge_action_into(&mut disk, &app.config, action);`
   `self.store.save_project(&disk)?;`
   `app.config = disk;`
   `app.refresh_warnings();`
   (A private helper `fn save_merged(&self, action: &AppAction, app: &mut App) -> Result<()>` holding these five lines is a free choice.)
3. Tests in bridge.rs `mod tests`:
   - `merge_action_into_copies_only_action_fields`: `disk = ProjectConfig::default_template("d")`, `disk.gate_tools = vec!["srv:gate".into()]`; `edited = disk.clone()` with `execution_mode = ExecutionMode::Loop`, `cycle_count = 4`, `gate_tools = Vec::new()`, `stages[1].agent_ids = vec!["a".into()]`. `merge_action_into(&mut disk, &edited, &AppAction::SetMode(ExecutionMode::Loop))` → `disk.execution_mode == Loop`, `disk.cycle_count == 0`, `disk.gate_tools == ["srv:gate"]`, `disk.stages[1].agent_ids.is_empty()`. Then `SetCycles(4)` → `cycle_count == 4`. Then `AssignAgent { stage: 1, agent_id: "a".into() }` → `disk.stages[1].agent_ids == ["a"]`, `gate_tools` still `["srv:gate"]`. Then `AppAction::Quit` → `disk` unchanged.
   - `tui_save_keeps_disk_changes`: `let (_tmp, store, mut app) = setup_project_dir(ValidationMode::Auto);` build an `EngineController` as `config_edit_persists` does. Simulate a CLI edit: `let mut cfg = store.load_project().unwrap(); cfg.max_step_retries = 9; cfg.limits.mcp_call_timeout_secs = 1800; store.save_project(&cfg).unwrap();`. In the TUI: `app.config.execution_mode = ExecutionMode::Loop;` then `controller.handle(&AppAction::SetMode(ExecutionMode::Loop), &mut app).await.unwrap()`. Reloaded from disk: `execution_mode == Loop`, `max_step_retries == 9`, `limits.mcp_call_timeout_secs == 1800`; and `app.config.max_step_retries == 9`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn merge_action_into(' crates/orrery-tui/src/bridge.rs && test "$(grep -c 'save_project(&app.config)' crates/orrery-tui/src/bridge.rs)" -le 2 && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^bridge::tests::(merge_action_into_copies_only_action_fields|tui_save_keeps_disk_changes): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

The `-le 2` allows the two existing `save_project(&app.config)` calls inside test helpers (`bridge.rs:899`, `2081`); production code has none after this step.

### STEP 102: Gate-Failure Rule in the Agents Protocol

* **Recommended Command:** `/goal`

Advances: BUG-0014.

```text
[STEP_ID]
STEP 102: the Agents-mode orchestrator contract gets rule 2, "a gate failed → Dispatch back to the Implementer with the gate output", and the later rules renumber to 3–5

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/llm.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1534-1541 (§9.19.3; binding)
  - crates/orrery-core/src/llm.rs:135-149 (the non-GateAndOperator rule lines), 3680-3700 (contract tests)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The GateAndOperator rules and every other contract line are unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.3 ' $S && grep -q 'pub fn merge_action_into(' crates/orrery-tui/src/bridge.rs && ! grep -q 'says a gate failed' crates/orrery-core/src/llm.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. llm.rs `protocol_contract`, the `else` branch (any mode other than GateAndOperator): after the rule-1 push, push
   `"2. \"# Notes from Orrery\" says a gate failed → `Dispatch` back to the Implementer stage, with the gate output from the note in `instructions`."`
   and renumber the existing three rules: `Denied` becomes `3.`, `Confirmed` becomes `4.`, `Otherwise` becomes `5.`. Their text after the number is unchanged.
2. Test in llm.rs `mod tests`, `contract_agents_mode_routes_gate_failure`: `protocol_contract(RoleKind::Orchestrator, 0, 3, ValidationMode::Agents)` contains `"2. \"# Notes from Orrery\" says a gate failed → `Dispatch` back to the Implementer stage"`, `"3. The latest Validator report is `Denied`"`, `"4. The latest Validator stage reported `Confirmed`"` and `"5. Otherwise → `Dispatch` to the next stage in order."`; the GateAndOperator contract does not contain `"says a gate failed"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'says a gate failed' crates/orrery-core/src/llm.rs && grep -q '5. Otherwise' crates/orrery-core/src/llm.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^llm::tests::contract_agents_mode_routes_gate_failure: test$')" -eq 1

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 103: CLI role skills and role show Lists Skills

* **Recommended Command:** `/goal`

Advances: TASK-0007.

```text
[STEP_ID]
STEP 103: orrery role skills <ID> [NAME]... sets an existing role's skills, and orrery role show prints a skills: line

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/args.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1543-1545 (§9.19.4; binding)
  - crates/orrery-cli/src/args.rs:323-348 (`RoleCmd`), the `every_spec_row_parses` test
  - crates/orrery-cli/src/commands.rs:559-625 (`dispatch_role`, `RoleCmd::Show`), the `role_add_reads_rules_file` test
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `role add` is unchanged.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.4 ' $S && grep -q 'says a gate failed' crates/orrery-core/src/llm.rs && ! grep -q 'Set skills for role' crates/orrery-cli/src/commands.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. args.rs `RoleCmd`: new variant after `Show`: `Skills { id: String, names: Vec<String> },` (both positional; `names` may be empty).
2. args.rs `every_spec_row_parses`: add rows `&["orrery", "role", "skills", "coder", "house-style", "repo-rules"]` and `&["orrery", "role", "skills", "coder"]`.
3. commands.rs `dispatch_role`, new arm `RoleCmd::Skills { id, names }`: find the role in `store.load_roles()?` (none → `Error::Validation { field: "role_id".into(), message: format!("unknown role_id '{id}'") }`), `let rules = store.load_role_rules(&id)?;`, set `role.skills = names;`, `store.save_role(&role, &rules)?;`, then `write_stdout_line(ctx, &format!("Set skills for role '{id}': {:?}", role.skills))?;` and `Ok(0)`.
4. commands.rs `RoleCmd::Show`: right after the `allowed_mcp_tools:` line, `write_stdout_line(ctx, &format!("skills: {:?}", role.skills))?;`.
5. Tests in commands.rs `mod tests`:
   - `role_skills_sets_and_clears`: `init` a project (as `role_add_reads_rules_file` does). `RoleCmd::Skills { id: "coder", names: ["house-style", "repo-rules"] }` → code 0; the loaded `coder` role has `skills == ["house-style", "repo-rules"]` and `load_role_rules("coder")` is unchanged from before. Then `names: []` → `skills` is empty. Then `id: "nope"` → `Err(Error::Validation { .. })`. Then `names: ["../x"]` → `Err(Error::PathTraversal { .. })`.
   - `role_show_prints_skills`: after setting `coder` skills to `["house-style"]`, `out.clear()`, `RoleCmd::Show { id: "coder" }` → stdout contains `skills: ["house-style"]`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'Set skills for role' crates/orrery-cli/src/commands.rs && grep -q '"skills: {:?}"' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(role_skills_sets_and_clears|role_show_prints_skills): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 104: Settable Project Fields

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 104: ProjectConfig::set_field(key, value) sets validation_mode, max_step_retries, max_concurrent_agents and the six limits.* fields from strings, listed in SETTABLE_KEYS

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1547-1554 (§9.19.5; binding)
  - crates/orrery-core/src/config.rs:159-179 (`RuntimeLimits`), 229-249 (`ProjectConfig`), 303 (`validate_staging`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. No CLI yet (Step 106). `execution_mode`, `cycle_count`, `plan_path` and `step_marker` are not settable here.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.5 ' $S && grep -q 'Set skills for role' crates/orrery-cli/src/commands.rs && ! grep -q 'pub fn set_field(' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs, above `impl ProjectConfig`: `pub const SETTABLE_KEYS: [&str; 9] = [...]` with exactly the nine keys of §9.19.5, in that order.
2. config.rs `impl ProjectConfig`: `pub fn set_field(&mut self, key: &str, value: &str) -> Result<()>` per §9.19.5: one `match key`; `validation_mode` matches the six accepted spellings; each integer key uses `value.parse::<T>()` for its field's type (`usize` or `u64`) and maps a parse error to the §9.19.5 message; the unknown-key message joins `SETTABLE_KEYS` with `", "`. The function ends with `self.validate_staging()`.
3. Tests in config.rs `mod tests`:
   - `set_field_known_keys`: on `ProjectConfig::default_template("t")`: `set_field("validation_mode", "agents")` → `Agents`; `"GateAndOperator"` → `GateAndOperator`; `"auto"` → `Auto`; `set_field("max_step_retries", "2")` → 2; `set_field("max_concurrent_agents", "1")` → 1; `set_field("limits.mcp_call_timeout_secs", "1800")` → 1800; and each of the other five `limits.*` keys with a valid value (`max_tool_calls` 40, `model_idle_timeout_secs` 120, `live_stream_ring_capacity` 500, `live_stream_max_file_bytes` 1048576, `live_stream_max_files` 2) lands in its field. `SETTABLE_KEYS.len() == 9`.
   - `set_field_rejects_bad_input`: `set_field("validation_mode", "sometimes")` → `Validation { field: "validation_mode", .. }`; `set_field("max_step_retries", "-1")` → `Validation { field: "max_step_retries", message }` with `message.contains("non-negative integer")`; `set_field("execution_mode", "loop")` → `Validation { field: "config", message }` with `message.contains("unknown key 'execution_mode'")` and `message.contains("limits.max_tool_calls")`; `set_field("max_concurrent_agents", "0")` → `Validation { field: "max_concurrent_agents", .. }` (from `validate_staging`); `set_field("limits.max_tool_calls", "0")` → `Validation { field: "limits.max_tool_calls", .. }`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn set_field(' crates/orrery-core/src/config.rs && grep -q 'pub const SETTABLE_KEYS' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(set_field_known_keys|set_field_rejects_bad_input): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 105: Catalog Sources

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 105: ConfigStore::catalog_entries lists every agent, role, skill, MCP server and inference id with its source (Global, Project or ProjectOverridesGlobal)

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1556-1561 (§9.19.6; binding)
  - crates/orrery-core/src/config.rs:1726 (`global_dir`), 1796 (`load_roles`), 1858 (`load_skill`), 1959 (`load_agents`), 2016 (`load_mcp_servers`), 2088 (`load_inference`), 2318-2410 (`read_*` helpers), 1611 (`validate_config_file_id`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The existing `load_*` functions are unchanged. No CLI yet (Step 106).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.6 ' $S && grep -q 'pub fn set_field(' crates/orrery-core/src/config.rs && ! grep -q 'pub fn catalog_entries(' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs: `CatalogKind`, `CatalogSource` and `CatalogEntry` exactly as §9.19.6 (derives and serde attributes as written; variant order as written).
2. config.rs `impl ConfigStore`: `pub fn catalog_entries(&self) -> Result<Vec<CatalogEntry>>` per §9.19.6. Read the global and project sides separately with the existing helpers (`read_roles_from_dir`, `read_agents_from_dir`, `read_mcp_servers_file`, `read_inference_file`; project inference also includes `load_project()?.inference` when `project.toml` exists). For skills, a private helper `fn skill_ids_in(dir: &Path) -> Result<Vec<String>>` lists `*.md` stems that pass `validate_config_file_id`, and returns empty when `dir` doesn't exist. Sort the result by `(kind, id)`.
3. Test in config.rs `mod tests`, `catalog_entries_report_sources`: `TempDir` project and global; `ConfigStore::new(project, Some(global))` and `init_project("t")` (writes the three default roles in the project). Global: write `<global>/orrery/agents/g-only.toml` and `<global>/orrery/agents/both.toml` (valid `LocalModel` agents with `inference_id = "inf"`; copy the shape another config.rs test uses), `<global>/orrery/roles/coder.toml` + `coder.md` (a copy of the default coder), `<global>/orrery/skills/style.md`, and `<global>/orrery/skills/notes.txt`. Project: `.orrery/agents/both.toml`, `.orrery/skills/style.md` and `.orrery/skills/p-skill.md`. Expect, among the entries: `(Agent, "g-only", Global)`, `(Agent, "both", ProjectOverridesGlobal)`, `(Role, "coder", ProjectOverridesGlobal)`, `(Role, "validator", Project)`, `(Skill, "style", ProjectOverridesGlobal)`, `(Skill, "p-skill", Project)`; no entry has id `"notes"`; and the list equals itself sorted by `(kind, id)`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn catalog_entries(' crates/orrery-core/src/config.rs && grep -q 'pub enum CatalogSource' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::catalog_entries_report_sources: test$')" -eq 1

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 106: CLI config set and config show

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 106: orrery config set <KEY> <VALUE> edits project.toml through set_field, and orrery config show [--json] prints the project config and the catalog with sources

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/args.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1563-1567 (§9.19.7; binding)
  - crates/orrery-cli/src/args.rs:37-110 (`Command`), the `every_spec_row_parses` test
  - crates/orrery-cli/src/commands.rs:114-150 (`dispatch` match), 357-365 (`edit_project`), 1091 (`test_ctx`)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `orrery-core` is not touched.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.7 ' $S && grep -q 'pub fn catalog_entries(' crates/orrery-core/src/config.rs && ! grep -q 'ConfigCmd::Show' crates/orrery-cli/src/commands.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. args.rs: `Command::Config { #[command(subcommand)] command: ConfigCmd }` (after `Hook`), and `pub enum ConfigCmd { Set { key: String, value: String }, Show { #[arg(long)] json: bool } }` with the same derives as `GateCmd`.
2. args.rs `every_spec_row_parses`: rows `&["orrery", "config", "set", "max_step_retries", "2"]`, `&["orrery", "config", "show"]`, `&["orrery", "config", "show", "--json"]`.
3. commands.rs: `dispatch` routes `Command::Config { command }` to a new `fn dispatch_config(command: ConfigCmd, project: &Path, ctx: &mut CliContext) -> Result<u8>` that builds `ConfigStore::new(project, ctx.config_home.clone())?` and implements §9.19.7. Text `show`: `kind`/`source` strings come from `serde_json::to_value(...)` of the enum (a `Value::String`), not a hand-written match.
4. Tests in commands.rs `mod tests`:
   - `config_set_updates_project`: `init`; `ConfigCmd::Set { key: "validation_mode", value: "agents" }` → 0 and stdout contains `Set validation_mode = agents`; `"limits.mcp_call_timeout_secs"`/`"1800"` → 0; the loaded project has `validation_mode == Agents` and `limits.mcp_call_timeout_secs == 1800`. `"bogus"`/`"1"` → `Err(Error::Validation { .. })`, and the loaded project still has `validation_mode == Agents`.
   - `config_show_text_and_json`: `init`; `Show { json: false }` → stdout contains `project: `, `validation_mode = `, `catalog:` and `  role coder (project)`. `out.clear()`; `Show { json: true }` → stdout parses as a `serde_json::Value` with `["project"]["project_name"]` a string and `["catalog"]` an array containing `{"kind": "role", "id": "coder", "source": "project"}`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'ConfigCmd::Show' crates/orrery-cli/src/commands.rs && grep -q 'fn dispatch_config(' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(config_set_updates_project|config_show_text_and_json): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 107: Global Profiles

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 107: ProjectProfile (from_project, apply) and ConfigStore::load_profile / save_profile / list_profiles over <global_dir>/profiles/<name>.toml

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1569-1576 (§9.19.8; binding)
  - crates/orrery-core/src/config.rs:38-100 (`StageConfig`, `HooksConfig`), 159 (`RuntimeLimits`), 229-249 (`ProjectConfig`), 1544 (`atomic_write`), 1611 (`validate_config_file_id`), 1726 (`global_dir`), 1858 (`load_skill`, the not-found error shape)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `init_project` is unchanged (Step 108). No CLI yet (Step 109).

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.8 ' $S && grep -q 'ConfigCmd::Show' crates/orrery-cli/src/commands.rs && ! grep -q 'pub struct ProjectProfile' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs: `pub struct ProjectProfile` exactly as §9.19.8 (derives, `deny_unknown_fields`, field order, per-field serde attributes, all fields `pub`).
2. config.rs `impl ProjectProfile`: `pub fn from_project(cfg: &ProjectConfig, roles: &[RoleConfig]) -> Self` and `pub fn apply(&self, cfg: &mut ProjectConfig)` per §9.19.8.
3. config.rs `impl ConfigStore`: `pub fn load_profile(&self, name: &str) -> Result<ProjectProfile>`, `pub fn save_profile(&self, name: &str, profile: &ProjectProfile) -> Result<PathBuf>`, `pub fn list_profiles(&self) -> Result<Vec<String>>` per §9.19.8, with the error fields and messages as written.
4. Tests in config.rs `mod tests`:
   - `profile_round_trip_and_apply`: `cfg = ProjectConfig::default_template("src")` with `validation_mode = Agents`, `max_step_retries = 2`, `max_concurrent_agents = 1`, `gate_tools = ["hybrid:step_check"]`, `stages[1].agent_ids = ["agy-impl"]`, `limits.mcp_call_timeout_secs = 1800`, and `hooks.on_step_start = Some(HookConfig { tool: "hybrid:step_start".into(), on_failure: HookFailure::Halt, args: serde_json::Map::new() })`; roles = the default roles with `coder.skills = ["repo-rules"]`. `p = ProjectProfile::from_project(&cfg, &roles)`; `p.role_skills == {"coder": ["repo-rules"]}`. Save with `save_profile("cloud", &p)` into a `TempDir` global; the returned path ends with `profiles/cloud.toml`; `load_profile("cloud") == p`. `target = ProjectConfig::default_template("dst")`; `p.apply(&mut target)`; `target.project_name == "dst"`, `target.plan_path` unchanged, and `validation_mode`, `max_step_retries`, `gate_tools`, `stages`, `hooks`, `limits` equal `cfg`'s. `ProjectProfile::default().apply(&mut x)` leaves `x` equal to a fresh template.
   - `profile_load_errors_and_list`: empty global `TempDir`: `list_profiles() == []`; `load_profile("nope")` → `Validation { field: "profile", message }` with `message.contains("profile 'nope' not found")`; `load_profile("../x")` → `PathTraversal { .. }`. Write `profiles/b.toml` = `max_step_retries = 1\n`, `profiles/a.toml` = `"validation_mode = \"Agents\"\n"`, `profiles/notes.md`; `list_profiles() == ["a", "b"]`. Write `profiles/bad.toml` = `"project_name = \"x\"\n"` → `load_profile("bad")` is `Err(Error::TomlDe { .. })` (deny_unknown_fields).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub struct ProjectProfile' crates/orrery-core/src/config.rs && grep -q 'pub fn load_profile(' crates/orrery-core/src/config.rs && grep -q 'pub fn save_profile(' crates/orrery-core/src/config.rs && grep -q 'pub fn list_profiles(' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(profile_round_trip_and_apply|profile_load_errors_and_list): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 108: init with a Profile

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 108: ConfigStore::init_project_with_profile applies a profile's settings and role skills to a new project, validating everything before writing; init_project wraps it with None

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-core/src/config.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1578-1581 (§9.19.9; binding)
  - crates/orrery-core/src/config.rs:1749 (`init_project`, with Step 100's check), `default_roles_with_rules`, `RoleConfig::validate`
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `init_project`'s signature and its callers are unchanged. Skill files are not checked at init.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.9 ' $S && grep -q 'pub fn list_profiles(' crates/orrery-core/src/config.rs && ! grep -q 'pub fn init_project_with_profile(' crates/orrery-core/src/config.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. config.rs `impl ConfigStore`: `pub fn init_project_with_profile(&self, name: impl Into<String>, profile: Option<&ProjectProfile>) -> Result<ProjectConfig>` doing exactly the §9.19.9 order. Default role ids come from `default_roles_with_rules()`.
2. `init_project(&self, name)` body becomes `self.init_project_with_profile(name, None)`.
3. Tests in config.rs `mod tests`:
   - `init_with_profile_applies_settings_and_skills`: profile with `validation_mode: Some(Agents)`, `max_step_retries: Some(2)`, `gate_tools: Some(vec!["hybrid:step_check".into()])`, `role_skills: {"orchestrator": ["hybrid-orchestrator", "repo-rules"], "coder": ["repo-rules"]}`. `init_project_with_profile("p", Some(&profile))` → the loaded project has those three values and `project_name == "p"`; loaded `orchestrator.skills == ["hybrid-orchestrator", "repo-rules"]`, `coder.skills == ["repo-rules"]`, `validator.skills` empty; `load_role_rules("coder")` equals the default coder rules text.
   - `init_with_bad_profile_writes_nothing`: (a) `role_skills: {"reviewer": ["x"]}` → `Validation { field: "role_skills", message }` with `message.contains("unknown default role 'reviewer'")`; (b) `max_concurrent_agents: Some(0)` → `Validation { field: "max_concurrent_agents", .. }`; (c) `role_skills: {"coder": ["../x"]}` → `PathTraversal { .. }`. After each, `.orrery/` does not exist in that project dir (use a fresh `TempDir` per case).

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'pub fn init_project_with_profile(' crates/orrery-core/src/config.rs && grep -q 'self.init_project_with_profile(name, None)' crates/orrery-core/src/config.rs && test "$(cargo test -p orrery-core --lib --locked -- --list 2>/dev/null | grep -cE '^config::tests::(init_with_profile_applies_settings_and_skills|init_with_bad_profile_writes_nothing): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 109: Profile CLI

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 109: orrery init --profile <P>, orrery profile save <NAME> and orrery profile list [--json]

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/src/args.rs
  - crates/orrery-cli/src/commands.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1583-1586 (§9.19.10; binding)
  - crates/orrery-cli/src/args.rs:42-46 (`Command::Init`), the `every_spec_row_parses` test
  - crates/orrery-cli/src/commands.rs:76-99 (`Command::Init` dispatch), 1091 (`test_ctx`), the `test_config_home` helper
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. `orrery-core` is not touched. Tests never write a profile under `test_config_home()`.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.10 ' $S && grep -q 'pub fn init_project_with_profile(' crates/orrery-core/src/config.rs && ! grep -q 'Applied profile' crates/orrery-cli/src/commands.rs && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. args.rs `Command::Init`: add `#[arg(long)] profile: Option<String>,` after `name`. New `Command::Profile { #[command(subcommand)] command: ProfileCmd }` (after `Config`) and `pub enum ProfileCmd { Save { name: String }, List { #[arg(long)] json: bool } }` with the same derives as `GateCmd`.
2. args.rs `every_spec_row_parses`: rows `&["orrery", "init", "--profile", "cloud-cli"]`, `&["orrery", "profile", "save", "cloud-cli"]`, `&["orrery", "profile", "list", "--json"]`. Existing `Command::Init { .. }` literals in tests gain `profile: None`.
3. commands.rs `Command::Init` arm: per §9.19.10. Without `--profile` it calls `init_project` exactly as today.
4. commands.rs: `dispatch` routes `Command::Profile { command }` to a new `fn dispatch_profile(command: ProfileCmd, project: &Path, ctx: &mut CliContext) -> Result<u8>` implementing `save` and `list` per §9.19.10.
5. Tests in commands.rs `mod tests`, each setting `ctx.config_home = Some(<its TempDir>.path().join("xdg"))` right after `test_ctx()`:
   - `profile_save_then_init_with_profile`: project A: `init`, then `ConfigCmd::Set` `validation_mode` → `agents` and `max_step_retries` → `2`, then `RoleCmd::Skills { id: "coder", names: ["repo-rules"] }`; `ProfileCmd::Save { name: "cloud" }` → 0, stdout contains `Saved profile 'cloud' to ` and `profiles/cloud.toml` exists under the xdg dir. Project B (another `TempDir`, same `ctx`): `Command::Init { path: None, name: Some("b"), profile: Some("cloud") }` → 0, stdout contains `Applied profile 'cloud'`; B's project has `validation_mode == Agents`, `max_step_retries == 2`, `project_name == "b"`, and B's `coder.skills == ["repo-rules"]`. `Init` with `profile: Some("missing")` in a third dir → `Err(Error::Validation { .. })` and that dir has no `.orrery/`.
   - `profile_list_json`: save profiles `b` and `a` from an initialized project; `ProfileCmd::List { json: true }` → stdout parses as `["a", "b"]`; `out.clear()`; `List { json: false }` → stdout is `"a\nb\n"`.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q 'Applied profile' crates/orrery-cli/src/commands.rs && grep -q 'fn dispatch_profile(' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(profile_save_then_init_with_profile|profile_list_json): test$')" -eq 2

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### STEP 110: README Config, Profiles and Gate Routing

* **Recommended Command:** `/goal`

```text
[STEP_ID]
STEP 110: the README documents config set/show, role skills, global profiles, the init refusal, TUI merge-on-save and gate-failure routing

[SCOPE_LOCK]
ALLOWED_FILES:
  - README.md
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1519-1590 (§9.19; binding)
  - README.md: the project layout tree, the "Hook and gate arguments" and "Roles, protocol and skills" sections, the quickstart
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. State only what §9.19 specifies; no claims about untested behavior.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && grep -q '^#### 9.19.11 ' $S && grep -q 'Applied profile' crates/orrery-cli/src/commands.rs && ! grep -q '^### Global profiles' README.md && cargo test --workspace --locked >/dev/null 2>&1

[MUTATION_SPEC]
1. Layout tree, under `~/.config/orrery/`: add `├── profiles/<name>.toml` after `skills/<name>.md`.
2. Quickstart, right after the `orrery init` line: one sentence that `orrery init` refuses a directory that already has `.orrery/project.toml` (exit 1); move `.orrery/` aside to start over.
3. "Roles, protocol and skills" section: add a `bash` line `orrery role skills coder repo-rules` with one sentence (sets an existing role's skills; no names clears them; `role show` lists them); and one sentence that in Agents mode a failed gate goes back to the Implementer with the gate output.
4. New `### Project settings` section after it: one sentence each for `orrery config set <KEY> <VALUE>` (the nine `SETTABLE_KEYS`, listed in one inline-code list) and `orrery config show [--json]` (the project config plus every catalog entry marked `global`, `project` or `project_overrides_global`), and a `bash` block with `orrery config set validation_mode agents` and `orrery config set limits.mcp_call_timeout_secs 1800`.
5. New `### Global profiles` section after that: what a profile holds (§9.19.8 fields, and never `project_name`, `plan_path` or `inference`), where it lives, and a `bash` block: `orrery profile save cloud-cli`, `orrery profile list`, `orrery init --name NewRepo --profile cloud-cli`. One sentence: MCP servers named by a profile's gate or hooks must be in the global `mcp/servers.toml` (or added to the new project) for the run to start.
6. TUI keys section: one sentence that TUI config changes reload `project.toml` and write back only the changed setting, so CLI or hand edits made while the TUI is open are kept.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && grep -q '^### Global profiles' README.md && grep -q '^### Project settings' README.md && grep -q 'profiles/<name>.toml' README.md && grep -q 'orrery role skills coder repo-rules' README.md && grep -q 'orrery init --name NewRepo --profile cloud-cli' README.md && grep -q 'project_overrides_global' README.md

[HALT_INVARIANT]
Halt and report if any POST_GATE command fails after two fix attempts, if a change is needed outside ALLOWED_FILES, or if this block contradicts the spec. Never commit; never edit the spec or this plan.
```

### User Step: LOOP-1

After Step 110 is committed. Proves the Phase N goal on a throwaway repo with the real binary. Uses only local, no-agent commands plus one `run` with this repo's cloud-cli agents. If you'd rather not spend agent time, skip 5 and report it as skipped.

1. Reinstall Orrery:

   ```bash
   cargo install --path /srv/devshare/Orrery/crates/orrery-cli --locked
   ```

2. Save this repo's setup as a profile, then build a fresh project from it:

   ```bash
   cd /srv/devshare/Orrery && orrery profile save cloud-cli && orrery profile list
   ```

   ```bash
   mkdir -p ~/scratch/orrery-loop1 && cd ~/scratch/orrery-loop1 && git init && orrery init --name loop1 --profile cloud-cli
   ```

   ```bash
   orrery config show
   ```

   Expect: `validation_mode = "Agents"`, `max_step_retries = 2`, `gate_tools = ["hybrid:step_check"]`, the three stage assignments, `[hooks.on_step_start]` and `mcp_call_timeout_secs = 1800`; the catalog shows the three agents as `(global)`; `orrery role show orchestrator` lists `skills: ["hybrid-orchestrator", "repo-rules"]`.

3. Init refusal:

   ```bash
   orrery config set max_step_retries 5 && orrery init --name loop1; echo "exit $?" && orrery config show | grep max_step_retries
   ```

   Expect: `exit 1`, and `max_step_retries = 5` is still there.

4. TUI keeps CLI edits: open `orrery` (the TUI) in `~/scratch/orrery-loop1`. In another shell there, run:

   ```bash
   orrery config set limits.mcp_call_timeout_secs 900 && orrery config set max_step_retries 4
   ```

   In the TUI, toggle the mode to Loop and back to Run, then quit. Then:

   ```bash
   orrery config show | grep -E 'execution_mode|mcp_call_timeout_secs|max_step_retries|gate_tools'
   ```

   Expect: `execution_mode = "Run"`, `mcp_call_timeout_secs = 900`, `max_step_retries = 4`, and `gate_tools = ["hybrid:step_check"]`.

5. Run ↔ Loop on one project (optional; uses agents): in this repo, with a pending step that's safe to run, `orrery run --step <POSITION>`, then `orrery workflow mode loop --cycles 1 && orrery run`, then `orrery workflow mode run`. Expect: no `project.toml` change other than `execution_mode` / `cycle_count` (`git diff .orrery/project.toml`).

Report back: pass/fail for 2–5, and the `orrery config show` output from step 2.
