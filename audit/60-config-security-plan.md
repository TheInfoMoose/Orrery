# Audit: Config, Security, Plan, Errors (Step 60)

## Scope
- Files read: `crates/orrery-core/src/config.rs:1-2293` (non-test), `crates/orrery-core/src/config.rs:2294-3985` (tests, C11 only); `crates/orrery-core/src/security.rs:1-233` (non-test), `crates/orrery-core/src/security.rs:234-483` and `crates/orrery-core/tests/live_keyring.rs:1-30` (tests, C11 only); `crates/orrery-core/src/plan.rs:1-222` (non-test), `crates/orrery-core/src/plan.rs:223-384` (tests, C11 only); `crates/orrery-core/src/lib.rs:1-63` (non-test), `crates/orrery-core/src/lib.rs:64-196` (tests, C11 only)
- Spec ranges: `ORRERY_TECHNICAL_SPECIFICATION.md:125-295` (§4), `298-355` (§5.1–§5.4), `649-717` (§9.0–§9.2), `922-955` (§9.6a), `1160-1192` (§9.11.2), `1300-1304` (§9.12.1), plus applicable Phase D–F amendment headings in §9.11–§9.13

## Findings
### F60-1: `AgentConfig::validate`, `RoleConfig::validate`, and `InferenceConfig::validate` do not check IDs with `validate_config_file_id`
- Severity: Medium
- Confidence: High
- Category: path-safety
- Location: `crates/orrery-core/src/config.rs:771`
- Spec: §4
- Evidence: `if self.agent_id.trim().is_empty() { return Err(Error::Validation { field: "agent_id".to_string(), message: "agent_id must not be empty".to_string() }); }`
- Impact: `ConfigStore::save_agent` (line 1835), `save_role` (line 1755), and `save_inference` (line 2000) call `validate_config_file_id`, but `AgentConfig::validate` (line 771), `RoleConfig::validate` (line 646), `InferenceConfig::validate` (line 1054), `read_single_role` (line 2144), and `read_agents_from_dir` (line 2182) only check for empty strings. A hand-edited `.orrery/agents/foo.toml` with `agent_id = "../escape"` passes `load_agents()` and `ProjectConfig::validate()`, then reaches `CheckpointManager::save_report` (`checkpoint.rs:447`) and `CliContainerDriver::run_turn` (`llm.rs:1098`, which joins `agent_id` onto `.orrery/agent_io/<run_id>/` and calls `remove_dir_all` on cleanup).
- Suggested fix: Call `validate_config_file_id(&self.agent_id, "agent_id")?` in `AgentConfig::validate`, `validate_config_file_id(&self.role_id, "role_id")?` in `RoleConfig::validate`, and `validate_config_file_id(&self.inference_id, "inference_id")?` in `InferenceConfig::validate`.

