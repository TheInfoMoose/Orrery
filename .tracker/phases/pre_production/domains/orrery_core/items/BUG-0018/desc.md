## Problem Description
`crates/orrery-core/src/checkpoint.rs:301` — a new checkpoint sets every step to `StepStatus::Pending`; `crates/orrery-core/src/plan.rs` has no notion of a finished step. Orrery never reads the plan's Completed roadmap table or the neb tracker's closed step tasks (`source_ref = "STEP <ID>"`). On a repo with history, the first Pending step is always plan position 1 (e.g. Perigee lists STEP 1A first), so `run` / Loop mode would redo committed work. Related: BUG-0001 (positional step numbers, needed to match IDs) and FEAT-0002 (manual operator pass). Suggested fix: at ingest (and on re-ingest), seed `Passed` for steps the source of truth marks done — the plan's `**COMPLETED**` rows and/or closed neb tasks by step ID — leaving existing checkpoint statuses intact.
Found during: User Step LOOP-1 (reported by the operator on Perigee).

## Reproduction Steps
1. In a repo whose plan has completed steps (e.g. Perigee), `orrery ingest STEPPED_IMPLEMENTATION_PLAN.md --marker "### STEP "`.
2. `orrery status` (or the TUI task list) → first step (STEP 1A) is Pending/next, though it was committed long ago.

## Fix Log & Agent Notes
