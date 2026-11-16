use std::fs;
use std::process::Command;

use tempfile::TempDir;

#[test]
fn init_ingest_show_status_exit_zero() {
    let bin = env!("CARGO_BIN_EXE_orrery");
    let dir = TempDir::new().expect("create tempdir");

    let init_out = Command::new(bin)
        .current_dir(dir.path())
        .args(["init", "--name", "smoke"])
        .output()
        .expect("run init");
    assert!(
        init_out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );
    assert!(dir.path().join(".orrery/project.toml").is_file());

    let plan_path = dir.path().join("plan.md");
    fs::write(
        &plan_path,
        "## Step 1: first\nalpha\n## Step 2: second\nbeta\n",
    )
    .expect("write plan.md");

    let ingest_out = Command::new(bin)
        .current_dir(dir.path())
        .args(["ingest", "plan.md"])
        .output()
        .expect("run ingest");
    assert!(
        ingest_out.status.success(),
        "ingest failed: {}",
        String::from_utf8_lossy(&ingest_out.stderr)
    );
    let ingest_stdout = String::from_utf8_lossy(&ingest_out.stdout);
    assert!(ingest_stdout.contains("1: first"));
    assert!(ingest_stdout.contains("2: second"));

    let show_out = Command::new(bin)
        .current_dir(dir.path())
        .args(["workflow", "show", "--json"])
        .output()
        .expect("run workflow show --json");
    assert!(
        show_out.status.success(),
        "workflow show --json failed: {}",
        String::from_utf8_lossy(&show_out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&show_out.stdout).expect("parse workflow show json");
    assert_eq!(
        parsed
            .get("stages")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(3)
    );

    let status_out = Command::new(bin)
        .current_dir(dir.path())
        .args(["status"])
        .output()
        .expect("run status");
    assert!(
        status_out.status.success(),
        "status failed: {}",
        String::from_utf8_lossy(&status_out.stderr)
    );
    let status_stdout = String::from_utf8_lossy(&status_out.stdout);
    assert!(status_stdout.contains("1: first"));
    assert!(status_stdout.contains("2: second"));
}
