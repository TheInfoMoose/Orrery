# Orrery — Stepped Implementation Plan

This plan builds Orrery one Argon step at a time: `orrery-core` (Steps 1–26), `orrery-tui` (27–32) and `orrery-cli` (33–37). `ORRERY_TECHNICAL_SPECIFICATION.md` (the spec) is the design. Each step block below says which spec lines it implements, which files it may touch, which tests it must add, and the exact shell lines that gate it.

Restructured 2026-11-13 to the Nebulous plan format (roadmap tables, one-line gates, `ALLOWED_FILES` / `READ_ONLY_REFS` / `FORBIDDEN_SCOPE`, a fixed `HALT_INVARIANT`). Step numbers, scopes and test names are unchanged from the 2026-11-12 plan. The old text is kept in `STEPPED_IMPLEMENTATION_PLAN.pre-2026-11-13.md`.

---

## Step Execution Roadmap

**How to read this:** work goes strictly top to bottom through **Up Next**, one step at a time. A step moves to **Completed** only after its `POST_GATE` exits 0. Step numbers are permanent: spec §3.1 and §3.2 name steps by number, and Step 6's acceptance test counts the `### STEP ` headings in this file (37). Never add, remove or renumber a step heading without updating that test and the spec in the same change.

**Who runs what:**
- **Gemini** runs one step from its Argon block **in this file**, found by its heading (`### STEP <N>:`). It never works from a pasted copy. If a pasted prompt and this file disagree, this file wins and Gemini says so in its report instead of halting. Gemini never edits this file or the spec.
- **Claude** writes and refreshes the blocks, checks each finished step's diff against its `MUTATION_SPEC`, and moves the row to **Completed**. Around each handoff Claude runs `scripts/validate-step.sh start <N>` (runs the step's `PRE_GATE`, then snapshots the tree) and, after Gemini reports, `scripts/validate-step.sh check <N>` (fails on a commit, an edit to this file, any change outside `ALLOWED_FILES`, or a failing `POST_GATE`). Claude commits each step after `check` passes. Gemini never commits.
- **The operator** runs `sudo`, installs host packages and tools, and runs the live-backend tests.

### Up Next

| # | Step | Command | Scope | Target Files | Waits on |
| :---: | :--- | :---: | :--- | :--- | :--- |
| 1 | **STEP 31** | `/boost` | TUI widgets and rendering | `crates/orrery-tui/src/lib.rs`, `crates/orrery-tui/src/widgets.rs` (NEW) | — |
| 2 | **STEP 32** | `/goal` | TUI ↔ engine bridge, `run_tui` | `crates/orrery-tui/src/lib.rs`, `crates/orrery-tui/src/bridge.rs` (NEW) | — |
| 3 | **STEP 33** | `/goal` | CLI crate and argument parsing, `clap` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-cli/Cargo.toml` (NEW), `crates/orrery-cli/src/main.rs` (NEW), `crates/orrery-cli/src/args.rs` (NEW) | — |
| 4 | **STEP 34** | `/goal` | CLI project and workflow commands | `crates/orrery-cli/src/main.rs`, `crates/orrery-cli/src/commands.rs` (NEW) | — |
| 5 | **STEP 35** | `/goal` | CLI catalog and secret commands, `rpassword` | `Cargo.toml`, `Cargo.lock`, `crates/orrery-cli/Cargo.toml`, `crates/orrery-cli/src/main.rs`, `crates/orrery-cli/src/commands.rs` | — |
| 6 | **STEP 36** | `/boost` | CLI run, operator, status commands; TUI launch | `crates/orrery-cli/src/main.rs`, `crates/orrery-cli/src/commands.rs` | — |
| 7 | **STEP 37** | `/goal` | Release readiness: CLI smoke test, final checks | `crates/orrery-cli/tests/cli_smoke.rs` (NEW) | **[USER]** runs the live-backend tests afterwards (command in the Step 37 report). |

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
   - `McpServerConfig::validate_project_scope(&self) -> Result<()>`: a Stdio `command` containing `/` or `\` → `Error::Validation { field: "transport.command", .. }`; any `args` entry where `Path::new(a).is_absolute()` → `Error::Validation { field: "transport.args", .. }`.
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

```text
[STEP_ID]
STEP 32: bridge.rs — BridgeInput, Effects, step(), EngineController (config persistence, engine spawn, interrupt, operator), run_tui with terminal guard and panic hook

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-tui/src/lib.rs
  - crates/orrery-tui/src/bridge.rs
READ_ONLY_REFS:
  - ORRERY_TECHNICAL_SPECIFICATION.md:1106-1119 (§9.9 bridge block; binding)
  - ORRERY_TECHNICAL_SPECIFICATION.md:574-580 (§6.5 engine bridge)
  - crates/orrery-core/src/engine.rs (build_engine, Backends, RunOptions, WorkflowEngine::operator_sender)
FORBIDDEN_SCOPE:
  - Any file not listed in ALLOWED_FILES. The render loop never awaits engine work. Tests never open a terminal.

[PRE_GATE]
S=ORRERY_TECHNICAL_SPECIFICATION.md && cargo test --workspace --locked >/dev/null 2>&1 && test -f crates/orrery-tui/src/widgets.rs && test ! -e crates/orrery-tui/src/bridge.rs && grep -q '^tokio-util.workspace = true$' crates/orrery-tui/Cargo.toml && sed -n 574p $S | grep -q '^### 6.5 '

