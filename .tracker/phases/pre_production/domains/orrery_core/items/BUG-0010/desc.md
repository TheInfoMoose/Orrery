## Problem Description
F62-8: `LogFollower::poll` can miss bytes written to the old inode between its read and its stat (same window as `tail -F`), or across multiple rotations in one poll. See `audit/` report and plan "Phase G validation notes".
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
