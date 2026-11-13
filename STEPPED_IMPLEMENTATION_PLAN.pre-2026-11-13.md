# Orrery — Stepped Implementation Plan

> **Read first:** `ORRERY_TECHNICAL_SPECIFICATION.md` is the source of truth. **§9 (Interface Reference) holds the exact signatures and algorithms every step implements.** A step names what to build and how it is tested; §9 says what it looks like. If the two disagree, halt and report — the spec wins.
>
> Product intent and recorded decisions: `Orrery_Initial.md` → "Decisions".
>
> Rewritten 2026-11-12 from Step 4 on: dependencies pinned and verified (§3.1), every open interface fixed in §9, large steps split. Steps 1–3 are done and unchanged.

## Rules for every step
- Load the Rust skills before editing (required by `Orrery_Initial.md`).
- **Dependencies:** add exactly the lines the step gives (they come from spec §3.1). Do not look up versions or feature names. After editing manifests, run `cargo fetch` once — the only cargo command run without `--locked` — so `Cargo.lock` records the new crates. Never hand-edit `Cargo.lock`. A step that changes a manifest lists `Cargo.lock` in its scope.
- Every other cargo command except `cargo fmt` uses `--locked`.
- **Pre-gate:** only `cargo test --workspace --locked` plus the step's own checks. Don't re-run the previous step's full gate set.
- Run `cargo fmt --all` before the gates.
- No `#[ignore]`, no `#[allow(...)]`, no empty or assertion-free tests, no `.unwrap()`/`.expect()` outside tests.
- Fixtures use only reserved placeholders (spec §8). No absolute host paths.
- Live-backend tests are written but **not run** by the agent; the operator runs them.
- **Decide, don't halt,** on anything §9.0 calls a free choice (private helpers, extra derives, internal field layout, test helper functions). Mention such choices in one line of the step report.
- **Halt only** on: a failed gate, a needed change to a file outside `[SCOPE_LOCK]`, a §9 signature that doesn't compile as written, or a named test that can't pass as described. Report the exact error and the line of §9 involved.
- **Tests:** the listed tests are the minimum, each with real assertions. Use the given names. Add more only to reach the coverage floor.
- **Step report** (on finish or halt): gate results, files changed, free choices made, anything the operator must do.

---

## Step 1: Workspace Scaffold, Gates & Core Errors

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

## Step 2: Config Models & Validation

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

## Step 3: ConfigStore & Layered Loading

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

## Step 4: SecretValue, SecretStore & OS Keychain

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

## Step 5: SecretRedactor

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

## Step 6: Plan Step Parser

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-06: Plan Step Parser

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/plan.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root `[workspace.dependencies]`: `sha2 = "0.11.1"`. Crate `[dependencies]`: `sha2.workspace = true`. Run `cargo fetch`.
2. `lib.rs`: `pub mod plan;`.
3. `plan.rs`: implement §9.2 exactly (`PlanStep`, `ImplementationPlan::{parse, from_file, plan_sha256, step}`, `sha256_hex`, fence rules, title/body/preamble/number rules).
4. Tests:
   - `sha256_hex_known_vector` — `sha256_hex(b"abc")` == `"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"`.
   - `parses_preamble_and_steps` — preamble + 2 steps; titles, bodies, `number` 1 and 2.
   - `marker_inside_backtick_fence_ignored` and `marker_inside_tilde_fence_ignored`.
   - `longer_fence_contains_shorter_fence_line` — a 4-backtick fence containing a 3-backtick line and a marker line; still one step.
   - `unclosed_fence_errors_with_line` — `Error::PlanParse { line }` points at the opening fence line.
   - `empty_marker_rejected`, `zero_steps_rejected`.
   - `numbering_gap_rejected` — titles `1: a`, `3: b` → `PlanParse` at the second marker line.
   - `unnumbered_titles_use_index` — titles without numbers get `number = index + 1`.
   - `crlf_and_lf_hash_identical` — same text with `\r\n` and `\n` gives equal `sha256_hex` and `plan_sha256`.
   - `body_edit_changes_hash`.
   - `from_file_reads_and_sets_source_path` (tempfile) and `from_file_missing_is_io_error`.
   - `acceptance_fixture_counts_plan_steps` — `ImplementationPlan::parse(include_str!("../../../STEPPED_IMPLEMENTATION_PLAN.md"), "## Step ")` yields **37** steps, numbered 1..=37.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if the acceptance fixture count is not 37.
