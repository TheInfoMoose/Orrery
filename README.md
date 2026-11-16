# Orrery

Orrery is a Linux workflow orchestrator that executes stepped Markdown implementation plans across multi-stage agent pipelines (orchestrator, implementer, and validator stages). It coordinates local `llama.cpp` inference servers, containerized CLI agents, cloud chat-completion APIs, and Model Context Protocol (MCP) tool servers through either an interactive terminal UI (TUI) or a headless CLI.

## Requirements

- **Operating system:** Linux (`x86_64-unknown-linux-gnu` or `aarch64-unknown-linux-gnu`).
- **Rust toolchain:** Rust stable supporting Edition 2024.
- **Host build packages:** `libdbus-1-dev` and `pkg-config` (required to link D-Bus Secret Service support).
- **OS keychain:** A D-Bus Secret Service daemon on desktop sessions, or Linux kernel `keyutils` on headless hosts, for storing API keys and tokens.
- **Docker (optional):** Docker Engine, rootless Docker, or Podman (via `DOCKER_HOST`) when managing inference containers or running `AgentCli` containers.
- **Local inference server (optional):** An OpenAI-compatible `llama.cpp` server (`llama-server`) when running `LocalModel` agents.

## Build

Install the required host development packages (Debian/Ubuntu shown):

```bash
sudo apt install libdbus-1-dev pkg-config
```

Build and verify the workspace:

```bash
cargo build --workspace --locked --release
cargo test --workspace --locked
```

The compiled binary is placed at `target/release/orrery`.

## Quickstart

Initialize a project, register the `coder-14b` inference server at `http://127.0.0.1:8080` and the `local-coder` agent, assign the agent to the workflow stages, ingest a plan, and run:

```bash
# 1. Initialize .orrery/ with the default 3-stage template
orrery init --name my-service

# 2. Register an existing llama.cpp inference server
orrery inference add --id coder-14b --endpoint http://127.0.0.1:8080

# 3. Register a local-model agent backed by coder-14b
orrery agent add-local-model \
  --id local-coder \
  --name "Local Coder" \
  --inference coder-14b \
  --model Qwen2.5-Coder-14B-Instruct-Q4_K_M

# 4. Assign the agent to stages 0 (orchestrator), 1 (coder), and 2 (validator)
orrery workflow assign 0 local-coder
orrery workflow assign 1 local-coder
orrery workflow assign 2 local-coder

# 5. Ingest a stepped implementation plan
orrery ingest STEPPED_IMPLEMENTATION_PLAN.md --marker "## Step "

# 6. Inspect the workflow configuration
orrery workflow show

# 7. Run one plan step headlessly (or run `orrery` with no subcommand for the TUI)
orrery run

# 8. When a step is awaiting an operator decision, approve or reject and resume
orrery approve
orrery reject --feedback "needs test coverage"
orrery resume

# 9. View redacted live-stream logs and check workflow status
orrery logs --tail 50
orrery status
```

## TUI keys

Launch the TUI with `orrery` (or `orrery <PATH>`).

| Key / Input | Action |
| :--- | :--- |
| `Tab` / `BackTab` | Cycle focus: `TopMenu` → `StepsPanel` → `StageCanvas` → `LiveStream` → `FooterMenu` |
| `Up` / `Down` / `Left` / `Right` | Move within the focused panel; scroll when the panel or modal (`Settings`, `Help`) is a list |
| `Enter` / `Space` | Activate the focused control, open a role/agent selector, open the `Cycles` modal in `Loop` mode, or confirm a modal |
| `+` / `-` | Adjust the focused numeric control (`Cycles` in `Loop` mode, `Stages`) |
| `Esc` | Close the active modal or cancel text entry |
| `Ctrl+C` | Interrupt the running workflow cleanly (does nothing when idle; does not quit) |
| `m` | Toggle mouse capture (`[Mouse: ON\|OFF]`); turn off to use native terminal text selection in the live stream |
| `q` | Quit when not editing text (prompts for confirmation while a workflow is running) |
| `?` | Open the Help modal |
| Mouse left-click / wheel | Focus and activate controls via hit-testing; scroll the panel under the cursor (when mouse capture is on) |

In the `Ingest` modal, `Tab` / `BackTab` switches between the plan path input and the step marker input.

## Project layout

```text
~/.config/orrery/                 # Global catalog (optional). Project files override by id.
├── roles/<role_id>.toml + .md
├── agents/<agent_id>.toml
└── mcp/servers.toml

<project_root>/.orrery/
├── project.toml
├── roles/<role_id>.toml + .md
├── agents/<agent_id>.toml
├── mcp/servers.toml
├── checkpoints/
│   ├── workflow_state.toml                       # WorkflowCheckpoint
│   └── step_001/round_02_stage_01_<agent_id>.toml # StampedReport
├── agent_io/<run_id>/<agent_id>/                 # AgentCli mounts: in/ (ro), out/ (rw), mask/ (empty, ro over /workspace/.orrery). Deleted after the stage.
└── logs/live_stream.log, .log.1, .log.2
```

