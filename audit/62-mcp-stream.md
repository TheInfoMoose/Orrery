# Audit: MCP Gatekeeper and Live Stream (Step 62)

## Scope
- Files read: `crates/orrery-core/src/mcp.rs:1-680` (non-test), `crates/orrery-core/src/mcp.rs:681-1357` and `crates/orrery-core/tests/live_mcp.rs:1-41` (tests, C11 only); `crates/orrery-core/src/stream_log.rs:1-555` (non-test), `crates/orrery-core/src/stream_log.rs:556-1075` (tests, C11 only)
- Spec ranges: `ORRERY_TECHNICAL_SPECIFICATION.md:382-390` (§5.6), `418-430` (§5.8), `779-843` (§9.4, §9.5), `1138-1159` (§9.11.1), `1202-1224` (§9.11.4, §9.11.5), `1309-1319` (§9.12.3–§9.12.5), plus applicable Phase D–F amendment headings in §9.11–§9.13 (`§9.11.8` lines `1248-1258` for `LogFollower`)

## Findings
### F62-1: `WriteScope::contains` and `McpGatekeeper::execute_for_role` perform purely lexical path checks without resolving symlinks or blocking `.orrery/`
- Severity: High
- Confidence: High
- Category: path-safety
- Location: `crates/orrery-core/src/mcp.rs:60`
- Spec: §5.8
- Evidence: `pub fn contains(&self, path: &str) -> bool { let Some(path_segs) = normalize_path_segments(path) else { return false; }; if self.prefixes.is_empty() { return true; } self.prefixes.iter().any(|prefix| { normalize_path_segments(prefix).is_some_and(|prefix_segs| path_segs.starts_with(&prefix_segs)) }) }`
- Impact: `WriteScope::contains` only splits `path` on `'/'` and checks lexical segments without canonicalizing against `project_root` on disk (unlike `ConfigStore::resolve_safe_path`). If a symlink inside the workspace or `write_scope` points outside `project_root` or into another agent's scope (e.g. `src/escape -> /etc`), `scope.contains("src/escape/passwd")` returns `true` and the host-side MCP server executes the write. In addition, `WriteScope::anywhere()` (`prefixes` empty) returns `true` for `.orrery/project.toml` and `.orrery/checkpoints/state.toml`, allowing MCP write tools to modify `.orrery/` files that `AgentCli` masks read-only (§5.7.1).
- Suggested fix: Reject `.orrery` as the first path segment in `normalize_path_segments`, and resolve candidate write-tool paths against `project_root` on disk (rejecting symlinks that escape `project_root` or the allowed `WriteScope` prefix) in `McpGatekeeper::execute_for_role`.

### F62-2: `McpGatekeeper::execute_for_role` allows write-tool calls that omit configured `path_args` keys
- Severity: Medium
- Confidence: High
- Category: spec-gap
- Location: `crates/orrery-core/src/mcp.rs:236`
- Spec: §9.5
- Evidence: `for spec in specs.iter().filter(|spec| spec.tool == tool) { for arg_name in &spec.path_args { if let Some(val) = args.get(arg_name) { let Some(path_str) = val.as_str() else { return Err(Error::Security { ... }); };`
- Impact: Following §9.5 ("each `path_args` key present in `args` must be a JSON string with `scope.contains(..)`"), `execute_for_role` skips validation whenever `args.get(arg_name)` is `None`. If an agent invokes a write tool under a restricted `WriteScope` and omits the configured `path_args` key from `args` (e.g. `{}` when the tool defaults an omitted path to `.`, or passes an unconfigured parameter alias), `execute_for_role` forwards the call to `transport.call_tool` without enforcing `scope`.
- Suggested fix: Require every `arg_name` in `spec.path_args` to be present as a JSON string in `args` and contained in `scope`, returning `Error::Security` when any listed key is missing.