```

---

## Step 7: Agent Reports

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-07: Agent Reports

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/checkpoint.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `chrono = { version = "0.4.45", default-features = false, features = ["clock", "std", "serde"] }`. Crate: `chrono.workspace = true`. Run `cargo fetch`.
2. `lib.rs`: `pub mod checkpoint;`.
3. `checkpoint.rs`: implement the first §9.3 block only — `AgentReport`, `ReportStatus`, `SubtaskAllocation`, `MAX_SUMMARY_BYTES`, `validate_for_role` (rules 1–5 in order), `report_json_schema`, `StampedReport`. (Checkpoint state is Step 8.)
4. Tests (use a small `role(kind)` helper building a `RoleConfig`):
   - `orchestrator_statuses` — `Dispatch`/`StepPassed`/`Escalate` allowed; the other 4 rejected.
   - `implementer_statuses` — `Complete`/`Blocked` allowed; others rejected.
   - `validator_statuses` — `Confirmed`/`Denied` allowed; others rejected.
   - `summary_empty_or_too_long_rejected` — `"  "` rejected; 4096 bytes accepted; 4097 rejected.
   - `dispatch_requires_target_and_allocations` — missing `target_stage`, `target_stage = 0`, `target_stage = stage_count`, empty allocations: each rejected.
   - `dispatch_rejects_bad_write_scope` — `"../x"` rejected.
   - `non_dispatch_rejects_target_or_allocations`.
   - `feedback_required_for_denied_blocked_escalate`.
   - `unknown_field_rejected` — TOML with an extra key fails to deserialize.
   - `stamped_report_toml_round_trip`.
   - `json_schema_required_matches_struct` — schema `required` == `["status","summary"]`; schema property names == keys of a fully populated `AgentReport` serialized to JSON; `status.enum` has 7 entries; a JSON value with only `status` + `summary` deserializes into `AgentReport`, and one missing `summary` does not.

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

## Step 8: Workflow Checkpoint & Reconcile

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-08: Workflow Checkpoint & Reconcile

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/checkpoint.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `uuid = { version = "1.29.2", features = ["v4", "serde"] }`. Crate: `uuid.workspace = true`. Run `cargo fetch`.
2. `checkpoint.rs`: implement the second §9.3 block — `StepStatus`, `EnginePhase`, `OperatorDecision`, `HookResult`, `WorkflowCheckpoint` (with `new`, `reconcile`, `reset_from`), `ReconcileOutcome`, `CheckpointManager`. Writes use `config::atomic_write`.
3. Tests (temp dirs; plans built with `ImplementationPlan::parse`):
   - `new_checkpoint_all_pending`.
   - `state_save_load_round_trip` and `load_state_missing_is_none`.
   - `report_path_format` — `save_report` returns `.../step_001/round_02_stage_01_local-coder.toml`.
   - `load_reports_filters_and_sorts` — 3 reports over 2 rounds; `round: Some(1)` returns only round 1; `None` returns all in (round, stage, agent) order.
   - `delete_reports_removes_round_only`.
   - `reconcile_unchanged`.
   - `reconcile_accepts_later_edit` — step 3 edited while current is step 1 → `Updated { changed_steps: [2] .. }` and the stored hash changes.
   - `reconcile_accepts_appended_step`.
   - `reconcile_rejects_passed_step_edit` → `Error::Engine { step_number: 1 }`.
   - `reconcile_rejects_current_in_progress_edit`.
   - `reconcile_removed_pending_ok_removed_passed_err`.
   - `reset_from_marks_later_pending` — steps 1–3 `Passed`, `reset_from(2)` → `[Passed, Pending, Pending]`, `current_step_index == 1`.
   - `unknown_field_in_state_rejected`.

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

## Step 9: Live Stream Hub & Rotating Log

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-09: Live Stream Hub & Rotating Log

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/stream_log.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `tokio = { version = "1.54.1", features = ["sync"] }`. Crate: `tokio.workspace = true`. Run `cargo fetch`.
2. `lib.rs`: `pub mod stream_log;`.
3. `stream_log.rs`: implement §9.4 exactly (types, `LiveStreamHub`, `tail_log`, emit order, rotation, oversize-line rule).
4. Tests (temp dir, tiny caps such as `max_file_bytes = 512`, `max_files = 3`, `ring_capacity = 4`):
   - `ring_evicts_oldest` — 6 emits, `recent(10)` returns the last 4 in order.
   - `secret_redacted_in_ring_and_file` — registered secret never appears in `recent` or in any log file.
   - `rotation_order` — after enough emits, `.log`, `.log.1`, `.log.2` exist and `.log.2` holds the oldest surviving events.
   - `oldest_file_dropped` — `.log.3` never exists.
   - `total_bytes_never_exceed_cap` — 1,000 emits; after each, the sum of the three file sizes ≤ `512 * 3`.
   - `oversize_line_is_shortened` — one event with 2,000-char text; the written line ≤ 512 bytes.
   - `subscriber_receives_redacted_event` (`tokio::sync::broadcast` receiver `try_recv`, no runtime needed).
   - `emit_without_subscribers_is_ok`.
   - `tail_log_reads_across_rotated_files` — returns the last N lines oldest-first across `.log.2`, `.log.1`, `.log`.

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

## Step 10: MCP Gatekeeper, WriteScope & Mock Transport

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-10: MCP Gatekeeper, WriteScope & Mock Transport

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/mcp.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `async-trait = "0.1.92"`; change the tokio line's features to `["sync", "macros", "rt"]`. Crate: `async-trait.workspace = true`. Run `cargo fetch`.
2. `lib.rs`: `pub mod mcp;` and variant `#[error("MCP error on {server_id}")] Mcp { server_id: String, #[source] source: Box<dyn std::error::Error + Send + Sync> }`.
3. `mcp.rs`: implement from §9.5: `McpToolSchema`, `McpTransport`, `McpConnector`, `WriteScope`, `scopes_overlap`, `tool_result_is_error`, `tool_result_text`, `McpGatekeeper`, `MockMcpTransport`, `MockMcpConnector`. (Not `RmcpTransport`/`RmcpConnector` — Step 11.)
4. Tests (`#[tokio::test]` where async):
   - `exact_pattern_allows_only_that_tool`, `server_wildcard_allows_server_tools`, `star_star_not_honored`.
   - `disallowed_call_is_security_error_and_never_reaches_transport` — mock `calls()` is empty afterwards.
   - `unregistered_server_is_security_error`.
   - `write_tool_path_outside_scope_rejected` — scope `["src"]`, path `"docs/a.md"` → `Error::Security`, zero calls.
   - `write_tool_path_inside_scope_allowed` — `"src/lib.rs"` reaches the mock.
   - `write_tool_dotdot_rejected` — `"src/../etc/x"` rejected.
   - `write_tool_non_string_path_rejected`.
   - `non_write_tool_ignores_scope`.
   - `anywhere_scope_contains_relative_paths` and `anywhere_scope_rejects_absolute`.
   - `scopes_overlap_cases` — `("src","src/lib.rs")` true, `("src","src2")` false, `("./src/","src")` true.
   - `tools_for_role_filters_catalog` — after `refresh_catalog`, only allowed tools are returned.
   - `tool_result_helpers` — `isError: true` detected; text parts joined with `"\n"`.
   - `failing_tool_returns_mcp_error`.
   - `mock_connector_unknown_server_errors`.

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

## Step 11: MCP Transport via `rmcp`

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-11: MCP Transport via `rmcp`

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- deny.toml
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/mcp.rs
- crates/orrery-core/tests/live_mcp.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root:
     reqwest = { version = "0.13.5", default-features = false, features = ["rustls", "json", "stream"] }
     rmcp = { version = "3.9.0", default-features = false, features = ["client", "transport-child-process", "transport-streamable-http-client-reqwest", "reqwest"] }
   and change tokio features to `["sync", "macros", "rt", "process", "io-util", "time"]`. Crate: `reqwest.workspace = true`, `rmcp.workspace = true`. Crate `[[test]] name = "live_mcp"`, `required-features = ["live-backends"]`. Run `cargo fetch`.
2. `deny.toml`: append
     [graph]
     targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
   (spec §8; without it `webpki-root-certs`, Android-only, fails the license check).
3. `lib.rs`: variant `#[error("HTTP error")] Http(#[source] reqwest::Error)`.
4. `mcp.rs`: implement §9.5 `RmcpTransport` (+ `McpTransport` impl) and `RmcpConnector` exactly as the `RmcpTransport::connect` bullet says.
5. `tests/live_mcp.rs`: `live_stdio_list_and_call` — read `ORRERY_LIVE_MCP_CMD` (split on whitespace: command + args; missing var → `panic!` with a clear message); connect; `list_tools` non-empty; call the first tool with `{}` and assert an `Ok` or an MCP-level error result (not a transport error).
6. Unit tests:
   - `missing_http_secret_is_keychain_error_before_network` — `StreamableHttp` at `https://mcp.example.com/mcp` with a keychain pair and an empty `InMemorySecretStore` → `Error::Keychain`.
   - `stdio_missing_command_errors` — command `"orrery-test-no-such-binary"` → `Err` (any variant), no panic.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if `cargo tree --locked -i openssl-sys` finds anything.
```

---

## Step 12: Inference Server Config (Attach to Existing Servers)

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-12: Inference Server Config (Attach to Existing Servers)

[SCOPE_LOCK]
- crates/orrery-core/src/config.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `config.rs`: replace `InferenceConfig` with the §9.6a shape — `InferenceMode`, `InferenceLifecycle`, `chat_url`, `validate`, `default_true`, `default_startup_timeout` (300). Delete `InferenceGpu` (and its private repr types) and `UnloadStrategy`; Orrery no longer sets images, ports, GPUs, memory or volumes for model servers. `InferenceFile` and `ProjectConfig.inference` stay. `Compose.file` may be absolute or relative (it's the operator's own file), but fixtures use relative placeholders only.
2. Rewrite every existing inference test and TOML fixture in `config.rs` to the new shape. Use the three §4.3 examples as fixtures. No fixture or test may mention any server product other than llama.cpp.
3. Tests:
   - `inference_defaults` — TOML with only `inference_id` + `endpoint_url` → `mode == Single`, `lifecycle == External`, `unload_after_stage == true`, `startup_timeout_secs == 300`.
   - `inference_spec_examples_round_trip` — the three §4.3 examples parse, validate and round-trip.
   - `inference_chat_url_trims_slash` — `"http://127.0.0.1:8080/"` → `"http://127.0.0.1:8080/v1/chat/completions"`.
   - `inference_rejects_bad_endpoint` — `"127.0.0.1:8080"` and `"ftp://x"` rejected.
   - `inference_rejects_empty_container_and_compose` — `Container { name: "" }`, `Compose { file: "" }` rejected.
   - `inference_rejects_removed_fields` — TOML containing `image`, `gpu`, `host_port` or `models_volume` fails (`deny_unknown_fields`).
   - `inference_rejects_embedded_secret_in_endpoint` — endpoint containing `sk-ABCDEFGH12345678`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if `grep -n "InferenceGpu\|UnloadStrategy\|models_volume\|keep_alive" crates/orrery-core/src/config.rs` finds anything.
```

---

## Step 13: Container & Inference Types, Mocks & Pure Helpers

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-13: Container & Inference Types, Mocks & Pure Helpers