### F60-2: `ConfigStore` loads roles and agents without enforcing `<id>.toml` filename matching, and `validate_config_file_id` allows leading-dot IDs that `sorted_toml_entries` ignores
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/config.rs:2173`
- Spec: §4
- Evidence: `for path in Self::sorted_toml_entries(agents_dir)? { ... let agent: AgentConfig = toml::from_str(&content)...; agent.validate()?; upsert_by_key(&mut agents, agent, |a| &a.agent_id); }`
- Impact: `read_single_role` (line 2129) and `read_agents_from_dir` (line 2173) read every non-hidden `*.toml` in `roles/` and `agents/` and key entries by the internal `role_id` / `agent_id` field without checking that `path.file_stem()` matches the ID, whereas `load_role_rules` (lines 1714, 1730), `remove_role` (line 1780), and `remove_agent` (line 1856) look up `{role_id}.toml` / `{agent_id}.toml` by filename. If `roles/custom.toml` sets `role_id = "reviewer"`, `load_roles()` and `ProjectConfig::validate()` succeed, but `load_role_rules("reviewer")` (called by `build_engine` at `engine.rs:1938` and `orrery role show` at `commands.rs:557`) fails with `Error::Io` (`NotFound`). Conversely, `validate_config_file_id` (line 1512) allows IDs starting with `.` (e.g. `.hidden`), so `save_role` / `save_agent` writes `.hidden.toml`, which `sorted_toml_entries` (line 2103) then silently skips on load.
- Suggested fix: Reject IDs starting with `'.'` in `validate_config_file_id`, and verify in `read_single_role` and `read_agents_from_dir` that `path.file_stem()` equals `role.role_id` / `agent.agent_id`.

### F60-3: Reserved role IDs `"__orrery_hooks"` and `"__orrery_gate"` are not rejected when validating, saving, or loading roles
- Severity: Low
- Confidence: High
- Category: missing-feature
- Location: `crates/orrery-core/src/config.rs:645`
- Spec: §9.0
- Evidence: `pub fn validate(&self) -> Result<()> { if self.role_id.trim().is_empty() { return Err(Error::Validation { field: "role_id".to_string(), message: "role_id must not be empty".to_string() }); }`
- Impact: Spec §9.0 states `Reserved role ids built by the engine (never loaded from disk): "__orrery_hooks", "__orrery_gate"`. Neither `RoleConfig::validate`, `ConfigStore::save_role` (line 1754), nor `ConfigStore::read_single_role` (line 2129) rejects those IDs, so `.orrery/roles/__orrery_hooks.toml` or `__orrery_gate.toml` can be saved to and loaded from disk.
- Suggested fix: Reject `role_id == "__orrery_hooks" || role_id == "__orrery_gate"` with `Error::Validation` in `RoleConfig::validate` (or in `ConfigStore::save_role` and `ConfigStore::read_single_role`).

### F60-4: `ConfigStore::remove_role` swallows `read_roles_from_dir` errors via `.unwrap_or_default()`, deleting a shared `rules_file`
- Severity: Medium
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/config.rs:1802`
- Spec: §5.2
- Evidence: `let still_used = Self::read_roles_from_dir(&roles_dir).unwrap_or_default().iter().any(|other| other.rules_file == role.rules_file); if !still_used { fs::remove_file(&rules_path)...?; }`
- Impact: When `remove_role` checks whether any remaining role in `.orrery/roles/` still references `role.rules_file`, `Self::read_roles_from_dir(&roles_dir).unwrap_or_default()` turns any parse or validation error on another role file in `.orrery/roles/` into an empty `Vec`. `still_used` then evaluates to `false`, and `remove_role` deletes `rules_path` even when another valid role in `.orrery/roles/` still shares that `rules_file`.
- Suggested fix: Propagate errors from `Self::read_roles_from_dir(&roles_dir)?` (or only delete `rules_path` when `read_roles_from_dir` succeeds and confirms no remaining role uses `role.rules_file`).

### F60-5: `ConfigStore::load_inference` resolves global `Compose.file` paths on disk before applying project overrides
- Severity: Low
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/config.rs:1953`
- Spec: §5.2
- Evidence: `for mut inf in Self::read_inference_file(gdir, &global_inf_path)? { if let InferenceLifecycle::Compose { ref mut file, .. } = inf.lifecycle { *file = resolve_compose_file(file, ConfigScope::Machine, &self.project_root, Some(gdir.as_path()))?; } upsert_by_key(&mut layered, inf, |i| &i.inference_id); }`
- Impact: Spec §5.2 requires that project entries with the same ID replace global ones. Because `load_inference` calls `resolve_compose_file` (which requires `fs::canonicalize` on the compose file on disk) inside the global loop before project `.orrery/inference.toml` and `project.toml` entries override by `inference_id`, a missing compose file on a shadowed global entry causes `load_inference()` to fail with `Error::Io` even when the project replaces that `inference_id` with a valid `Container`, `External`, or project `Compose` config.
- Suggested fix: Layer `(InferenceConfig, ConfigScope)` entries by `inference_id` first, then run `resolve_compose_file` on each surviving entry in the merged list.

### F60-6: `parse_title_number` swallows integer overflow with `.ok()`, treating an overflowing step number as an unnumbered title
- Severity: Low
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/plan.rs:74`
- Spec: §9.2
- Evidence: `if rest.is_empty() || rest.starts_with(':') || rest.starts_with('.') || rest.starts_with(' ') { title[..digit_len].parse::<usize>().ok() }`
- Impact: Spec §9.2 states: "if `title` starts with ASCII digits followed by `:`, `.`, a space or end of string, that number; otherwise `index + 1`. If **any** title starts with digits, all must, and they must be `1, 2, 3, …` in order; else `PlanParse` at the offending marker line." When a step title starts with digits that overflow `usize` (e.g. `## Step 99999999999999999999999999999: Init`), `.parse::<usize>().ok()` discards the `ParseIntError` and returns `None`, so a single-step plan with an overflowing number is accepted as unnumbered step 1 instead of failing with `Error::PlanParse`.
- Suggested fix: Return a dedicated enum (`Unnumbered`, `Numbered(usize)`, `Overflow`) from `parse_title_number` and map `Overflow` to `Error::PlanParse` at `marker_line`.

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §4 | On-disk layout (`.orrery/{project.toml,roles,agents,mcp,checkpoints,agent_io,logs}`) and `atomic_write` (temp file in same dir, `fsync`, rename) | Implemented | `crates/orrery-core/src/config.rs:1445`, `crates/orrery-core/src/config.rs:1645` |
| §4.1 | `ProjectConfig.project_name`: exists, set by `default_template`, validated non-empty, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:217`, `crates/orrery-core/src/config.rs:288`, read at `crates/orrery-core/src/engine.rs:1274` |
| §4.1 | `ProjectConfig.plan_path`: exists, default `"STEPPED_IMPLEMENTATION_PLAN.md"`, validated via `validate_safe_relative_path` + `resolve_safe_path`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:218`, `crates/orrery-core/src/config.rs:240`, `crates/orrery-core/src/config.rs:313`, read at `crates/orrery-core/src/engine.rs:1945` |
| §4.1 | `ProjectConfig.step_marker`: exists, default `"## Step "`, validated non-empty after trim, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:219`, `crates/orrery-core/src/config.rs:241`, `crates/orrery-core/src/config.rs:305`, read at `crates/orrery-core/src/engine.rs:1946` |
| §4.1 | `ProjectConfig.execution_mode`: exists, default `ExecutionMode::Run`, validated via serde enum, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:220`, `crates/orrery-core/src/config.rs:242`, read at `crates/orrery-core/src/engine.rs:347` |
| §4.1 | `ProjectConfig.cycle_count`: exists, default `0`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:221`, `crates/orrery-core/src/config.rs:243`, read at `crates/orrery-core/src/engine.rs:348` |
| §4.1 | `ProjectConfig.max_concurrent_agents`: exists, default `2`, validated `>= 1`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:222`, `crates/orrery-core/src/config.rs:244`, `crates/orrery-core/src/config.rs:315`, read at `crates/orrery-core/src/engine.rs:2039` |
| §4.1 | `ProjectConfig.max_step_retries`: exists, default `3`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:223`, `crates/orrery-core/src/config.rs:245`, read at `crates/orrery-core/src/engine.rs:679` |
| §4.1 | `ProjectConfig.validation_mode` & `effective_validation_mode`: exists, default `Auto`, `Agents` requires validator stage, `GateAndOperator` with empty `gate_tools` returns `Warning`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:224`, `crates/orrery-core/src/config.rs:268`, `crates/orrery-core/src/config.rs:428`, read at `crates/orrery-core/src/engine.rs:349` |
| §4.1 | `ProjectConfig.gate_tools`: exists, default `[]`, validated via `validate_concrete_tool_ref`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:226`, `crates/orrery-core/src/config.rs:366`, read at `crates/orrery-core/src/engine.rs:1352` |
| §4.1 | `ProjectConfig.stages` (`StageConfig.role_id`, `StageConfig.agent_ids`): default 3 stages, `2..=15` stages, stage 0 orchestrator (`0..=1` staging, `1` running), stages `1..` (`0..=15` staging, `1..=15` running), referenced IDs exist, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:39`, `crates/orrery-core/src/config.rs:227`, `crates/orrery-core/src/config.rs:295`, `crates/orrery-core/src/config.rs:393`, `crates/orrery-core/src/config.rs:456`, read at `crates/orrery-core/src/engine.rs:1550` |
| §4.1 | `ProjectConfig.hooks` (`on_step_start`, `on_step_pass`, `on_step_retry`, `on_halt`; `HookConfig.tool`, `HookConfig.on_failure` default `Halt`): validated via `validate_concrete_tool_ref`, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:62`, `crates/orrery-core/src/config.rs:77`, `crates/orrery-core/src/config.rs:123`, read at `crates/orrery-core/src/engine.rs:257`, `crates/orrery-core/src/engine.rs:1262`, `crates/orrery-core/src/engine.rs:1326` |
| §4.1 / §9.11.2 | `ProjectConfig.limits` (`RuntimeLimits`: `max_tool_calls=50`, `model_idle_timeout_secs=300`, `live_stream_ring_capacity=2000`, `live_stream_max_file_bytes=10485760`, `live_stream_max_files=3`) and `AGENT_CLI_GUARDRAIL_NOTE`: defaults, validation bounds/order, read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:18`, `crates/orrery-core/src/config.rs:154`, `crates/orrery-core/src/config.rs:175`, read at `crates/orrery-core/src/engine.rs:1865`, `crates/orrery-core/src/engine.rs:1985` |
| §4.2 | `RoleConfig` (`role_id`, `display_name`, `kind`, `rules_file`, `allowed_mcp_tools` default `[]`) & `is_orchestrator`/`is_validator`/`is_writer`: validated and read outside `config.rs` | Partial | `crates/orrery-core/src/config.rs:593`, `crates/orrery-core/src/config.rs:602`, `crates/orrery-core/src/config.rs:645`, read at `crates/orrery-core/src/llm.rs:72`, `crates/orrery-core/src/engine.rs:1938`, `crates/orrery-core/src/mcp.rs:163` (see F60-1, F60-2, F60-3) |
| §4.3 | `AgentConfig` (`agent_id`, `display_name`, `backend`): `LocalModel` (`inference_id`, `model`, `temperature`, `max_context_tokens`), `AgentCli` (`image`, `command`, `workspace_mount`, `memory_limit_mb`, `timeout_secs`, `keychain_service`, `keychain_account`, `secret_env`), `CloudApi` (`endpoint_url`, `model`, `temperature`, `max_context_tokens`, `keychain_service`, `keychain_account`): validated and read outside `config.rs` | Partial | `crates/orrery-core/src/config.rs:677`, `crates/orrery-core/src/config.rs:709`, `crates/orrery-core/src/config.rs:768`, read at `crates/orrery-core/src/llm.rs:309`, `crates/orrery-core/src/llm.rs:341`, `crates/orrery-core/src/llm.rs:987`, `crates/orrery-core/src/llm.rs:1080`, `crates/orrery-tui/src/widgets.rs:740` (see F60-1, F60-2) |
| §4.3 | `AgentConfig::validate_no_embedded_secrets` & `check_no_embedded_secret`: rejects `sk-`, `Bearer `, `AIza`, `ghp_`, `xox` | Implemented | `crates/orrery-core/src/config.rs:20`, `crates/orrery-core/src/config.rs:716`, `crates/orrery-core/src/config.rs:1432` |
| §4.3 / §9.6a | `InferenceConfig` (`inference_id`, `endpoint_url`, `mode` default `Single`, `lifecycle` default `External` / `Container { name }` / `Compose { file, service }`, `unload_after_stage` default `true`, `startup_timeout_secs` default `300`), `chat_url`, `resolve_compose_file`: validated and read outside `config.rs` | Partial | `crates/orrery-core/src/config.rs:923`, `crates/orrery-core/src/config.rs:1017`, `crates/orrery-core/src/config.rs:1050`, read at `crates/orrery-core/src/docker.rs:217`, `crates/orrery-core/src/docker.rs:251`, `crates/orrery-core/src/docker.rs:403`, `crates/orrery-core/src/docker.rs:845`, `crates/orrery-core/src/llm.rs:315` (see F60-1, F60-5) |
| §4.4 / §9.6a | `McpServerConfig` (`server_id`, `transport`: `Stdio { command, args }` / `StreamableHttp { endpoint_url, keychain_service, keychain_account }`, `write_tools`: `WriteToolSpec { tool, path_args }`) & project-scope stdio validation: validated and read outside `config.rs` | Implemented | `crates/orrery-core/src/config.rs:1134`, `crates/orrery-core/src/config.rs:1158`, `crates/orrery-core/src/config.rs:1166`, `crates/orrery-core/src/config.rs:1185`, read at `crates/orrery-core/src/mcp.rs:137`, `crates/orrery-core/src/mcp.rs:235`, `crates/orrery-core/src/mcp.rs:474`, `crates/orrery-core/src/mcp.rs:505` |
| §4 / §5.2 | Global-vs-project catalog override by id (`load_roles`, `load_agents`, `load_mcp_servers`, `load_inference` replace, not merge, global entries by id) | Partial | `crates/orrery-core/src/config.rs:1692`, `crates/orrery-core/src/config.rs:1816`, `crates/orrery-core/src/config.rs:1873`, `crates/orrery-core/src/config.rs:1945`, `crates/orrery-core/src/config.rs:2243` (see F60-5) |
| §4.1 / §5.2 | `ConfigStore::resolve_safe_path` & `resolve_safe_within_dir` (rejects `..`, absolute paths, symlink escapes, dangling symlinks) | Implemented | `crates/orrery-core/src/config.rs:1363`, `crates/orrery-core/src/config.rs:1527`, `crates/orrery-core/src/config.rs:1641` |
| §5.3 / §9.1 / §9.12.1 | `SecretValue`, `SecretStore`, `OsKeyringStore` headless fallback (Secret Service first, then Linux kernel keyutils, else `Error::Keychain`; no file/env fallback), `InMemorySecretStore` | Implemented | `crates/orrery-core/src/security.rs:12`, `crates/orrery-core/src/security.rs:44`, `crates/orrery-core/src/security.rs:56`, `crates/orrery-core/src/security.rs:112` |
| §5.3 / §9.1 | `SecretRedactor` (`register` ignores `len < 8`, sorts longest-first; `redact` replaces registered secrets then `Bearer `, `sk-`, `AIza`, `ghp_` via hand-written `redact_prefixed`) | Implemented | `crates/orrery-core/src/security.rs:154`, `crates/orrery-core/src/security.rs:200` |
| §5.4 / §9.2 | `ImplementationPlan` (`parse`, `from_file`, `plan_sha256`, `step`) & `sha256_hex`: CRLF normalization, CommonMark fences, marker prefix, contiguous `1..=N` numbering, SHA-256 of `title + "\n" + body` | Partial | `crates/orrery-core/src/plan.rs:8`, `crates/orrery-core/src/plan.rs:24`, `crates/orrery-core/src/plan.rs:80` (see F60-6) |
| §5.1 | `Error` enum (all 16 variants: `Io`, `TomlDe`, `TomlSer`, `Json`, `PathTraversal`, `Validation`, `Security`, `Keychain`, `PlanParse`, `Report`, `Docker`, `Mcp`, `Agent`, `Http`, `Engine`, `Interrupted`) and `Result<T>` | Implemented | `crates/orrery-core/src/lib.rs:14`, `crates/orrery-core/src/lib.rs:61` |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F60-2, F60-3, F60-5, F60-6 | Checked all types, fields, defaults, validations, and methods in `config.rs`, `security.rs`, `plan.rs`, and `lib.rs` against §4, §5.1–§5.4, §9.0–§9.2, §9.6a, §9.11.2, and §9.12.1, and verified every §4 field is read outside `config.rs`. |
| C2 | none | Checked all `let _ =` sites in non-test code (`config.rs:1490`, `1498`, `1506`; none in `security.rs`, `plan.rs`, `lib.rs`). `1490`/`1498` clean up temp files on write/rename failure; `1506` is best-effort parent directory `sync_all` after `tmp_file.sync_all()?` and `fs::rename` succeed. |
| C3 | findings: F60-4, F60-6 | Checked all `.ok()`, `unwrap_or*`, and `if let Ok` sites in non-test code. `config.rs:1802` uses `.unwrap_or_default()` on `read_roles_from_dir`, and `plan.rs:74` uses `.ok()` on `parse::<usize>()`. `security.rs:63` (`if let Ok(store) = dbus_secret_service_keyring_store::Store::new()`) is the spec'd headless fallback. |
| C4 | none | Checked all `match` expressions in non-test code across all four files. Only `ValidationMode` in `effective_validation_mode` (`config.rs:283`) and `Error` in `validate_concrete_tool_ref` (`config.rs:1301`) use open bindings, both intentional. |
| C5 | none | Zero occurrences of `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, or `#[allow(..)]` in non-test code of `config.rs`, `security.rs`, `plan.rs`, and `lib.rs`. |
| C6 | none | Zero numeric `as` casts in non-test code across all four files; all index and length arithmetic is bounded by slice/string lengths. |
| C7 | none | `InMemorySecretStore` (`Mutex`) and `SecretRedactor` (`RwLock`) hold locks only across synchronous in-memory collection operations and map poisoned locks to `Error::Security` per §9.0. |
| C8 | none | `config::atomic_write` (`config.rs:1445`) writes to a `create_new` temp file in the same directory, flushes, calls `sync_all()`, closes, renames over the target, and syncs the parent directory. All config mutations use `atomic_write`. |
| C9 | findings: F60-1, F60-2 | `resolve_safe_within_dir` (`config.rs:1527`) and `resolve_compose_file` (`config.rs:923`) guard against `..`, absolute paths, and symlink escapes, but `AgentConfig::validate`, `RoleConfig::validate`, and `InferenceConfig::validate` do not call `validate_config_file_id` when loading configs from disk. |
| C10 | none | `SecretValue` redacts `Debug` and `Display` output and restricts `expose()` to `pub(crate)`; `SecretRedactor` redacts registered secrets (`len >= 8`) and the four spec'd token prefixes; `AgentConfig` and `InferenceConfig` reject embedded secret patterns. |
| C11 | none | Reviewed all tests in `config.rs:2294-3985` (34 tests), `security.rs:234-483` (16 tests), `tests/live_keyring.rs:1-30` (1 test), `plan.rs:223-384` (16 tests), and `lib.rs:64-196` (4 tests). No `#[ignore]` or assertion-free tests. |
| C12 | none | Zero `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, `#[allow(dead_code)]`, or commented-out code blocks in any of the four files. |
