## Problem Description
`crates/orrery-core/src/llm.rs:~135` — the Agents-mode decision rules are 1 Blocked→Escalate, 2 validator Denied→retry implementer, 3 validator Confirmed with no implementer since→StepPassed, 4 otherwise Dispatch. After a failed gate (`engine.rs:907` pushes the note), rule 3 still matches, so a protocol-following orchestrator reports StepPassed again → "StepPassed again without new work" violation. Workaround: the global `hybrid-orchestrator` skill tells the orchestrator to retry stage 1 on a gate-failure note. Fix: insert a rule before 3 — "a note says a gate failed after the latest Implementer report → Dispatch to the Implementer stage with the gate output".
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