[SCOPE_LOCK]
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/docker.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `lib.rs`: `pub mod docker;`.
2. `docker.rs`, from §9.6: `EnvValue`, `MountSpec`, `ContainerState`, `ExitStatus`, `ContainerSpec`, `ContainerBackend`, `ComposeRunner`, `InferenceControl`, `InferenceCall`, `MockInferenceControl`, `unload_request`, `MockCall`, `MockRunScript`, `MockContainerBackend`, `cli_container_name`, `agent_io_dir`, `effective_concurrency`, `mounts_for`. (Not `DockerComposeCli`, `DockerInferenceControl`, `ResourceManager` or `BollardContainerBackend` yet.)
   `MockContainerBackend`: `start` of a name not added → the §9.6 `Error::Validation`; `start` sets `Running`; `inspect` of an unknown name → `Missing`; `stop` → `Exited { code: 0 }`; `remove` → `Missing`; `run_to_completion` sleeps the delay (if set), calls the script, feeds each output line to `on_output`, records `RunToCompletion`, returns `ExitStatus { code }`.
   `MockInferenceControl`: `is_running` reads the set; `start` adds, `stop` removes; `wait_ready`/`unload` record and return `Ok`; every call is recorded.
3. Tests:
   - `concurrency_non_writer_uses_cap` — max 4, 2 agents → 2; max 2, 5 agents → 2; 0 agents → 1.
   - `concurrency_writer_without_scopes_is_one`.
   - `concurrency_writer_disjoint_scopes` — `["src/a"]` and `["src/b"]` → cap.
   - `concurrency_writer_overlapping_scopes_is_one` — `["src"]` vs `["src/lib.rs"]`.
   - `concurrency_writer_with_rw_cli_is_one`.
   - `mounts_local_and_cloud_empty`.
   - `mounts_cli_readonly`, `mounts_cli_readwrite`, `mounts_cli_none_has_no_project_mount`.
   - `mounts_never_bind_orrery_dir` — for every mount mode, no `Bind.host` equals `<root>/.orrery`.
   - `unload_request_router_only` — `Router` → `("http://127.0.0.1:8081/models/unload", {"model": m})`; `Single` → `None`.
   - `cli_container_name_format`.
   - `mock_start_unknown_container_fails` and `mock_records_calls_and_runs_script` (`#[tokio::test]`).

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

## Step 14: ResourceManager & Inference Control

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-14: ResourceManager & Inference Control

[SCOPE_LOCK]
- crates/orrery-core/src/docker.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. From §9.6: `DockerComposeCli` (+ `ComposeRunner`), `DockerInferenceControl` (+ `new`, `InferenceControl`), `ResourceManager` (`new`, `begin_stage` steps 1–5, `end_stage`, `shutdown`), `StageLease` (`acquire_slot`, `concurrency`, `Drop`), `SlotPermit`.
   `DockerComposeCli` builds its argv with a private pure fn `compose_args(file, service, op) -> Vec<String>` so tests can check it without running Docker.
2. Tests (`#[tokio::test]`, `MockContainerBackend` + `MockInferenceControl`, temp project root, inference configs from the §4.3 examples):
   - `second_begin_stage_fails_while_open`, `begin_after_end_succeeds`, `dropped_lease_does_not_block_next_stage`.
   - `stopped_server_started_and_waited` — not running → `Start` then `WaitReady`.
   - `running_server_not_started_or_stopped` — `set_running(true)` first; no `Start`; `shutdown` records no `Stop`.
   - `external_server_never_started` — use `DockerInferenceControl` with the mock backend and a mock `ComposeRunner`: `External` → no container or compose calls.
   - `same_server_next_stage_not_restarted` — two stages on `coder-14b` → one `Start`, no `Stop`.
   - `single_mode_swapped_between_stages` — stage A uses `coder-14b`, stage B uses a different `Single` server → B's `begin_stage` records `Stop(coder-14b)` before `Start(other)`.
   - `router_unloads_models_next_stage_drops` — stage A uses router model `m1`, stage B uses router model `m2` → `Unload(router, m1)` at B's `begin_stage`; `m2` not unloaded.
   - `unload_after_stage_false_keeps_server` — no `Stop`/`Unload` between stages.
   - `end_stage_removes_cli_containers_and_agent_io` — `Remove(cli_container_name(..))` recorded; `.orrery/agent_io/<run_id>` gone.
   - `unknown_inference_id_is_validation_error`.
   - `slots_limit_parallelism` — concurrency 1: a second `acquire_slot` doesn't complete while the first permit is held (`tokio::time::timeout` of 50 ms elapses).
   - `shutdown_unloads_router_and_stops_only_started`.
   - `compose_args_shapes` — up: `["compose","-f",F,"up","-d",S]`; down without service: `[...,"down"]`; with service: `[...,"stop",S]`; ps: `[...,"ps","--status","running","-q",S]`.

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

## Step 15: Docker Backend via `bollard`

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-15: Docker Backend via `bollard`

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/docker.rs
- crates/orrery-core/tests/live_docker.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `bollard = "0.21.1"`, `futures-util = "0.3.34"`. Crate: both `.workspace = true`; `[[test]] name = "live_docker"`, `required-features = ["live-backends"]`. Run `cargo fetch`.
2. `lib.rs`: variant `#[error("docker error")] Docker(#[source] bollard::errors::Error)`.
3. `docker.rs`: `BollardContainerBackend::connect`, `create_body` (§9.6 bullet), and `ContainerBackend for BollardContainerBackend`:
   - `inspect`: `inspect_container`; 404 → `Missing`; `state.running` → `Running`; `state.status == created` → `Created`; else `Exited { code: state.exit_code }`.
   - `start`: `inspect` → `Missing` → the §9.6 `Error::Validation`; `Running` → `Ok`; else `start_container`. Never creates a container.
   - `stop` / `remove` (`force: true`): 404 → `Ok(())`.
   - `run_to_completion` (AgentCli): `create_container` with `create_body`, `attach_container` (stdin, stdout, stderr, stream), `start_container`, write all of `stdin` to the attach input then shut it down, read output with `futures_util::StreamExt`, split into lines for `on_output`, then `wait_container` → `ExitStatus { code: status_code }`.
   Use the `bollard::query_parameters::*OptionsBuilder` types and `bollard::models::{ContainerCreateBody, HostConfig}`.
4. `tests/live_docker.rs` (missing env var → `panic!` with a clear message):
   - `live_start_stop_existing_container` — container named by `ORRERY_LIVE_CONTAINER` (an existing container the operator picks; stopped is fine): `start`, `inspect` == `Running`, `stop`, `inspect` is `Exited`. It is never removed.
   - `live_run_to_completion_echoes_stdin` — image `ORRERY_LIVE_IMAGE`, `cmd ["cat"]`, stdin `"hello\n"`, output contains `"hello"`, exit 0, then `remove`.
5. Unit tests (pure, no Docker):
   - `create_body_maps_image_cmd_env_memory`.
   - `create_body_binds_readonly_with_ro_suffix`.
   - `create_body_sets_no_gpu_or_ports` — `device_requests`, `devices`, `port_bindings` and `exposed_ports` are all `None`.
   - `secret_env_only_in_create_body` — `format!("{spec:?}")` doesn't contain the secret value; `create_body` env does.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if `cargo tree --locked -i openssl-sys` finds anything.
