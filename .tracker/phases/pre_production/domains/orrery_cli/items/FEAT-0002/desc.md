## Problem Description
When a run is stopped because the gate itself was wrong (plan defect) and the operator validates and commits on the host, Orrery has no way to record the step as Passed: `orrery reset --step N` only sets Pending, so `status` disagrees with git and Loop mode would re-run it. Done when e.g. `orrery mark --step N passed --note <text>` records an operator pass in the checkpoint. Seen on ST 4D, 2026-11-18.
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
