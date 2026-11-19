## Problem Description
`orrery init` on a project that already has `.orrery/` silently overwrites `project.toml` (stages, agents, limits, validation_mode, plan_path, step_marker) and every `roles/*.toml` / `roles/*.md` with defaults, then prints "Initialized project" as if new. Suggested fix: refuse with exit 1 when `.orrery/project.toml` exists (or add `--force`). Found 2026-11-18 while writing the orrery-setup skill.
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
1. `orrery init --name T` in an empty git repo; edit `max_step_retries` and append a line to `roles/coder.md`.
2. `orrery init --name T` again.
3. Observe: both edits are gone; exit 0.

## Fix Log & Agent Notes