```

---

## Step 16: Prompt Builders, AgentDriver & Mock Driver

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-16: Prompt Builders, AgentDriver & Mock Driver

[SCOPE_LOCK]
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/llm.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `lib.rs`: `pub mod llm;`.
2. `llm.rs`, first §9.7 block: `AgentInvocation` (derive `Clone`), `StageSummary`, `SUBMIT_REPORT_TOOL`, `CLI_REPORT_PATH`, `build_system_prompt`, `build_user_prompt`, `build_orchestrator_prompt` (text exactly per the §9.7 prompt bullets), `parse_mapped_tool_name`, `AgentDriver` (§5.9 signature), `MockAgentDriver`.
3. Tests:
   - `user_prompt_contains_step_allocation_prior_and_notes`.
   - `user_prompt_omits_empty_sections`.
   - `user_prompt_never_contains_stream_text` — emit an event with text `"STREAM-SENTINEL-7f3a"` to a `LiveStreamHub`; build every prompt kind for the same step; none contains the sentinel.
   - `system_prompt_chat_backends_mention_submit_report`.
   - `system_prompt_cli_mentions_report_path_and_schema`.
   - `orchestrator_prompt_lists_stages`.
   - `mapped_name_round_trip` — `("filesystem","read_file")` → `"filesystem__read_file"` → back.
   - `mapped_name_rejects_invalid` — `"nodelimiter"`, `"__tool"`, `"server__"`, 65-char name.
   - `mock_driver_returns_scripted_reports_in_order` and `mock_driver_exhausted_is_agent_error` (`#[tokio::test]`).
   - `mock_driver_tracks_max_in_flight` — two concurrent `run_turn` calls with a 50 ms delay → `max_in_flight() == 2`.

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

## Step 17: HTTP Chat — Request Building & Stream Parsing

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-17: HTTP Chat — Request Building & Stream Parsing

[SCOPE_LOCK]
- crates/orrery-core/src/llm.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. From §9.7 second block, pure parts only: `endpoint_for`, `build_chat_request`, `ToolCallDelta`, `ChatChunk`, `SseParser`, `SseItem`, `AssembledToolCall`, `ToolCallAccumulator`. No network code in this step.
2. Tests (inline fixture strings):
   - `endpoint_local_model_uses_chat_url`, `endpoint_cloud_uses_url`, `endpoint_cli_rejected`, `endpoint_unknown_inference_rejected`.
   - `request_includes_tools_and_submit_report` — tool names are `server__tool`; last tool is `submit_report` with `report_json_schema()` as parameters; `stream: true`.
   - `sse_content_chunk`.
   - `sse_reasoning_content_and_reasoning_keys` — both spellings map to `reasoning`.
   - `sse_split_across_pushes` — one `data:` line split over two `push` calls yields one chunk.
   - `sse_done_marker`.
   - `sse_ignores_comments_and_blank_lines` — lines starting with `:`.
   - `sse_bad_json_is_json_error`.
   - `accumulator_joins_argument_fragments` — 3 deltas for index 0 → one call with full arguments.
   - `accumulator_keeps_parallel_calls_ordered` — indices 1 and 0 → output ordered 0, 1.

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

## Step 18: HttpChatDriver Turn Loop

**Recommended Slash Command:** `/boost`

```text
[STEP_ID]
STEP-18: HttpChatDriver Turn Loop

[SCOPE_LOCK]
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/llm.rs
- crates/orrery-core/tests/live_llm.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Crate: `[[test]] name = "live_llm"`, `required-features = ["live-backends"]`. (No new crates.)
2. `llm.rs`: `HttpChatDriver` (`new`, `with_max_tool_calls`) and `AgentDriver for HttpChatDriver` per the §9.7 "Turn loop" bullet. Factor the per-response handling into a private `fn handle_response(state: &mut TurnState, items: Vec<SseItem>, ...) -> Result<TurnStep>` (or similar) so the unit tests below drive it with fixture `SseItem`s and no network.
3. `tests/live_llm.rs`: `live_single_turn` — `LocalModel`-style agent pointed at `ORRERY_LIVE_LLM_URL` with model `ORRERY_LIVE_LLM_MODEL` (missing → `panic!` with message); empty gatekeeper; assert `run_turn` returns `Ok` or `Error::Report` (model didn't comply) — not `Http`/`Agent`.
4. Unit tests (fixture-driven, no network):
   - `submit_report_ends_turn`.
   - `invalid_report_reprompted_once_then_error` — first invalid → a `tool` message with the error is queued; second invalid → `Error::Report`.
   - `no_tool_call_reprompted_once_then_error`.
   - `disallowed_tool_returns_error_to_model_and_emits_event` — the `tool` message starts with `"error:"`; a `ToolCall` event containing `"denied"` is in `stream.recent(..)`; mock transport has zero calls.
   - `allowed_tool_result_returned_to_model`.
   - `tool_call_cap_enforced` — `with_max_tool_calls(2)`, 3 calls → `Error::Agent`.
   - `bearer_only_for_cloud_api` — the private header builder returns `Some("Bearer …")` for `CloudApi` (secret from `InMemorySecretStore`) and `None` for `LocalModel`.
   - `cloud_missing_secret_is_keychain_error`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if a secret is stored anywhere except inside a `SecretValue`.
```

---

## Step 19: CliContainerDriver

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-19: CliContainerDriver

[SCOPE_LOCK]
- crates/orrery-core/src/llm.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `CliContainerDriver::new` and `AgentDriver for CliContainerDriver` per the §9.7 `run_turn` bullet.
2. Tests (`#[tokio::test]`, `MockContainerBackend` with a run script that writes `<host out mount>/report.toml` — find it in `spec.mounts` by `container == "/orrery/out"`):
   - `valid_report_accepted` — also `in/prompt.md` exists and stdin length equals the prompt length.
   - `output_lines_streamed` — script output lines appear as `Output` events.
   - `missing_report_reprompted_then_error` — 2 `RunToCompletion` calls, then `Error::Report`.
   - `invalid_then_valid_report_accepted` — first run writes bad TOML, second valid → `Ok`; second prompt contains `"Previous report was invalid"`.
   - `timeout_is_agent_error_and_removes_container` — `timeout_secs = 1`, mock delay 2 s → `Error::Agent`, a `Remove` call recorded.
   - `secret_env_injected_as_secret` — recorded `secret_env == ["ANTHROPIC_API_KEY"]`, `plain_env` doesn't include it.
   - `missing_secret_fails_before_run` — empty secret store → `Error::Keychain`, zero `RunToCompletion` calls.

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

## Step 20: Engine Types & Pure Rules

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-20: Engine Types & Pure Rules

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-core/Cargo.toml
- crates/orrery-core/src/lib.rs
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `tokio-util = "0.7.20"`. Crate: `tokio-util.workspace = true`. Run `cargo fetch`.
2. `lib.rs`: `pub mod engine;`.
3. `engine.rs`, from §9.8: `DriverSet`, `EngineContext`, `RunOptions`, `RunOutcome`, `EngineEvent`, `DispatchKind`, `StageTurnRecord`, `check_dispatch`, `agents_pass_rule`, and `WorkflowEngine` with `new`, `subscribe`, `operator_sender` only. `run` exists with the §9.8 signature and returns `Err(Error::Engine { step_number: 0, message: "not implemented until Step 21".into() })` — this stub is allowed in this step only and is replaced in Step 21.
   Add a `#[cfg(test)]` helper `fn test_context(dir: &Path, stages: &[(RoleKind, &[&str])]) -> EngineContext` building roles, agents (all `LocalModel`), a 3-step plan, mocks and a temp `.orrery/`; later engine steps reuse it.
4. Tests:
   - `dispatch_forward_from_start` — `None`, target 1 → `Forward`; target 2 → error.
   - `dispatch_forward_next` — last 1, target 2 → `Forward`.
   - `dispatch_retry_to_earlier_implementer` — kinds `[O, I, V]`, last 2, target 1 → `Retry`.
   - `dispatch_to_orchestrator_rejected`, `dispatch_retry_to_validator_rejected`, `dispatch_out_of_range_rejected`.
   - `pass_rule_all_validators_confirmed`.
   - `pass_rule_one_denied_fails`.
   - `pass_rule_missing_validator_report_fails`.
   - `pass_rule_implementer_after_validator_fails`.
   - `pass_rule_gate_failed_fails`.
   - `pass_rule_no_validator_stage_fails`.
   - `new_rejects_config_invalid_for_run` — a stage with zero agents → `Err`.

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

## Step 21: Engine Core Loop, Run & Loop Modes

**Recommended Slash Command:** `/boost`