- `project.toml`: Workflow stages, `plan_path`, `step_marker`, `execution_mode` (`Run` or `Loop`), `validation_mode` (`Auto`, `Agents`, or `GateAndOperator`), `gate_tools`, `[hooks]`, `[[inference]]`, and optional `[limits]`.
- `roles/<role_id>.toml` + `.md`: Role kind (`Orchestrator`, `Implementer`, or `Validator`), allowed MCP tools (`<server>:<tool>` or `<server>:*`), and Markdown system-prompt rules.
- `agents/<agent_id>.toml`: Agent backend definition (`LocalModel`, `AgentCli`, or `CloudApi`).
- `mcp/servers.toml`: Configured `Stdio` and `StreamableHttp` MCP servers and their `write_tools` path-argument definitions.
- `checkpoints/`: Atomic workflow checkpoint (`workflow_state.toml`) and per-round stamped agent reports.
- `logs/`: Redacted JSON-lines event logs rotated across `live_stream.log`, `live_stream.log.1`, and `live_stream.log.2`.

## Security

- **Secrets in OS keychain only:** API keys and bearer tokens are never stored in `.orrery/` config files or read from plaintext fallback files. Store credentials with `orrery secret set --service <S> --account <A>` and remove them with `orrery secret delete --service <S> --account <A>`. `OsKeyringStore` uses D-Bus Secret Service first and falls back to Linux kernel `keyutils` on headless machines (kernel keyrings do not survive reboot, so re-run `orrery secret set` after a reboot). Config validation also rejects embedded key patterns (`sk-`, `Bearer `, `AIza`, `ghp_`, `xox`), and `SecretRedactor` scrubs registered secrets and token patterns from all live-stream events and log files.
- **MCP gatekeeper & path scope:** Agents can only call MCP tools permitted by their role's `allowed_mcp_tools`. Tools listed in a server's `write_tools` have their path arguments validated against the project root and any orchestrator-assigned `write_scope` before execution.
- **`.orrery/` mask for `AgentCli` containers:** When an `AgentCli` container mounts the project workspace at `/workspace`, Orrery bind-mounts an empty read-only directory over `/workspace/.orrery` so containerized CLI agents cannot read or tamper with `.orrery/` configs, checkpoints, or role prompts.
- **`AgentCli` guardrails and residual risks (§5.7.2):**
  - Guardrails: The MCP gatekeeper does not see a CLI's built-in tools. Guardrails are: `workspace_mount`, container isolation, memory/time limits, and whatever tool config the image ships.
  - Residual risk: Secrets are injected as an environment variable at container create time and the container is removed when the stage ends. Anyone with Docker socket access can read container environment variables while it runs (that access is already root-equivalent).
  - Residual risk: A `ReadWrite` mount (`--mount rw`) can write `.git/hooks`, which the host runs at the next `git` command.

## Live-backend tests

Live-backend integration tests live in `crates/orrery-core/tests/live_*.rs` behind the `live-backends` feature flag and are not run in the default test gate.

To set up and run the `live_docker` test:

```bash
docker pull busybox:1.37
docker create --name orrery-live-test busybox:1.37 sh -c 'trap exit TERM; while :; do sleep 1; done'
ORRERY_LIVE_IMAGE=busybox:1.37 ORRERY_LIVE_CONTAINER=orrery-live-test cargo test -p orrery-core --locked --features orrery-core/live-backends --test live_docker
```

`trap exit TERM` matters: `sh` as PID 1 ignores SIGTERM, so without it `docker stop` waits out its 10 s grace period.

To run the full live-backend suite:

```bash
cargo test --workspace --locked --features orrery-core/live-backends
```

Required environment variables and keychain setup for the full suite:

- `live_keyring`: Uses the OS keychain directly (service `orrery.test`, account `testuser`); requires an active D-Bus Secret Service session or kernel `keyutils`.
- `ORRERY_LIVE_MCP_CMD`: Executable name or command for a stdio MCP server (`live_mcp`).
- `ORRERY_LIVE_IMAGE`: Small container image with `/bin/sh`, such as `busybox:1.37` (`live_docker`).
- `ORRERY_LIVE_CONTAINER`: Existing idle container (`orrery-live-test`) that Orrery starts and stops (`live_docker`).
- `ORRERY_LIVE_LLM_URL` and `ORRERY_LIVE_LLM_MODEL`: OpenAI-compatible local server URL (e.g. `http://127.0.0.1:8080`) and model ID (`live_llm` and `live_engine`).
- `ORRERY_LIVE_CLOUD_URL` and `ORRERY_LIVE_CLOUD_MODEL`: Full chat-completions URL (e.g. `https://api.example.com/v1/chat/completions`) and model ID for `live_cloud_api_turn` (`live_llm`), with its API key stored beforehand via `orrery secret set --service orrery.test.cloud --account testuser`.
