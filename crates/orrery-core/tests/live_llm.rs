use std::sync::Arc;

use orrery_core::Error;
use orrery_core::config::{
    AgentBackend, AgentConfig, InferenceConfig, InferenceLifecycle, InferenceMode, RoleConfig,
    RoleKind,
};
use orrery_core::llm::{
    AgentDriver, AgentInvocation, HttpChatDriver, build_system_prompt, build_user_prompt,
};
use orrery_core::mcp::{McpGatekeeper, WriteScope};
use orrery_core::plan::PlanStep;
use orrery_core::security::{InMemorySecretStore, OsKeyringStore, SecretRedactor};
use orrery_core::stream_log::{LiveStreamConfig, LiveStreamHub};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn live_single_turn() {
    let Ok(raw_url) = std::env::var("ORRERY_LIVE_LLM_URL") else {
        panic!("ORRERY_LIVE_LLM_URL must be set to an OpenAI-compatible endpoint URL");
    };
    assert!(
        !raw_url.trim().is_empty(),
        "ORRERY_LIVE_LLM_URL must not be empty"
    );

    let Ok(model) = std::env::var("ORRERY_LIVE_LLM_MODEL") else {
        panic!("ORRERY_LIVE_LLM_MODEL must be set to a model identifier");
    };
    assert!(
        !model.trim().is_empty(),
        "ORRERY_LIVE_LLM_MODEL must not be empty"
    );

    let endpoint_url = raw_url
        .trim_end_matches('/')
        .trim_end_matches("/v1/chat/completions")
        .to_string();

    let inference = vec![InferenceConfig {
        inference_id: "live-local".to_string(),
        endpoint_url,
        mode: InferenceMode::Single,
        lifecycle: InferenceLifecycle::External,
        unload_after_stage: false,
        startup_timeout_secs: 60,
    }];

    let role = RoleConfig {
        role_id: "coder".to_string(),
        display_name: "Coder".to_string(),
        kind: RoleKind::Implementer,
        rules_file: "roles/coder.md".to_string(),
        allowed_mcp_tools: Vec::new(),
        skills: Vec::new(),
    };
    let agent = AgentConfig {
        agent_id: "live-coder".to_string(),
        display_name: "Live Coder".to_string(),
        backend: AgentBackend::LocalModel {
            inference_id: "live-local".to_string(),
            model,
            temperature: 0.0,
            max_context_tokens: 4096,
        },
    };
    let step = PlanStep {
        index: 0,
        number: 1,
        id: "1".to_string(),
        kind: orrery_core::plan::StepKind::Agent,
        item_id: None,
        title: "1: Trivial check".to_string(),
        body: "Call submit_report immediately with status Complete and summary 'done'.".to_string(),
        sha256_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    };

    let inv = AgentInvocation {
        run_id: Uuid::new_v4(),
        step_number: 1,
        stage_index: 1,
        stage_count: 3,
        round: 1,
        system_prompt: build_system_prompt(&role, "Complete the task immediately.", &agent.backend),
        user_prompt: build_user_prompt(&step, None, &[], &[]),
        role,
        agent,
        tools: Vec::new(),
        write_scope: WriteScope::anywhere(),
    };

    let tmp = TempDir::new().expect("tempdir should be created");
    let hub = LiveStreamHub::new(
        tmp.path().join("logs"),
        LiveStreamConfig::default(),
        Arc::new(SecretRedactor::new()),
    )
    .expect("LiveStreamHub should initialize");
    let mcp = McpGatekeeper::new();
    let secrets = Arc::new(InMemorySecretStore::new());
    let driver = HttpChatDriver::new(secrets, inference);

    let outcome = driver.run_turn(&inv, &mcp, &hub).await;
    match outcome {
        Ok(report) => {
            assert!(!report.summary.trim().is_empty());
        }
        Err(Error::Report { .. }) => {}
        Err(other) => {
            panic!("expected Ok or Error::Report from live_single_turn, got {other:?}");
        }
    }
}

#[tokio::test]
async fn live_cloud_api_turn() {
    let Ok(endpoint_url) = std::env::var("ORRERY_LIVE_CLOUD_URL") else {
        panic!("ORRERY_LIVE_CLOUD_URL must be set to a full chat-completions endpoint URL");
    };
    assert!(
        !endpoint_url.trim().is_empty(),
        "ORRERY_LIVE_CLOUD_URL must not be empty"
    );

    let Ok(model) = std::env::var("ORRERY_LIVE_CLOUD_MODEL") else {
        panic!("ORRERY_LIVE_CLOUD_MODEL must be set to a model identifier");
    };
    assert!(
        !model.trim().is_empty(),
        "ORRERY_LIVE_CLOUD_MODEL must not be empty"
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
        agent_id: "live-cloud".to_string(),
        display_name: "Live Cloud".to_string(),
        backend: AgentBackend::CloudApi {
            endpoint_url,
            model,
            temperature: 0.0,
            max_context_tokens: 16384,
            keychain_service: "orrery.test.cloud".to_string(),
            keychain_account: "testuser".to_string(),
        },
    };
    let step = PlanStep {
        index: 0,
        number: 1,
        id: "1".to_string(),
        kind: orrery_core::plan::StepKind::Agent,
        item_id: None,
        title: "1: Trivial check".to_string(),
        body: "Call submit_report immediately with status Complete and summary 'done'.".to_string(),
        sha256_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    };

    let inv = AgentInvocation {
        run_id: Uuid::new_v4(),
        step_number: 1,
        stage_index: 1,
        stage_count: 3,
        round: 1,
        system_prompt: build_system_prompt(&role, "Complete the task immediately.", &agent.backend),
        user_prompt: build_user_prompt(&step, None, &[], &[]),
        role,
        agent,
        tools: Vec::new(),
        write_scope: WriteScope::anywhere(),
    };

    let tmp = TempDir::new().expect("tempdir should be created");
    let hub = LiveStreamHub::new(
        tmp.path().join("logs"),
        LiveStreamConfig::default(),
        Arc::new(SecretRedactor::new()),
    )
    .expect("LiveStreamHub should initialize");
    let mcp = McpGatekeeper::new();
    let secrets = Arc::new(OsKeyringStore::new().expect("OsKeyringStore::new should succeed"));
    let driver = HttpChatDriver::new(secrets, Vec::new());

    let outcome = driver.run_turn(&inv, &mcp, &hub).await;
    match outcome {
        Ok(report) => {
            assert!(!report.summary.trim().is_empty());
        }
        Err(Error::Report { .. }) => {}
        Err(other) => {
            panic!("expected Ok or Error::Report from live_cloud_api_turn, got {other:?}");
        }
    }
}
