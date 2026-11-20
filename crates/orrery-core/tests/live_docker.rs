use std::sync::{Arc, Mutex};

use orrery_core::checkpoint::ReportStatus;
use orrery_core::config::{AgentBackend, AgentConfig, RoleConfig, RoleKind, WorkspaceMount};
use orrery_core::docker::{
    BollardContainerBackend, ContainerBackend, ContainerSpec, ContainerState, cli_container_name,
};
use orrery_core::llm::{
    AgentDriver, AgentInvocation, CliContainerDriver, build_system_prompt, build_user_prompt,
};
use orrery_core::mcp::{McpGatekeeper, WriteScope};
use orrery_core::plan::PlanStep;
use orrery_core::security::{InMemorySecretStore, SecretRedactor};
use orrery_core::stream_log::{LiveStreamConfig, LiveStreamHub};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn live_start_stop_existing_container() {
    let Ok(name) = std::env::var("ORRERY_LIVE_CONTAINER") else {
        panic!("ORRERY_LIVE_CONTAINER must be set to an existing container name");
    };
    assert!(
        !name.trim().is_empty(),
        "ORRERY_LIVE_CONTAINER must not be empty"
    );

    let backend =
        BollardContainerBackend::connect().expect("BollardContainerBackend::connect should work");

    backend
        .start(&name)
        .await
        .expect("starting existing container should succeed");

    let running_state = backend
        .inspect(&name)
        .await
        .expect("inspect after start should succeed");
    assert_eq!(running_state, ContainerState::Running);

    backend
        .stop(&name)
        .await
        .expect("stopping container should succeed");

    let stopped_state = backend
        .inspect(&name)
        .await
        .expect("inspect after stop should succeed");
    assert!(
        matches!(stopped_state, ContainerState::Exited { .. }),
        "expected Exited state after stop, got {stopped_state:?}"
    );
}

#[tokio::test]
async fn live_run_to_completion_echoes_stdin() {
    let Ok(image) = std::env::var("ORRERY_LIVE_IMAGE") else {
        panic!("ORRERY_LIVE_IMAGE must be set to a test container image");
    };
    assert!(
        !image.trim().is_empty(),
        "ORRERY_LIVE_IMAGE must not be empty"
    );

    let backend =
        BollardContainerBackend::connect().expect("BollardContainerBackend::connect should work");

    let run_id = Uuid::new_v4();
    let name = cli_container_name(&run_id, "echo");
    let spec = ContainerSpec {
        name: name.clone(),
        image,
        cmd: vec!["cat".to_string()],
        env: Vec::new(),
        mounts: Vec::new(),
        memory_limit_mb: 64,
        auto_remove: false,
    };

    let output_lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let status = backend
        .run_to_completion(&spec, b"hello\n".to_vec(), &|line| {
            if let Ok(mut guard) = output_lines.lock() {
                guard.push(line);
            }
        })
        .await
        .expect("run_to_completion should succeed");

    backend
        .remove(&name)
        .await
        .expect("remove after run_to_completion should succeed");

    assert_eq!(status.code, 0);
    let captured = output_lines.into_inner().expect("mutex not poisoned");
    assert!(
        captured.iter().any(|l| l.contains("hello")),
        "expected output to contain 'hello', got {captured:?}"
    );
}

#[tokio::test]
async fn live_cli_driver_reads_report() {
    let Ok(image) = std::env::var("ORRERY_LIVE_IMAGE") else {
        panic!("ORRERY_LIVE_IMAGE must be set to a test container image");
    };
    assert!(
        !image.trim().is_empty(),
        "ORRERY_LIVE_IMAGE must not be empty"
    );

    let tmp = TempDir::new().expect("tempdir should be created");
    let backend = Arc::new(
        BollardContainerBackend::connect().expect("BollardContainerBackend::connect should work"),
    );
    let driver = CliContainerDriver::new(
        Arc::clone(&backend) as Arc<dyn ContainerBackend>,
        Arc::new(InMemorySecretStore::new()),
        tmp.path().to_path_buf(),
    );

    let role = RoleConfig {
        role_id: "coder".to_string(),
        display_name: "Coder".to_string(),
        kind: RoleKind::Implementer,
        rules_file: "roles/coder.md".to_string(),
        allowed_mcp_tools: Vec::new(),
        skills: Vec::new(),
    };
    let agent = AgentConfig {
        agent_id: "live-cli".to_string(),
        display_name: "Live CLI".to_string(),
        backend: AgentBackend::AgentCli {
            image,
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "cat >/dev/null; printf 'status = \"Complete\"\\nsummary = \"live cli\"\\n' > /orrery/out/report.toml"
                    .to_string(),
            ],
            workspace_mount: WorkspaceMount::None,
            memory_limit_mb: 256,
            timeout_secs: 120,
            keychain_service: None,
            keychain_account: None,
            secret_env: None,
        },
    };
    let step = PlanStep {
        index: 0,
        number: 1,
        id: "1".to_string(),
        kind: orrery_core::plan::StepKind::Agent,
        item_id: None,
        title: "1: Trivial check".to_string(),
        body: "Write report.toml.".to_string(),
        sha256_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    };

    let run_id = Uuid::new_v4();
    let container_name = cli_container_name(&run_id, &agent.agent_id);
    let inv = AgentInvocation {
        run_id,
        step_number: 1,
        stage_index: 1,
        stage_count: 3,
        round: 1,
        system_prompt: build_system_prompt(&role, "", &agent.backend),
        user_prompt: build_user_prompt(&step, None, &[], &[]),
        role,
        agent,
        tools: Vec::new(),
        write_scope: WriteScope::anywhere(),
    };

    let hub = LiveStreamHub::new(
        tmp.path().join("logs"),
        LiveStreamConfig::default(),
        Arc::new(SecretRedactor::new()),
    )
    .expect("LiveStreamHub should initialize");
    let mcp = McpGatekeeper::new();

    let report = driver
        .run_turn(&inv, &mcp, &hub)
        .await
        .expect("CliContainerDriver::run_turn should succeed");
    assert_eq!(report.status, ReportStatus::Complete);
    assert_eq!(report.summary, "live cli");

    let after_state = backend
        .inspect(&container_name)
        .await
        .expect("inspect after run_turn should succeed");
    assert_eq!(after_state, ContainerState::Missing);
}