```text
[STEP_ID]
STEP-21: Engine Core Loop, Run & Loop Modes

[SCOPE_LOCK]
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Replace the `run` stub. Flow for each plan step (spec §5.10.1, §5.10.5, `Agents` mode only in this step):
   a. Load or create the checkpoint (`load_state` / `WorkflowCheckpoint::new`); pick the first `Pending` step (or `start_step`). None → `NoPendingSteps`.
   b. Mark `InProgress`, emit `StepStatusChanged`.
   c. Round loop: `round += 1`, emit `RoundStarted`, save state. Orchestrator turn: `build_orchestrator_prompt` with this step's saved reports and the current notes; driver chosen by backend via `DriverSet`; stamp + `save_report`; emit `ReportSaved` stream event.
   d. `Dispatch` → `check_dispatch` → stage turn: `resources.begin_stage` (store the lease in `current_lease`), run each allocated agent **one at a time** with `build_user_prompt`, `acquire_slot` per agent, stamp + save each report, `end_stage`, record a `StageTurnRecord`, save state. Allocation whose `target_agent_id` is not in the stage → protocol violation.
   e. `StepPassed` → `agents_pass_rule(history, gate_ok = true)` (gates are Step 22). Holds → `Passed`, `steps_passed_this_run += 1`, save, emit. Fails → protocol violation.
   f. Protocol violation (bad dispatch, bad allocation, rule fails, driver `Error::Report`): add a note `"protocol violation: {msg}"` and redo the orchestrator turn; a second consecutive violation → `AwaitingOperator` handling is Step 22, so in this step return `Err(Error::Engine{..})`.
   g. Mode: `Run` → stop after one `Passed` (`Completed { steps_passed: 1 }`). `Loop` → continue until `cycles > 0 && steps_passed_this_run == cycles` or no `Pending` step.
   `opts.mode`/`opts.cycles` override the project values when `Some`.
2. Tests (`test_context` + `MockAgentDriver`; stages `[Orchestrator, Implementer, Validator]`):
   - `run_mode_passes_one_step_and_stops` — step 2 still `Pending`.
   - `loop_cycles_three_passes_three` — use a 4-step plan.
   - `loop_cycles_zero_passes_all`.
   - `orchestrator_runs_between_every_stage_turn` — the invocation sequence's stage indices alternate `0, s, 0, s, …`.
   - `step_passed_while_validator_denied_is_violation` — orchestrator says `StepPassed` after a `Denied` → a note containing `"protocol violation"` reaches the next orchestrator prompt.
   - `dispatch_skipping_stage_is_violation`.
   - `checkpoint_saved_after_every_round` — `load_state` after the run shows `round` == number of orchestrator turns.
   - `reports_stamped_with_step_hash_and_round`.
   - `stage_lease_closed_after_run` — a fresh `begin_stage` on the same `ResourceManager` succeeds.

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

## Step 22: Engine Gates, Retries, Escalation & Operator

**Recommended Slash Command:** `/boost`

```text
[STEP_ID]
STEP-22: Engine Gates, Retries, Escalation & Operator

