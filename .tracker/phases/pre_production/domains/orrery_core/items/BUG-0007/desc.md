## Problem Description
F60-6: `plan.rs:74` drops a `usize` overflow with `.ok()`; only matters when every numbered title overflows. Overlaps the step-ID item (A3). See `audit/` report and plan "Phase G validation notes".
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