[MUTATION_SPEC]
1. lib.rs: `pub mod bridge;` and `pub use bridge::run_tui;`.
2. bridge.rs, the §9.9 bridge block:
   - `step` maps inputs to `App` calls and returns `Effects`. `redraw` is true for every input except a `Tick` with no change; `quit` = `app.should_quit`.
   - `EngineController::handle`:
     - config-edit actions → save via `ConfigStore::save_project`;
     - `IngestPlan` → parse with `ImplementationPlan::from_file`, set `plan_path`/`step_marker`, save;
     - `Run` → `build_engine(store, make_backends()?)`, then spawn `engine.run(RunOptions { wait_for_operator: true, ..Default }, cancel.child_token())` on a Tokio task, and forward its `EngineEvent`s and `LiveStreamHub` events into the app's channels;
     - `Interrupt` → `cancel()`;
     - `Approve`/`Reject` → `operator_sender().send(..)`.
   - `run_tui`:
     - enable raw mode, the alternate screen and mouse capture;
     - install a panic hook that restores the terminal, then calls the previous hook; a guard struct also restores on drop;
     - loop with `tokio::select!` over `crossterm::event::EventStream` (`futures_util::StreamExt::next`), the engine/stream channels and a 250 ms tick;
     - recompute `UiLayoutRegions` on each draw;
     - `SetMouseCapture` toggles crossterm mouse capture.
3. Tests (bridge::tests; no terminal; `EngineController` with mock `Backends`; `#[tokio::test]`):
   - `step_key_returns_actions`.
   - `step_engine_event_updates_app`.
   - `step_stream_event_appends`.
   - `run_action_starts_engine_status_running`.
   - `interrupt_cancels_and_saves_checkpoint`: the status returns to `Idle`, and the checkpoint step is `InProgress`.
   - `approve_reaches_operator_channel`: with the engine awaiting the operator, `Approve` makes the step `Passed`.
   - `config_edit_persists`: `SetStageCount(4)`, then `ConfigStore::load_project` shows 4 stages.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && grep -q '^pub use bridge::run_tui;$' crates/orrery-tui/src/lib.rs && test "$(cargo test -p orrery-tui --lib --locked -- --list 2>/dev/null | grep -cE '^bridge::tests::(step_key_returns_actions|step_engine_event_updates_app|step_stream_event_appends|run_action_starts_engine_status_running|interrupt_cancels_and_saves_checkpoint|approve_reaches_operator_channel|config_edit_persists): test$')" -eq 7

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

## Phase C — `orrery-cli` (Steps 33–37)

### STEP 33: CLI Crate & Argument Parsing

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. **Changed from the 2026-11-12 block:** `tokio-util` is added to the CLI crate's dependencies. `orrery run` cancels the engine on Ctrl+C through a `CancellationToken`. `main` prints the parsed command in this step only (Step 34 replaces it).

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
4. main.rs: `mod args;` and a `main` that parses `Cli` and prints `{:?}` of `into_command()`.
5. Tests (args::tests; `Cli::try_parse_from`):
   - `every_spec_row_parses`: one assertion per §7 row (table-driven `&[&[&str]]`).
   - `bare_orrery_is_tui_dot`.
   - `orrery_path_is_tui_path`.
   - `stage_count_out_of_range_rejected`: `workflow stages 1` and `16` fail; `2` and `15` parse.
   - `project_flag_is_global`: `orrery status --project ./x` and `orrery --project ./x status` both parse.
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
2. Tests (commands::tests; mock `Backends` via `make_backends`):
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
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! grep -q 'not implemented until Step' crates/orrery-cli/src/commands.rs && grep -q 'run_tui' crates/orrery-cli/src/commands.rs && test "$(cargo test -p orrery-cli --bin orrery --locked -- --list 2>/dev/null | grep -cE '^commands::tests::(run_mode_passes_one_step_exit_0|run_loop_cycles_two|escalation_exits_3_then_approve_resume_completes|reject_records_feedback|approve_without_awaiting_step_exits_1|reset_step_one_marks_pending|status_json_shape|logs_tail_shows_redacted_lines|tui_action_parity): test$')" -eq 9

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```

---

### STEP 37: Release Readiness Check

* **Recommended Command:** `/goal`

Scope note: written 2026-11-13. The smoke plan uses the product default marker `## Step ` (what `init` writes), not this file's `### STEP `.

```text
[STEP_ID]
STEP 37: tests/cli_smoke.rs — end-to-end CLI smoke test of the built binary; final dependency and live-test compile checks

[SCOPE_LOCK]
ALLOWED_FILES:
  - crates/orrery-cli/tests/cli_smoke.rs
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
2. Step report to the operator: the live-backend command
     cargo test --workspace --locked --features orrery-core/live-backends
   with `ORRERY_LIVE_MCP_CMD`, `ORRERY_LIVE_IMAGE`, `ORRERY_LIVE_CONTAINER`, `ORRERY_LIVE_LLM_URL`, `ORRERY_LIVE_LLM_MODEL` (§8). `live_keyring` needs a desktop session or keyutils.

[POST_GATE]
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo clippy --workspace --all-targets --features orrery-core/live-backends --locked -- -D warnings && cargo test --workspace --locked && cargo deny check && cargo llvm-cov --workspace --locked --fail-under-lines 80 && ! cargo tree --locked -i openssl-sys >/dev/null 2>&1 && ! cargo tree --locked -i zbus >/dev/null 2>&1 && cargo test -p orrery-cli --test cli_smoke --locked 2>&1 | grep -q 'test result: ok. 1 passed'

[HALT_INVARIANT]
Halt immediately after POST_GATE exits 0 and report verification status.
```
