## Problem Description
`crates/orrery-tui/src/bridge.rs:344,366` — the TUI loads `ProjectConfig` once (`load_initial_app`, :633) and every SetMode/SetCycles/Assign/Unassign/IngestPlan action calls `save_project(&app.config)` with that whole in-memory copy. Any change made on disk after the TUI started (CLI `orrery gate set`, `hook set`, a hand edit, a git pull) is silently reverted. Seen 2026-11-18 in Perigee: switching to Loop in a TUI opened before a config commit reverted validation_mode, gate_tools, [hooks] and [limits]. Fix: reload from disk and apply only the changed field before saving (or refuse with a 'changed on disk' message when the file's mtime/hash differs from load time).
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
1. Open `orrery` (TUI) in a configured project.
2. In another shell: `orrery gate set some:tool`.
3. In the TUI, change the execution mode.
4. Observe: `gate_tools` in project.toml is back to the TUI's startup value.

## Fix Log & Agent Notes
