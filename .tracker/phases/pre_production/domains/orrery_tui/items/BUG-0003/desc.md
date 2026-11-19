## Problem Description
`orrery-tui` `app.rs`, `bridge.rs` — `status_message = Some(err.to_string())` drops the error's source chain, like the CLI did before Step 45 — reuse a core helper once one exists.
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