### F62-3: `RmcpTransport` applies no timeout to MCP handshake (`serve`), `list_tools`, or `call_tool`
- Severity: High
- Confidence: High
- Category: concurrency
- Location: `crates/orrery-core/src/mcp.rs:498`
- Spec: §5.8
- Evidence: `rmcp::ServiceExt::serve((), transport).await.map_err(|e| Error::Mcp { server_id: server_id.clone(), source: Box::new(e) })?`
- Impact: Neither `RmcpTransport::connect_with_stream` (`mcp.rs:498`, `529`), `RmcpTransport::list_tools` (`mcp.rs:564`), nor `RmcpTransport::call_tool` (`mcp.rs:596`) wraps the `rmcp` future in `tokio::time::timeout`. If a stdio child process or remote StreamableHttp MCP server stalls during the `initialize` handshake, tool catalog refresh, or a tool call, `build_engine`, `orrery mcp add-*`/`tools`, agent turns, gate checks, or lifecycle hooks hang indefinitely.
- Suggested fix: Wrap `rmcp::ServiceExt::serve`, `peer().list_all_tools()`, and `peer().call_tool()` in `tokio::time::timeout` using a configured timeout from `RuntimeLimits`, mapping elapsed timeouts to `Error::Mcp`.

### F62-4: `spawn_stderr_forwarder` exits and closes the child's stderr pipe on non-UTF-8 bytes, buffers unbounded lines, and drops its `JoinHandle`
- Severity: Medium
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/mcp.rs:553`
- Spec: §9.11.1
- Evidence: `let mut lines = tokio::io::BufReader::new(stderr).lines(); while let Ok(Some(line)) = lines.next_line().await { if !line.trim().is_empty() { let _ = stream.emit(StreamEvent::notice(format!("[mcp:{server_id}] {line}"))); } }`
- Impact: Implementing §9.11.1 as specified, `BufReader::lines().next_line().await` returns `Err(io::ErrorKind::InvalidData)` if the MCP child writes a single non-UTF-8 byte to stderr, terminating the `while let Ok(Some(line))` loop and dropping `stderr`. Closing the read end of the pipe causes subsequent stderr writes by the child MCP server to fail with `EPIPE` / `SIGPIPE`. In addition, `next_line()` buffers into an unbounded `String` until `\n` (unlike `HttpChatDriver`'s 4096-byte `LineBuffer`), and `connect_with_stream` (`mcp.rs:493`) drops the returned `JoinHandle<()>` without storing or aborting it when the transport drops.
- Suggested fix: Read raw byte chunks from `stderr` with lossy UTF-8 decoding and a bounded line buffer so invalid UTF-8 never closes the child's stderr pipe, and keep or abort the task handle on `RmcpTransport`.

### F62-5: `RmcpTransport::connect_with_stream` passes the HTTP bearer secret into `rmcp` as an unzeroized `String`
- Severity: Low
- Confidence: High
- Category: secrets
- Location: `crates/orrery-core/src/mcp.rs:517`
- Spec: §9.5
- Evidence: `(Some(svc), Some(acc)) => { let secret = secrets.get(svc, acc)?; cfg = cfg.auth_header(secret.expose()); }`
- Impact: Whereas Step 52 (§9.12.2) updated `HttpChatDriver::auth_header_value` to build the bearer header in `zeroize::Zeroizing<String>` and validate/mark the HTTP `HeaderValue` with `set_sensitive(true)`, `RmcpTransport::connect_with_stream` passes `secret.expose()` to `StreamableHttpClientTransportConfig::auth_header`, which stores an unzeroized `Option<String>` copy in memory and does not pre-validate header bytes before connecting.
- Suggested fix: Validate that `secret.expose()` is a valid HTTP header value before calling `serve`, and if supported by the HTTP client configuration, pass a `set_sensitive(true)` header built from a `Zeroizing<String>`.

### F62-6: `tail_log` hardcodes `.log.2`, `.log.1`, `.log` and uses `fs::read_to_string`, breaking when `live_stream_max_files > 3` or when logs contain non-UTF-8 bytes
- Severity: Medium
- Confidence: High
- Category: bug
- Location: `crates/orrery-core/src/stream_log.rs:356`
- Spec: §9.4
- Evidence: `let candidate_paths = [ rotated_log_path(log_dir, 2), rotated_log_path(log_dir, 1), log_dir.join(LOG_FILE), ]; let mut lines = Vec::new(); for path in candidate_paths { match fs::read_to_string(&path) {`
- Impact: Step 39/41 (§9.11.2, §9.11.4) made `live_stream_max_files` configurable in `1..=10` and `rotate_files` (`stream_log.rs:332`) rotates up to `.log.{max_files - 1}`, but `tail_log` still checks only `.log.2`, `.log.1`, and `.log` (ignoring `.log.3`..`.log.9` when `max_files > 3`, and reading leftover `.log.2`/`.log.1` when `max_files < 3`). Furthermore, `tail_log` uses `fs::read_to_string(&path)` (`stream_log.rs:364`), so if any log file contains non-UTF-8 bytes, `orrery logs` and `orrery logs --follow` (which calls `tail_log` at `commands.rs:902` before `LogFollower::new`) fail with `Error::Io` despite Step 54 (§9.12.4) making `LogFollower` byte-safe.
- Suggested fix: Scan rotated log files up to `RuntimeLimits` max (`9` down to `1`, or pass `max_files`) and read files with `fs::read` + `String::from_utf8_lossy` instead of `fs::read_to_string`.

### F62-7: `LiveStreamHub::emit` only surfaces asynchronous writer-thread errors on a subsequent `emit` call, and no production path calls `flush()`
- Severity: Medium
- Confidence: High
- Category: error-handling
- Location: `crates/orrery-core/src/stream_log.rs:242`
- Spec: §9.12.5
- Evidence: `let _ = self.sender.send(event); let write_err = { let mut slot = self.write_error.lock().map_err(|_| Error::Security { message: "LiveStreamHub lock poisoned".into() })?; slot.take() };`
- Impact: `emit` sends `WriterMsg::Line(line)` over `mpsc` to the `orrery-log-writer` thread and immediately checks `self.write_error` in the same call, typically before the background thread has executed `write_line`. If a disk write or rotation fails on the final event(s) of a run (or when no subsequent `emit` occurs after `LogWriter::record_error` sets `shared_error`), the `StreamEventKind::Error` (`"live stream log write failed: ..."`) is never pushed to the ring or broadcast, and because no non-test code ever calls `LiveStreamHub::flush()`, the error is silently lost when `LiveStreamHub` drops.
- Suggested fix: Give `LogWriter` access to `ring`, `redactor`, and `sender` so `record_error` emits the `StreamEventKind::Error` immediately from the writer thread when a write fails, and call `stream.flush()` on engine shutdown.

### F62-8: `LogFollower::poll` can drop lines written to the old inode right before rotation or across multiple rotations in one poll interval
- Severity: Low
- Confidence: High
- Category: file-io
- Location: `crates/orrery-core/src/stream_log.rs:433`
- Spec: §9.11.8
- Evidence: `if let Some(file) = self.file.as_mut() { ... let read_bytes = file.read_to_end(&mut self.partial)...; self.offset += read_bytes as u64; } match fs::metadata(&self.path) { Ok(meta) => { if self.file.is_none() || meta.ino() != self.ino || meta.len() < self.offset {`
- Impact: In `LogFollower::poll`, if `LogWriter` appends final lines to the old `live_stream.log` and rotates it to `live_stream.log.1` in the window between step 1 (`file.read_to_end` on the old handle at line 425) and step 2 (`fs::metadata(&self.path)` at line 433), step 2 sees the new inode at `live_stream.log` and replaces `self.file` without reading the final bytes written to the old handle before rotation. Similarly, if two or more rotations happen within a single 500 ms sleep in `orrery logs --follow`, intermediate rotated files (`.log.1`, `.log.2`) are skipped.
- Suggested fix: When `meta.ino() != self.ino` is detected in step 2, perform one more `read_to_end` on the existing `self.file` handle after opening the new file (when `open_meta.ino() != self.ino`) before replacing `self.file`.

## Coverage
| Spec § | Requirement (short) | Status | Where |
| :--- | :--- | :--- | :--- |
| §5.8 / §9.5 / §9.11.5 | `McpGatekeeper::is_tool_allowed` and role patterns (`server:tool`, `server:*`, rejects `*:*` and unmappable `<server>__<tool>` names) | Implemented | `crates/orrery-core/src/mcp.rs:168` |
| §5.8 / §9.5 | `WriteScope` (`new`, `anywhere`, `contains`), `scopes_overlap`, and `execute_for_role` `write_tools` path-argument checks (`..`, absolute paths, symlinks, missing arguments, project root) | Partial | `crates/orrery-core/src/mcp.rs:38`, `crates/orrery-core/src/mcp.rs:74`, `crates/orrery-core/src/mcp.rs:92`, `crates/orrery-core/src/mcp.rs:204` (see F62-1, F62-2) |
| §5.8 / §9.5 / §9.11.1 | `Stdio` and `StreamableHttp` transports (`RmcpTransport::connect`, `connect_in`, `connect_with_stream`, `RmcpConnector`, `resolve_stdio_command`, handshake, timeouts, auth header) | Partial | `crates/orrery-core/src/mcp.rs:364`, `crates/orrery-core/src/mcp.rs:448`, `crates/orrery-core/src/mcp.rs:561`, `crates/orrery-core/src/mcp.rs:609` (see F62-3, F62-5) |
| §9.11.1 | MCP child stderr forwarder (`spawn_stderr_forwarder`, `McpConnector::attach_stream`, `[mcp:{server_id}] {line}` redacted `SystemNotice` events) | Partial | `crates/orrery-core/src/mcp.rs:34`, `crates/orrery-core/src/mcp.rs:484`, `crates/orrery-core/src/mcp.rs:542`, `crates/orrery-core/src/mcp.rs:654` (see F62-4) |
| §5.8 / §9.5 / §9.12.3 | `McpGatekeeper` catalog and `tools_for_role` (`register`, `refresh_catalog`, `catalog`, `is_write_tool`, `tools_for_role`) | Implemented | `crates/orrery-core/src/mcp.rs:122`, `crates/orrery-core/src/mcp.rs:144`, `crates/orrery-core/src/mcp.rs:164`, `crates/orrery-core/src/mcp.rs:190`, `crates/orrery-core/src/mcp.rs:196` |
| §9.5 | Tool-result handling (`tool_result_is_error`, `tool_result_text`, `MockMcpTransport`, `MockMcpConnector`) | Implemented | `crates/orrery-core/src/mcp.rs:100`, `crates/orrery-core/src/mcp.rs:107`, `crates/orrery-core/src/mcp.rs:266`, `crates/orrery-core/src/mcp.rs:660` |
| §5.6 / §9.4 | Live stream redaction of every event (`StreamEventKind`, `StreamEvent`, `StreamEvent::notice`, `LiveStreamHub::emit` redacts `event.text` before ring/file/broadcast, `redactor()`) | Implemented | `crates/orrery-core/src/stream_log.rs:19`, `crates/orrery-core/src/stream_log.rs:30`, `crates/orrery-core/src/stream_log.rs:209`, `crates/orrery-core/src/stream_log.rs:256`, `crates/orrery-core/src/stream_log.rs:316` |
| §5.6 / §9.4 | Live stream ring buffer (`ring_capacity`, oldest eviction, `recent`, `subscribe` broadcast channel) | Implemented | `crates/orrery-core/src/stream_log.rs:194`, `crates/orrery-core/src/stream_log.rs:212`, `crates/orrery-core/src/stream_log.rs:304`, `crates/orrery-core/src/stream_log.rs:308` |
| §9.11.4 | Live stream writer thread (`orrery-log-writer`, `WriterMsg`, `LogWriter`, `flush`, `Drop` joins writer after draining queued lines) | Implemented | `crates/orrery-core/src/stream_log.rs:69`, `crates/orrery-core/src/stream_log.rs:74`, `crates/orrery-core/src/stream_log.rs:168`, `crates/orrery-core/src/stream_log.rs:277`, `crates/orrery-core/src/stream_log.rs:321` |
| §5.6 / §9.4 | Log file rotation and oversize-line fitting (`rotate_files` before exceeding `max_file_bytes`, `fit_event_line` binary search on UTF-8 char boundaries) | Implemented | `crates/orrery-core/src/stream_log.rs:95`, `crates/orrery-core/src/stream_log.rs:332`, `crates/orrery-core/src/stream_log.rs:511` |
| §9.4 | `tail_log` (reads rotated logs and `live_stream.log` in oldest-to-newest order, returns last `limit` lines) | Partial | `crates/orrery-core/src/stream_log.rs:355` (see F62-6) |
| §9.11.8 / §9.12.4 | `LogFollower` (`new`, `poll` across inode rotation and truncation, byte-safe `partial: Vec<u8>` for split UTF-8) | Partial | `crates/orrery-core/src/stream_log.rs:377` (see F62-8) |
| §9.12.5 | Live stream write-error event (`write_error` shared slot, redacted `"live stream log write failed: {msg}"` `StreamEventKind::Error` to ring and broadcast only) | Partial | `crates/orrery-core/src/stream_log.rs:85`, `crates/orrery-core/src/stream_log.rs:242` (see F62-7) |

## Checklist
| Check | Result | Notes |
| :--- | :--- | :--- |
| C1 | findings: F62-2, F62-6, F62-7 | Checked all types, signatures, ordering rules, error variants, and behaviors in `mcp.rs` and `stream_log.rs` against §5.6, §5.8, §9.4, §9.5, §9.11.1, §9.11.4, §9.11.5, §9.11.8, and §9.12.3–§9.12.5. |
| C2 | none | Checked all `let _ =` sites in non-test code (`mcp.rs:555`, `655`; `stream_log.rs:184`, `240`, `271`, `324`, `327`). Each is explicitly specified by §9.4, §9.11.1, §9.11.4, or §9.12.5 (the dropped `JoinHandle` at `mcp.rs:493` is covered under C7 / F62-4). |
| C3 | findings: F62-4, F62-7 | Checked all `.ok()`, `unwrap_or*`, `if let Ok`, `while let Ok`, and `Err` sites in non-test code. `mcp.rs:553` `while let Ok(Some(line)) = lines.next_line().await` terminates the stderr forwarder and drops `ChildStderr` on non-UTF-8 bytes; `stream_log.rs:242` only drains `write_error` on a subsequent `emit`. |
| C4 | none | Checked all `match` expressions in non-test code of `mcp.rs` and `stream_log.rs`. Matches on `McpTransportConfig` (`mcp.rs:473`, `619`) and `WriterMsg` (`stream_log.rs:181`) are exhaustive without `_ =>` catch-all arms. |
| C5 | none | Zero occurrences of `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, or `#[allow(..)]` in non-test code of `mcp.rs` (lines 1–680) and `stream_log.rs` (lines 1–555). |
| C6 | none | Checked all `as u64` casts (`stream_log.rs:96`, `430`, `451`, `513`, `520`, `541`, widening `usize` to `u64` on 64-bit Linux) and arithmetic (`saturating_sub` at `stream_log.rs:312`, `373`; safe binary search bounds at `531-550`; `max_files <= 1` guard before `max_files - 1` at `333`). |
| C7 | findings: F62-3, F62-4, F62-7 | No `std::sync::Mutex` lock is held across `.await` in `mcp.rs` or `stream_log.rs`. Unbounded awaits on `rmcp` transport calls (F62-3), the unjoined/unaborted `spawn_stderr_forwarder` task and unbounded `next_line()` buffer (F62-4), and the race between `emit` and `LogWriter::record_error` (F62-7) are reported above. |
| C8 | findings: F62-6, F62-8 | Checked log rotation, `tail_log`, and `LogFollower::poll`. `tail_log` hardcoded file count and strict UTF-8 read (F62-6) and the `LogFollower::poll` rotation race window (F62-8) are reported above. |
| C9 | findings: F62-1, F62-2 | `resolve_stdio_command` (`mcp.rs:364`) canonicalizes commands and blocks executables inside the agent-writable workspace outside `.orrery/`. Lexical-only `WriteScope::contains` / `.orrery/` exposure (F62-1) and omitted `path_args` keys in `execute_for_role` (F62-2) are reported above. |
| C10 | findings: F62-5 | `LiveStreamHub::emit` redacts `event.text` and write-error messages before ring storage, file serialization, and broadcast (`stream_log.rs:210`, `256`), and `spawn_stderr_forwarder` routes lines through `stream.emit`. The unzeroized `String` copy in `StreamableHttpClientTransportConfig::auth_header` is reported in F62-5. |
| C11 | none | Reviewed all unit tests in `mcp.rs:681-1357` (22 tests), `stream_log.rs:556-1075` (15 tests), and `tests/live_mcp.rs:1-41` (1 test). No `#[ignore]` or assertion-free tests. |
| C12 | none | Zero `TODO`, `FIXME`, `XXX`, `HACK`, stub bodies, `#[allow(dead_code)]`, or commented-out code blocks in `mcp.rs` or `stream_log.rs`. |
