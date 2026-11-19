## Problem Description
F62-7: the Error event appears on the *next* `emit`, so a failure on a run's last events is never shown (§9.12.5 design limit) — emit from the writer thread. See `audit/` report and plan "Phase G validation notes".
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
