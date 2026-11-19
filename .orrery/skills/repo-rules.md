**Repo:** Orrery, a Rust CLI/TUI workflow orchestrator. Crates: `crates/orrery-core` (config, security, plan, checkpoint, stream_log, docker, mcp, llm, engine), `crates/orrery-tui` (ratatui), `crates/orrery-cli` (clap).

**Source of truth:** `/workspace/ORRERY_TECHNICAL_SPECIFICATION.md` (§9 is the binding interface section) and the step's Argon block in `/workspace/STEPPED_IMPLEMENTATION_PLAN.md`. The plan header's "Who runs what" section applies.

**Rules:**
- Gates (run on the host by the operator): `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`. Don't add or bump dependencies unless the step lists `Cargo.toml` / `Cargo.lock`.
- Never edit the spec or the plan file.
- Secrets are only handled through `SecretValue` / `SecretStore`, and must be redacted from output.
- Linux only for now (spec §9.12.1).
- CLI tests must not read the operator's global catalog. Use `test_config_home()` / `ctx.config_home`.
