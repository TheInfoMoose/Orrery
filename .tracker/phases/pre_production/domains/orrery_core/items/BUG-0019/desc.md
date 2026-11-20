## Problem Description
`crates/orrery-core/src/checkpoint.rs` `reconcile_by_id` indexes `self.step_ids[cur]` and zips `step_ids` with `step_hashes` / `step_statuses`, assuming equal lengths. A hand-edited or truncated `workflow_state.toml` with mismatched lengths panics (index out of bounds) or silently drops steps instead of returning an error. Orrery itself always writes them together. Suggested fix: at the top of `reconcile` (and in `load_state`), return `Error::Validation { field: "checkpoint", message: "step_ids, step_hashes and step_statuses lengths differ; run `orrery reset --step 1`" }` when non-empty `step_ids` length ≠ `step_statuses.len()` or `step_hashes.len()`.
Found during: STEP 114 validation.

## Reproduction Steps
1. Edit `.orrery/checkpoints/workflow_state.toml` and delete one entry from `step_ids`, with the current step InProgress at the last index.
2. `orrery status` → panic instead of a Validation error.

## Fix Log & Agent Notes