[SCOPE_LOCK]
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Gates per §9.8 (role `"__orrery_gate"`, run at pass-check time, `GateResult` events); `gate_ok` now comes from the gate run (empty `gate_tools` → `true`).
2. `GateAndOperator` mode (`effective_validation_mode`): `StepPassed` → gate → pass → awaiting operator; fail → note with the gate output, back to the orchestrator.
3. Retries (§5.10.1): `DispatchKind::Retry` after a `Denied` report or a failed gate increments `retry_count`; `retry_count > max_step_retries` → awaiting operator. `Escalate` → awaiting operator. Second consecutive protocol violation → awaiting operator (replaces Step 21's `Err`).
4. Awaiting operator per §9.8: set phase/status `AwaitingOperator`, `awaiting_reason`, emit `AwaitingOperator`, save. `wait_for_operator = true` → await `operator_rx`; `false` → return `RunOutcome::AwaitingOperator`. On entry to `run`, a stored `operator_decision` is applied and cleared. Decision effects exactly as the §9.8 "Operator decisions" bullet.
5. Tests:
   - `deny_revise_confirm_passes` — V `Denied` → O dispatches to stage 1 (retry 1) → I `Complete` → V `Confirmed` → O `StepPassed` → `Passed`, `retry_count == 1`.
   - `retries_exhausted_awaits_operator` — `max_step_retries = 1`, two denials → `AwaitingOperator`, checkpoint saved with that status.
   - `escalate_awaits_operator_headless` — `wait_for_operator = false` → `RunOutcome::AwaitingOperator`.
   - `stored_approve_resumes_after_escalation` — set `operator_decision = Approve` in the checkpoint, run again → orchestrator turn runs, `retry_count == 0`.
   - `gate_failure_blocks_pass` — `MockMcpTransport::with_failing_tool`; `StepPassed` doesn't pass; next orchestrator prompt has a gate note.
   - `gate_and_operator_approve_passes` — 2-stage workflow `[O, I]`, `wait_for_operator = true`, send `Approve` on `operator_sender()` → `Passed`.
   - `gate_and_operator_reject_feeds_feedback` — `Reject { feedback: "fix the README" }` → next orchestrator prompt contains it, `retry_count == 1`.
   - `protocol_violation_twice_awaits_operator`.

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

## Step 23: Engine Lifecycle Hooks

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-23: Engine Lifecycle Hooks

[SCOPE_LOCK]
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Hooks per spec §5.10.4 and §9.8: `on_step_start` before the first round of a step; `on_step_retry` before each retry dispatch; `on_step_pass` after `Passed`; `on_halt` when entering `AwaitingOperator` and on cancel (Step 25 calls it). Role `"__orrery_hooks"`, args object per §5.10.4. Store a `HookResult` (output redacted via `stream.redactor()`, truncated to 4096 bytes at a char boundary). `Halt` failure → awaiting operator with reason `"hook {event} failed"`; `Warn` failure → `SystemNotice` event and continue. Phase `Hook` while a hook runs.
2. Tests:
   - `hook_args_contain_orrery_object` — mock call args have all 6 keys and `event == "step_start"`.
   - `step_retry_hook_runs_before_each_retry` — two retries → two `on_step_retry` calls, each before the retried stage's invocation.
   - `step_pass_hook_runs_after_pass`.
   - `halt_hook_failure_awaits_operator`.
   - `warn_hook_failure_continues_with_notice`.
   - `hook_role_can_call_only_hook_tools` — a hook configured as `git:commit` while the gatekeeper also has `git:push`: build the hook role and assert `McpGatekeeper::is_tool_allowed(&role, "git", "push") == false`.
   - `hook_output_redacted_and_truncated` — mock result text with a registered secret and 5,000 chars → stored output has `[REDACTED]` and ≤ 4096 bytes.

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

## Step 24: Engine Stage Concurrency & Validator Broadcast

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-24: Engine Stage Concurrency & Validator Broadcast

[SCOPE_LOCK]
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `futures-util` is already a dependency of `orrery-core` (Step 15). No manifest changes.
2. Stage turn runs agents concurrently: one future per agent, each `acquire_slot().await` then `run_turn`, joined with `futures_util::future::join_all`. Concurrency comes from the lease (`effective_concurrency`, spec §5.10.3). `writer` = `RoleConfig::is_writer(&servers, &stage_agents)`.
3. Each agent's `write_scope` = `WriteScope::new(allocation.write_scope)`; empty → `WriteScope::anywhere()`.
4. Validator broadcast (§5.10.3): in a `Validator` stage, every agent in the stage runs. An agent with its own allocation uses it; others get the first allocation's instructions. `StageTurnRecord.expected_agents` = all stage agents.
5. Tests:
   - `non_writer_stage_runs_in_parallel` — 2 implementers, `max_concurrent_agents = 2`, mock delay 50 ms → `max_in_flight() == 2`.
   - `writer_stage_serialized_without_scopes` → `max_in_flight() == 1`.
   - `writer_stage_parallel_with_disjoint_scopes` → `2`.
   - `writer_stage_serialized_with_overlapping_scopes` — `src` and `src/lib.rs` → `1`.
   - `validator_broadcast_runs_every_validator` — 2 validators, 1 allocation → both invoked with the same instructions.
   - `pass_requires_every_broadcast_validator` — one of two validators `Denied` → not passed.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if two stages' agents can be in flight at the same time.
```

---

## Step 25: Engine Interrupt, Resume & Plan Reconcile

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-25: Engine Interrupt, Resume & Plan Reconcile

[SCOPE_LOCK]
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Cancel per the §9.8 "Cancel" bullet: race every await in `run` with `cancel.cancelled()` (`tokio::select!`); on cancel end the stored lease, save the checkpoint (`InProgress`, phase `OrchestratorTurn`, same `round`), run `on_halt`, return `RunOutcome::Interrupted`.
2. Resume: a step found `InProgress` at start → `delete_reports(step, round)`, restart that round from its orchestrator turn (don't increment `round` for it).
3. On start, an existing checkpoint is reconciled against the plan (`reconcile`); `Err` is returned before any agent runs.
4. `start_step = Some(n)`: step `n` must be `Pending` (else `Error::Engine { step_number: n, .. }`); it becomes the current step.
5. Tests:
   - `cancel_mid_stage_saves_checkpoint_and_closes_lease` — mock delay 500 ms, cancel after 50 ms → `Interrupted`; checkpoint step `InProgress`; a new `begin_stage` succeeds.
   - `cancel_runs_halt_hook`.
   - `resume_restarts_interrupted_round_orchestrator` — after the cancel above, `run` again: the first invocation is the orchestrator with the same `round`; stale reports of that round are gone.
   - `edited_passed_step_blocks_start` — pass step 1, edit its body, new engine `run` → `Err(Error::Engine { step_number: 1, .. })` and zero driver invocations.
   - `appended_step_accepted_on_start`.
   - `start_step_must_be_pending`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if cancel can leave a stage lease open.
```

---

## Step 26: Engine Assembly (`Backends`, `build_engine`)

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-26: Engine Assembly (`Backends`, `build_engine`)

[SCOPE_LOCK]
- crates/orrery-core/src/engine.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `Backends` and `Backends::real` (§9.8): `OsKeyringStore::new()?`, `BollardContainerBackend::connect(None)?`, `DockerInferenceControl::new(containers.clone(), Arc::new(DockerComposeCli))`, `RmcpConnector`, `DriverSet { chat: HttpChatDriver::new(..), cli: CliContainerDriver::new(..) }`. Inference list comes from `ConfigStore::load_inference`, so `real` takes `project_root` and builds a `ConfigStore` itself.
2. `build_engine(store, backends)`: load project, roles (+ `load_role_rules` for each), agents, MCP servers, inference; `ImplementationPlan::from_file(resolve_safe_path(plan_path), step_marker)`; `SecretRedactor` with every secret the run will use registered (each agent keychain pair and each HTTP MCP pair that `secrets.get` returns `Ok` for); `LiveStreamHub::new(<.orrery>/logs, Default, redactor)`; gatekeeper with every server `connector.connect`ed + `refresh_catalog`; `ResourceManager::new(..)`; `CheckpointManager::new(<.orrery>/checkpoints)`; `WorkflowEngine::new(ctx)`.
3. Tests (temp project from `ConfigStore::init_project`, a 2-step plan file, `MockMcpConnector`, mocks for the rest):
   - `build_engine_from_initialized_project` — after assigning one agent per stage, `build_engine` succeeds and `run` with scripted mocks passes step 1.
   - `build_engine_registers_secrets_with_redactor` — a `CloudApi` agent's secret emitted in a stream event is stored as `[REDACTED]`.
   - `build_engine_missing_plan_is_io_error`.
   - `build_engine_unknown_mcp_server_errors` — role allows a server the connector doesn't know → `Err`.

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

## Step 27: TUI Crate, Theme & Banner

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-27: TUI Crate, Theme & Banner

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-tui/Cargo.toml
- crates/orrery-tui/src/lib.rs
- crates/orrery-tui/src/theme.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: add `"crates/orrery-tui"` to `members`; `[workspace.dependencies]`: `ratatui = "0.30.2"`, `crossterm = { version = "0.29.0", features = ["event-stream"] }`, `orrery-core = { path = "crates/orrery-core" }`.
2. `crates/orrery-tui/Cargo.toml`: `name = "orrery-tui"`, workspace package fields as in `orrery-core`, `[dependencies]` `orrery-core`, `ratatui`, `crossterm`, `tokio`, `futures-util` (all `.workspace = true`), `[dev-dependencies] tempfile.workspace = true`, `[lints] workspace = true`. Run `cargo fetch`.
3. `lib.rs`: `pub mod theme;` only.
4. `theme.rs` per spec §6.3: the 7 colour constants as `ratatui::style::Color::Rgb(..)` from the table; `BANNER: [&str; 9]` copied byte-for-byte from the spec's `text` block as raw strings (`r"..."`); `COMPACT_HEADER` from the compact block; `banner_line(row)`; `base_block`, `focused_block`, `alert_block`, `muted`, `highlight`.
5. Tests:
   - `palette_rgb_values` — each constant equals its §6.3 RGB.
   - `banner_shape` — 9 rows, ASCII only, each ≤ 72 chars.
   - `banner_matches_spec_block` — `include_str!("../../../ORRERY_TECHNICAL_SPECIFICATION.md")`; find the line starting `Full banner (9 rows`, then the next line that is exactly "```text"; the 9 lines after it equal `BANNER` exactly.
   - `banner_letter_cell_is_primary` — row 3, column 15 (`_`, 1-based) → `AMBER_PRIMARY`.
   - `banner_planet_cell_is_bright` — row 5, column 3 (`@`) → `AMBER_BRIGHT`.
   - `banner_orbit_cell_is_dim` — row 2, column 13 (`~`, inside the letter columns but on an orbit row) → `AMBER_DIM`.
   - `banner_star_rows_are_dark` — rows 1 and 9, every span `AMBER_DARK`.

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

## Step 28: TUI Layout & Hit Testing

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-28: TUI Layout & Hit Testing

[SCOPE_LOCK]
- crates/orrery-tui/src/layout.rs
- crates/orrery-tui/src/lib.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `lib.rs`: `pub mod layout;`.
2. `layout.rs`: §9.9 layout block and its bullets, plus spec §6.1 (vertical split with `Length`/`Fill`, `main_workspace` split `Percentage(25)` / `Fill(1)`).
3. Tests:
   - `too_small_below_80x24` — 79×24 and 80×23 → `too_small`, empty item lists.
   - `rows_cover_height_80x24` — header 3, top 3, main 9, live 6, footer 3.
   - `rows_cover_height_at_120x40_and_200x60` — the five heights sum to the area height; header is 11.
   - `header_switches_at_40` — height 39 → 3, height 40 → 11.
   - `regions_do_not_overlap` — at 80×24, 120×40, 200×60 the five vertical rects are pairwise disjoint; steps panel and canvas are disjoint.
   - `fifteen_stages_scroll_horizontally` — 80×24, 15 stages → visible columns × 18 ≤ canvas inner width, first column index == `stage_scroll`.
   - `slots_scroll_vertically` — 80×24: column slot count < 15, first slot index == `slot_scroll`.
   - `hit_test_corners` — for every rect in `top_items`, `footer_items`, each column header and each slot, all four corner cells return that region.
   - `hit_test_outside_is_none` — a border cell returns `None`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if any widget rendering is added in this step.
```

---

## Step 29: TUI App State & Keyboard Input

**Recommended Slash Command:** `/boost`

```text
[STEP_ID]
STEP-29: TUI App State & Keyboard Input

[SCOPE_LOCK]
- crates/orrery-tui/src/app.rs
- crates/orrery-tui/src/lib.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `lib.rs`: `pub mod app;`.
2. `app.rs`: §9.9 app block — types, `App::new`, `handle_key_event` (key table rows 1–11 in order), `apply_engine_event`, `push_stream_event`, `unlocked_slots`, `approve_visible`. `handle_mouse_event` returns `Vec::new()` in this step (filled in Step 30; allowed stub). Config edits go through the `ProjectConfig` methods (spec §5.2).
3. Tests (one per row group; `key(code)` and `ctrl(c)` helpers):
   - `ctrl_c_interrupts_only_when_running`.
   - `other_modifier_chords_do_nothing` — `Ctrl+Q`, `Ctrl+S`, `Alt+X`, `Ctrl+Tab`: no actions and `App` unchanged (compare a clone).
   - `tab_and_backtab_cycle_focus`.
   - `q_quits_when_idle_confirms_when_running`.
   - `quit_confirm_enter_interrupts_and_quits`.
   - `question_mark_opens_help_esc_closes`.
   - `m_toggles_mouse_capture`.
   - `text_entry_keeps_q_and_m_as_text` — in the Ingest modal, typing `q`, `m`, `?` appends them.
   - `ingest_enter_emits_path`.
   - `mode_toggle_emits_set_mode`.
   - `cycles_plus_minus_and_dimmed_in_run`.
   - `stage_count_clamps_2_to_15`.
   - `role_selection_unlocks_first_slot`.
   - `orchestrator_has_one_slot`.
   - `other_roles_unlock_up_to_15`.
   - `delete_unassigns_agent`.
   - `invalid_edit_sets_status_message_no_action` — e.g. setting the orchestrator role on stage 2.
   - `live_stream_scroll_and_end_follows`.
   - `approve_hidden_unless_gate_and_operator`.
   - `engine_events_update_status` — `AwaitingOperator` event → `status == AwaitingOperator`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if any modifier chord other than Ctrl+C produces an action.
```

---

## Step 30: TUI Mouse Input

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-30: TUI Mouse Input

[SCOPE_LOCK]
- crates/orrery-tui/src/app.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Replace the `handle_mouse_event` stub per the §9.9 "Mouse" line: ignored when `!mouse_capture`; left-down → `region_at` → focus + same effect as `Enter` on that item; wheel over `StepsList`, `LiveStream`, a stage column → `Up`/`Down` there. Clicks on Approve/Reject do nothing unless `approve_visible()`.
2. Tests (layout from `UiLayoutRegions::compute` at 120×40):
   - `click_run_emits_run`.
   - `click_ingest_opens_modal`.
   - `click_stage_header_opens_role_selector`.
   - `click_locked_slot_does_nothing`.
   - `wheel_scrolls_live_stream`.
   - `mouse_ignored_when_capture_off`.
   - `click_hidden_approve_does_nothing`.

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

## Step 31: TUI Widgets & Rendering

**Recommended Slash Command:** `/boost`

```text
[STEP_ID]
STEP-31: TUI Widgets & Rendering

[SCOPE_LOCK]
- crates/orrery-tui/src/widgets.rs
- crates/orrery-tui/src/lib.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `lib.rs`: `pub mod widgets;`.
2. `widgets.rs`: `render(frame, app, regions)` drawing every widget and modal of spec §6.2, each strictly inside its `UiLayoutRegions` rect, styles only from `theme.rs`; every cell background `BG_BLACK`, modals `PANEL_BG`. `too_small` → only the centered notice `"Terminal too small (need 80×24)"`. Modals are centered at 60% × 50% of the area (min 40×8), cleared with `ratatui::widgets::Clear` first.
3. Tests (`ratatui::backend::TestBackend` + `Terminal::draw`):
   - `banner_at_80x40_compact_at_80x24` — the buffer contains banner row 3 at height 40, `COMPACT_HEADER` text at 24.
   - `too_small_notice_at_79x24`.
   - `status_badges` — each `RunStatus` renders its §6.2 label.
   - `top_menu_labels` — `[Ingest Plan]`, `[Mode: RUN]`, `[Stages:`.
   - `steps_panel_badges` — `[ ]`, `[>]`, `[✓]`, `[?]`, `[!]`.
   - `slot_backend_tags` — `LM`, `CLI`, `API`.
   - `locked_slots_use_amber_dark` — fg of a locked slot cell == `AMBER_DARK`.
   - `denial_text_uses_amber_alert`.
   - `approve_reject_only_when_awaiting_gate_and_operator`.
   - `every_cell_has_black_or_panel_background` — scan the whole buffer.

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

## Step 32: TUI ↔ Engine Bridge & `run_tui`

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-32: TUI ↔ Engine Bridge & `run_tui`

[SCOPE_LOCK]
- crates/orrery-tui/src/bridge.rs
- crates/orrery-tui/src/lib.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `lib.rs`: `pub mod bridge;` and `pub use bridge::run_tui;`.
2. `bridge.rs`: §9.9 bridge block. `step` maps inputs to `App` calls and returns `Effects` (`redraw` true for every input except `Tick` with no change; `quit` = `app.should_quit`). `EngineController::handle`: config-edit actions → save via `ConfigStore::save_project`; `IngestPlan` → parse with `ImplementationPlan::from_file`, set `plan_path`/`step_marker`, save; `Run` → `build_engine(store, make_backends()?)`, spawn `engine.run(RunOptions { wait_for_operator: true, ..Default }, cancel.child_token())` on a Tokio task, forward its `EngineEvent`s and `LiveStreamHub` events into the app's channels; `Interrupt` → `cancel()`; `Approve`/`Reject` → `operator_sender().send(..)`.
   `run_tui`: enable raw mode + alternate screen + mouse capture; install a panic hook that restores the terminal then calls the previous hook; a guard struct restores on drop; loop with `tokio::select!` over `crossterm::event::EventStream` (`futures_util::StreamExt::next`), engine/stream channels and a 250 ms tick; recompute `UiLayoutRegions` on each draw; `SetMouseCapture` toggles crossterm mouse capture.
3. Tests (no terminal; `EngineController` with mock `Backends`; `#[tokio::test]`):
   - `step_key_returns_actions`.
   - `step_engine_event_updates_app`.
   - `step_stream_event_appends`.
   - `run_action_starts_engine_status_running`.
   - `interrupt_cancels_and_saves_checkpoint` — status returns to `Idle`, checkpoint step `InProgress`.
   - `approve_reaches_operator_channel` — engine awaiting operator → `Approve` → step `Passed`.
   - `config_edit_persists` — `SetStageCount(4)` → `ConfigStore::load_project` shows 4 stages.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if the render loop awaits engine work.
```

---

## Step 33: CLI Crate & Argument Parsing

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-33: CLI Crate & Argument Parsing

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-cli/Cargo.toml
- crates/orrery-cli/src/main.rs
- crates/orrery-cli/src/args.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: add `"crates/orrery-cli"` to `members`; `[workspace.dependencies]`: `clap = { version = "4.6.7", features = ["derive"] }`, `orrery-tui = { path = "crates/orrery-tui" }`; change tokio features to `["sync", "macros", "rt", "rt-multi-thread", "process", "io-util", "time", "signal"]`.
2. `crates/orrery-cli/Cargo.toml`: `name = "orrery-cli"`, workspace package fields, `[[bin]] name = "orrery"`, `path = "src/main.rs"`; `[dependencies]` `orrery-core`, `orrery-tui`, `clap`, `tokio`, `serde_json` (all `.workspace = true`); `[dev-dependencies] tempfile.workspace = true`; `[lints] workspace = true`. Run `cargo fetch`.
3. `args.rs`: §9.10 args bullet — every spec §7 command and flag, exactly those names.
4. `main.rs`: `mod args;` and a `main` that parses `Cli` and prints `{:?}` of `into_command()` (replaced in Step 34; allowed stub).
5. Tests (`Cli::try_parse_from`):
   - `every_spec_row_parses` — one assertion per §7 row (table-driven `&[&[&str]]`).
   - `bare_orrery_is_tui_dot` and `orrery_path_is_tui_path`.
   - `stage_count_out_of_range_rejected` — `workflow stages 1` and `16` fail; `2` and `15` parse.
   - `project_flag_is_global` — `orrery status --project ./x` and `orrery --project ./x status` both parse.
   - `mount_and_inference_flags` — `--mount rw` and `--mode router` parse; `--mount bad` and `--mode other` fail; `inference add` with both `--container` and `--compose` fails.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if any command or flag name differs from spec §7.
```

---

## Step 34: CLI Project & Workflow Commands

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-34: CLI Project & Workflow Commands

[SCOPE_LOCK]
- crates/orrery-cli/src/commands.rs
- crates/orrery-cli/src/main.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `commands.rs`: `CliContext`, `dispatch`, `exit_code_for` (§9.10). Handlers in this step: `init`, `ingest`, `workflow show|stages|set-role|assign|unassign|clear|mode`, `gate set|clear`, `hook set|clear`. Every handler uses `ConfigStore` + `ProjectConfig` methods only (no duplicated validation). `workflow show --json` shape per §9.10. Other commands return `Err(Error::Validation { field: "command", message: "not implemented until Step 35/36" })` — allowed stub in this step only.
2. `main.rs`: `mod commands;`, real `main` per §9.10 (async, `CliContext` with `OsKeyringStore`, exit codes).
3. Tests (temp dirs, `CliContext` writing to `Vec<u8>`, `InMemorySecretStore`):
   - `init_creates_layout_and_template`.
   - `ingest_sets_plan_and_prints_steps`.
   - `ingest_bad_plan_exits_1`.
   - `workflow_show_json_shape`.
   - `workflow_stages_grows_and_shrinks`.
   - `workflow_set_role_assign_unassign_clear`.
   - `workflow_invalid_edit_exits_1` — orchestrator role on stage 2.
   - `workflow_mode_loop_with_cycles`.
   - `gate_set_and_clear`.
   - `hook_set_and_clear`.

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

## Step 35: CLI Catalog & Secret Commands

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-35: CLI Catalog & Secret Commands

[SCOPE_LOCK]
- Cargo.toml
- Cargo.lock
- crates/orrery-cli/Cargo.toml
- crates/orrery-cli/src/commands.rs
- crates/orrery-cli/src/main.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Root: `rpassword = "7.5.5"`. CLI crate: `rpassword.workspace = true`. Run `cargo fetch`.
2. Handlers: `agent add-local-model|add-cli|add-cloud|list|show|remove`, `inference add|list|remove`, `role add|list|show|remove`, `mcp add-stdio|add-http|list|tools|remove`, `secret set|delete`. `agent add-cli` prints the §5.7.2 residual-risk note; `agent show` for `AgentCli` prints the §5.7.2 guardrail note. `role add --rules-file` reads that Markdown file and saves it via `save_role`. `mcp tools <ID>` connects through `make_backends(project)?.connector`. `secret set` uses `ctx.read_secret` and `ctx.secrets.set`; it never prints the value. When the real store's backend is `Keyutils`, print the §5.3 re-run note.
3. `main.rs`: wire `read_secret` (rpassword on a TTY, stdin line otherwise).
4. Tests:
   - `agent_add_each_backend_and_list_json`.
   - `agent_add_cli_prints_residual_risk`.
   - `agent_add_with_embedded_secret_exits_1` — `--model sk-ABCDEFGH12345678`.
   - `agent_show_and_remove`.
   - `inference_add_defaults_external_single` — only `--id` and `--endpoint` → `mode == Single`, `lifecycle == External`.
   - `inference_add_container_and_compose` — `--container llama-coder-14b` → `Container`; `--compose docker-models/router/compose.yaml --service llama --mode router` → `Compose` + `Router`.
   - `inference_list_remove`.
   - `role_add_reads_rules_file`.
   - `mcp_add_stdio_and_http_list`.
   - `mcp_tools_lists_mock_tools` — `MockMcpConnector` via `make_backends`.
   - `secret_set_and_delete` — value lands in the `InMemorySecretStore`; output doesn't contain it.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if a secret value can reach stdout, stderr or a log.
```

---

## Step 36: CLI Run, Operator & Status Commands; TUI Launch

**Recommended Slash Command:** `/boost`

```text
[STEP_ID]
STEP-36: CLI Run, Operator & Status Commands; TUI Launch

[SCOPE_LOCK]
- crates/orrery-cli/src/commands.rs
- crates/orrery-cli/src/main.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. Handlers: `run [--loop] [--cycles] [--step]`, `approve`, `reject --feedback`, `resume`, `reset --step`, `status [--json]`, `logs [--tail] [--follow]`, and `Command::Tui { path }` → `orrery_tui::run_tui(path)`. Behaviour and exit codes per §9.10 (`RunOptions { wait_for_operator: false, .. }`; Ctrl+C via `tokio::signal::ctrl_c()`). `logs` uses `stream_log::tail_log`; `--follow` polls the file every 500 ms until Ctrl+C. Remove the Step 34 stub; every `Command` variant is now handled.
2. Tests (mock `Backends` via `make_backends`):
   - `run_mode_passes_one_step_exit_0`.
   - `run_loop_cycles_two`.
   - `escalation_exits_3_then_approve_resume_completes`.
   - `reject_records_feedback` — next orchestrator prompt (from `MockAgentDriver::invocations`) contains it.
   - `approve_without_awaiting_step_exits_1`.
   - `reset_step_one_marks_pending`.
   - `status_json_shape`.
   - `logs_tail_shows_redacted_lines`.
   - `tui_action_parity` — `fn cli_equivalent(a: &AppAction) -> Option<Vec<String>>`, a `match` with one arm per `AppAction` variant and no `_` arm (a new variant then fails to compile). `Run` → `["orrery","run"]`, `Approve` → `["orrery","approve"]`, `Reject(f)` → `["orrery","reject","--feedback",f]`, `IngestPlan(p)` → `["orrery","ingest",p]`, `SetMode` → `["orrery","workflow","mode","run"|"loop"]`, `SetCycles(n)` → `["orrery","workflow","mode","loop","--cycles",n]`, `SetStageCount(n)` → `["orrery","workflow","stages",n]`, `SetStageRole` → `set-role`, `AssignAgent` → `assign`, `UnassignAgent` → `unassign`. `None` only for `Interrupt` (Ctrl+C during `orrery run`), `SetMouseCapture` and `Quit`. Assert every `Some` argv parses with `Cli::try_parse_from`.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins). Also halt if bare `orrery` doesn't route to the TUI.
