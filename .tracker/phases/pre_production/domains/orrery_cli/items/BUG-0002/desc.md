## Problem Description
`crates/orrery-cli/src/commands.rs:1145` (and every `ConfigStore::new(dir.path(), None)` / `dispatch` test) — tests resolve the global catalog through `dirs::config_dir()`, so real `~/.config/orrery/agents/*.toml` leaks into `load_agents` and fails `agent_add_each_backend_and_list_json` (6 ≠ 3), `agent_show_and_remove`, `agent_add_with_embedded_secret_exits_1` — pass an explicit temp `xdg_override` (or carry one in the CLI context).
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
1. Have agents in `~/.config/orrery/agents/` (now true on this machine: 3 CLI agents).
2. `cargo test --workspace --locked` → the three tests fail; they pass with `XDG_CONFIG_HOME` set to an empty dir.

## Fix Log & Agent Notes
