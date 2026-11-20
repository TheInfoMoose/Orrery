## Problem Description
Orrery runs every `### STEP` block in the plan file in file order (`engine.rs:1543` `pick_next_step_index`), but a plan holds all phases' Argon blocks and Orrery doesn't author plans. The operator needs to feed it the current phase's items in order, including operator-run ("user") items that must pause the run, e.g. a VM boot test between two steps. Done when an optional `.orrery/phase.toml` (`[[queue]]` entries with `step`, `user`, `item`, `title`) defines the run plan for `run`, `status`, `ingest`, `reset`, `mark` and the TUI; user entries pause at AwaitingOperator until `orrery approve`; `item` reaches hooks as `orrery.item_id`. Design: spec §9.20.3, §9.20.5.

## Reproduction Steps
n/a (planned work)

## Fix Log & Agent Notes
