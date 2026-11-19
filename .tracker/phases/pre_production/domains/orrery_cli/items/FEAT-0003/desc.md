## Problem Description
`crates/orrery-core/src/config.rs:247` — `ProjectConfig.limits` has `skip_serializing_if = "RuntimeLimits::is_default"`, so `orrery config show` (text and `--json`, `crates/orrery-cli/src/commands.rs` `dispatch_config`) omits the whole `[limits]` block when every limit is at its default. Users can't see effective values such as `mcp_call_timeout_secs` without reading the source. Suggested fix: in `config show`, render limits explicitly (e.g. serialize `cfg.limits` separately under a `limits (effective):` heading / a `"limits"` JSON key) without changing the on-disk serialization.
Found during: STEP 106 validation.

## Reproduction Steps
1. `orrery init` in a scratch dir (default limits).
2. `orrery config show` — no `[limits]` section appears; `orrery config show --json` — no `limits` key.

## Fix Log & Agent Notes
