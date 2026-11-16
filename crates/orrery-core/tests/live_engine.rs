use std::path::PathBuf;
use std::time::Duration;

use orrery_core::config::{
    AgentBackend, AgentConfig, ConfigStore, InferenceConfig, InferenceLifecycle, InferenceMode,
};
use orrery_core::engine::{Backends, RunOptions, build_engine};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn live_engine_run_with_real_backends() {
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

    let tmp = TempDir::new().expect("tempdir should be created");
    let store = ConfigStore::new(tmp.path(), None).expect("ConfigStore::new should succeed");
    let mut project = store
        .init_project("live")
        .expect("init_project should succeed");

    std::fs::write(
        store.project_root().join("plan.md"),
        "## Step 1: trivial\nCall submit_report now.\n",
    )
    .expect("writing plan.md should succeed");

    project.plan_path = PathBuf::from("plan.md");
    project.max_step_retries = 1;
    project.inference = vec![InferenceConfig {
        inference_id: "live-local".to_string(),
        endpoint_url,
        mode: InferenceMode::Single,
        lifecycle: InferenceLifecycle::External,
        unload_after_stage: false,
        startup_timeout_secs: 60,
    }];
    for idx in 0..project.stages.len() {
        project
            .assign_agent(idx, "live-lm")
            .expect("assign_agent should succeed");
    }
    store
        .save_project(&project)
        .expect("save_project should succeed");

    let agent = AgentConfig {
        agent_id: "live-lm".to_string(),
        display_name: "Live LM".to_string(),
        backend: AgentBackend::LocalModel {
            inference_id: "live-local".to_string(),
            model,
            temperature: 0.0,
            max_context_tokens: 16384,
        },
    };
    store.save_agent(&agent).expect("save_agent should succeed");

    let backends = Backends::real(tmp.path()).expect("Backends::real should succeed");
    let engine = build_engine(&store, backends)
        .await
        .expect("build_engine should succeed");

    let outcome = tokio::time::timeout(
        Duration::from_secs(600),
        engine.run(
            RunOptions {
                wait_for_operator: false,
                ..Default::default()
            },
            CancellationToken::new(),
        ),
    )
    .await;

    match outcome {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => {
            panic!("engine.run returned error: {err:?}");
        }
        Err(elapsed) => {
            panic!("engine.run timed out after 600s: {elapsed}");
        }
    }
}
