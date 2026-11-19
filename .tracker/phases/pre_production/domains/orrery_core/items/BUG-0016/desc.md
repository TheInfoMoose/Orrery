## Problem Description
`crates/orrery-core/src/config.rs:1531` — `check_no_embedded_secret` (the `sk-` / `Bearer ` / `AIza` / `ghp_` / `xox` check) runs on agent config, but `HookConfig.args` and `ProjectConfig.gate_args` (`validate_tool_args`, config.rs:~1387) only reject the reserved `orrery` key and `null`. A token pasted into `orrery hook set ... --args` or `orrery gate args` lands in the committed `project.toml`, and Phase N profiles would copy it to `<global>/profiles/`. Suggested fix: walk every string value in the args map through `check_no_embedded_secret` inside `validate_tool_args`.
Found during: Phase N planning (2026-11-18).

## Reproduction Steps
1. `orrery hook set pass srv:tool --args '{"token":"ghp_example0000"}'`
2. Observe: exit 0, and `.orrery/project.toml` contains the value.

## Fix Log & Agent Notes
