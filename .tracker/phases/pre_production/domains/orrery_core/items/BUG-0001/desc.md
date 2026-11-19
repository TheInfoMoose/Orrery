## Problem Description
Nebulous Phase 21 audit A3 (should-fix): Orrery numbers steps by position, so `step_number` isn't the plan's step ID. Headings like `### STEP 13A:` parse as unnumbered, so 139 steps become 1..139 (13A → 1); hooks get the wrong number and only `step_title` carries `13A`. One plain `### STEP 40:` heading makes ingest fail ("unnumbered step where step N was expected"). No start-at-step by ID either. Suggested fix: carry the parsed step ID (`^[0-9]+[A-Z]*$`) through plan, checkpoints and hook args; let `run --step` take it.
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
1. `orrery ingest STEPPED_IMPLEMENTATION_PLAN.md --marker "### STEP "` on a plan with lettered IDs (e.g. Nebulous).
2. Observe: steps numbered 1..N; hook `orrery.step_number` ≠ the plan's ID.

## Fix Log & Agent Notes
