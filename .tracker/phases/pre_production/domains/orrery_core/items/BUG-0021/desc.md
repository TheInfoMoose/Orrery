## Problem Description
With `hooks.on_step_pass = neb:git_commit_documented` (default `on_failure = Halt`), re-running a step whose first pass already committed `.tracker/` (e.g. after `orrery reset --step N`) fails the hook: neb returns `Validation error: No changes to commit in workspace or .tracker/`, and Orrery halts at AwaitingOperator ("hook step_pass failed") although the step passed (`crates/orrery-core/src/engine.rs` `complete_step_pass`). Looped runs that revisit a step will hit this. Options: neb treats an empty commit as success (Nebulous-side change); or Orrery's README recommends `on_failure = continue` for commit hooks; or a per-hook "no-op is success" setting. Needs an operator decision before a step.
Found during: User Step LIVE-3 step 4 (2026-11-20), second `orrery run` after `reset --step 1`.

## Reproduction Steps
1. LIVE-3 setup (local model, `orrery hook set pass neb:git_commit_documented --args '{"summary":"orrery step pass","dry_run":false}'`), `orrery run` → passes, commits `.tracker/`.
2. `orrery reset --step 1 && orrery run` → StepPassed, then hook fails with "No changes to commit"; status `AwaitingOperator`, exit 3.

## Fix Log & Agent Notes
