use std::sync::Mutex;

use orrery_core::docker::{
    BollardContainerBackend, ContainerBackend, ContainerSpec, ContainerState, cli_container_name,
};
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
