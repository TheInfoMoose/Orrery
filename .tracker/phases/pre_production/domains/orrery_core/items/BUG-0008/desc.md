## Problem Description
F61-5: `llm.rs:880` returns at the first valid `submit_report`, silently dropping later tool calls in the same batch; spec silent. See `audit/` report and plan "Phase G validation notes".
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