```

---

## Step 37: Release Readiness Check

**Recommended Slash Command:** `/goal`

```text
[STEP_ID]
STEP-37: Release Readiness Check

[SCOPE_LOCK]
- crates/orrery-cli/tests/cli_smoke.rs

[PRE_GATE]
- `cargo test --workspace --locked` passes.

[MUTATION_SPEC]
1. `tests/cli_smoke.rs`: run `env!("CARGO_BIN_EXE_orrery")` in a temp dir: `init --name smoke`, write a 2-step plan, `ingest plan.md`, `workflow show --json` (parse with `serde_json`), `status` — each exits 0. No network, no Docker, no keychain.
2. Confirm `cargo tree --locked -i openssl-sys` and `cargo tree --locked -i zbus` find nothing, and `cargo deny check` is clean.
3. Report to the operator the live-backend command and its env vars:
     cargo test --workspace --locked --features orrery-core/live-backends
   with `ORRERY_LIVE_MCP_CMD`, `ORRERY_LIVE_IMAGE`, `ORRERY_LIVE_CONTAINER`, `ORRERY_LIVE_LLM_URL`, `ORRERY_LIVE_LLM_MODEL` (spec §8); `live_keyring` needs a desktop session or keyutils.

[POST_GATE]
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- cargo deny check
- cargo llvm-cov --workspace --locked --fail-under-lines 80

[HALT_INVARIANT]
Halt if any post-gate fails, if a file outside [SCOPE_LOCK] would change, or if this step conflicts with the spec (report the conflict; the spec wins).
```
