## Problem Description
`crates/orrery-core` `stream_log.rs`, `checkpoint.rs`, `llm.rs` use chrono (`Utc::now`, `DateTime<Utc>`). RUSTSEC-2026-0543 marks chrono unmaintained; `deny.toml` ignores it (operator, 2026-11-17) because rmcp 3.9 also pulls it. Migrate to jiff (matches Nebulous) and drop the ignore once rmcp no longer depends on chrono.
Migrated from the plan file's Parked list (2026-11-18).

## Reproduction Steps
n/a (parked finding; see Problem Description)

## Fix Log & Agent Notes
