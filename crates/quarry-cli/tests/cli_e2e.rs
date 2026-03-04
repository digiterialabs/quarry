use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn write_fixture_files() -> (tempfile::TempDir, String, String) {
    let dir = tempdir().expect("tempdir");
    let model_path = dir.path().join("model.yml");
    let query_path = dir.path().join("query.json");

    fs::write(
        &model_path,
        include_str!("../../../models/example/model.yml"),
    )
    .expect("write model");
    fs::write(
        &query_path,
        include_str!("../../../models/example/query.json"),
    )
    .expect("write query");

    (
        dir,
        model_path.to_string_lossy().to_string(),
        query_path.to_string_lossy().to_string(),
    )
}

fn write_fixture_files_with_data() -> (tempfile::TempDir, String, String, String) {
    let (dir, model, query) = write_fixture_files();
    let data_dir = dir.path().join("data");
    fs::create_dir_all(&data_dir).expect("create data dir");

    fs::write(
        data_dir.join("orders.csv"),
        include_str!("../../../models/example/data/orders.csv"),
    )
    .expect("write orders");

    fs::write(
        data_dir.join("customers.csv"),
        include_str!("../../../models/example/data/customers.csv"),
    )
    .expect("write customers");

    (dir, model, query, data_dir.to_string_lossy().to_string())
}

fn write_query_by_region(dir: &tempfile::TempDir) -> String {
    let path = dir.path().join("query_by_region.json");
    fs::write(
        &path,
        include_str!("../../../models/example/query_by_region.json"),
    )
    .expect("write query_by_region");
    path.to_string_lossy().to_string()
}

fn write_filesystem_sync_config(dir: &tempfile::TempDir, docs_rel_dir: &str) -> String {
    let config_path = dir.path().join("sync_config.json");
    let docs_path = dir.path().join(docs_rel_dir);
    fs::create_dir_all(&docs_path).expect("create docs dir");
    fs::write(
        docs_path.join("sales_playbook.txt"),
        "Enterprise revenue playbook for EMEA and NA teams with tenant-safe guidance.",
    )
    .expect("write docs file");

    let config = serde_json::json!({
        "paths": [docs_path.to_string_lossy().to_string()],
        "recursive": true,
        "extensions": ["txt"]
    });
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).expect("json"),
    )
    .expect("write config");
    config_path.to_string_lossy().to_string()
}

#[test]
fn validate_command_succeeds() {
    let (_dir, model, _query) = write_fixture_files();

    let mut cmd = Command::cargo_bin("quarry").expect("binary should build");
    cmd.arg("validate").arg("--model").arg(&model);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"validated\": true"));
}

#[test]
fn query_command_returns_v1_envelope() {
    let (_dir, model, query) = write_fixture_files();

    let mut cmd = Command::cargo_bin("quarry").expect("binary should build");
    cmd.arg("query")
        .arg("--model")
        .arg(&model)
        .arg("--catalog")
        .arg("local")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--input")
        .arg(&query);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": \"v1\""))
        .stdout(predicate::str::contains("\"status\": \"ok\""));
}

#[test]
fn explain_command_returns_plan_payload() {
    let (_dir, model, query) = write_fixture_files();

    let mut cmd = Command::cargo_bin("quarry").expect("binary should build");
    cmd.arg("explain")
        .arg("--model")
        .arg(&model)
        .arg("--catalog")
        .arg("local")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--input")
        .arg(&query);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"plan\""));
}

#[test]
fn glue_catalog_requires_aws_region() {
    let (_dir, model, query) = write_fixture_files();

    let mut cmd = Command::cargo_bin("quarry").expect("binary should build");
    cmd.arg("query")
        .arg("--model")
        .arg(&model)
        .arg("--catalog")
        .arg("glue")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--input")
        .arg(&query)
        .env_remove("AWS_REGION");

    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"CONFIG_ERROR\""));
}

#[test]
fn local_data_dir_returns_non_empty_rows() {
    let (_dir, model, query, data_dir) = write_fixture_files_with_data();

    let mut cmd = Command::cargo_bin("quarry").expect("binary should build");
    cmd.arg("query")
        .arg("--model")
        .arg(&model)
        .arg("--catalog")
        .arg("local")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--local-data-dir")
        .arg(&data_dir)
        .arg("--input")
        .arg(&query);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"row_count\": 3"))
        .stdout(predicate::str::contains("\"tenant_id\": \"tenant_123\""))
        .stdout(predicate::str::contains("\"revenue\": 100.0"));
}

#[test]
fn tenant_isolation_produces_distinct_region_aggregates() {
    let (dir, model, _query, data_dir) = write_fixture_files_with_data();
    let query_by_region = write_query_by_region(&dir);

    let mut tenant_123_cmd = Command::cargo_bin("quarry").expect("binary should build");
    tenant_123_cmd
        .arg("query")
        .arg("--model")
        .arg(&model)
        .arg("--catalog")
        .arg("local")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--local-data-dir")
        .arg(&data_dir)
        .arg("--input")
        .arg(&query_by_region);

    tenant_123_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"orders.region\": \"EU\""))
        .stdout(predicate::str::contains("\"revenue\": 250.0"))
        .stdout(predicate::str::contains("\"tenant_id\": \"tenant_123\""));

    let mut tenant_999_cmd = Command::cargo_bin("quarry").expect("binary should build");
    tenant_999_cmd
        .arg("query")
        .arg("--model")
        .arg(&model)
        .arg("--catalog")
        .arg("local")
        .arg("--tenant")
        .arg("tenant_999")
        .arg("--local-data-dir")
        .arg(&data_dir)
        .arg("--input")
        .arg(&query_by_region);

    tenant_999_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"orders.region\": \"APAC\""))
        .stdout(predicate::str::contains("\"revenue\": 500.0"))
        .stdout(predicate::str::contains("\"tenant_id\": \"tenant_999\""))
        .stdout(predicate::str::contains("\"orders.region\": \"EU\"").not());
}

#[test]
fn collection_create_and_list_are_tenant_scoped() {
    let dir = tempdir().expect("tempdir");
    let context_dir = dir.path().join("context");

    let mut create_cmd = Command::cargo_bin("quarry").expect("binary should build");
    create_cmd
        .arg("collection")
        .arg("create")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--name")
        .arg("sales_docs")
        .arg("--description")
        .arg("Sales docs")
        .arg("--context-dir")
        .arg(&context_dir);

    create_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"name\": \"sales_docs\""))
        .stdout(predicate::str::contains("\"tenant_id\": \"tenant_123\""));

    let mut list_123_cmd = Command::cargo_bin("quarry").expect("binary should build");
    list_123_cmd
        .arg("collection")
        .arg("list")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--context-dir")
        .arg(&context_dir);

    list_123_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"collections\""))
        .stdout(predicate::str::contains("\"name\": \"sales_docs\""))
        .stdout(predicate::str::contains("\"tenant_id\": \"tenant_123\""));

    let mut list_999_cmd = Command::cargo_bin("quarry").expect("binary should build");
    list_999_cmd
        .arg("collection")
        .arg("list")
        .arg("--tenant")
        .arg("tenant_999")
        .arg("--context-dir")
        .arg(&context_dir);

    list_999_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"collections\": []"));
}

#[test]
fn collection_create_rejects_duplicates_for_same_tenant() {
    let dir = tempdir().expect("tempdir");
    let context_dir = dir.path().join("context");

    let mut create_cmd = Command::cargo_bin("quarry").expect("binary should build");
    create_cmd
        .arg("collection")
        .arg("create")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--name")
        .arg("sales_docs")
        .arg("--context-dir")
        .arg(&context_dir);
    create_cmd.assert().success();

    let mut duplicate_cmd = Command::cargo_bin("quarry").expect("binary should build");
    duplicate_cmd
        .arg("collection")
        .arg("create")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--name")
        .arg("sales_docs")
        .arg("--context-dir")
        .arg(&context_dir);

    duplicate_cmd
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"CONFIG_ERROR\""))
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn sync_and_search_filesystem_connector_work_end_to_end() {
    let dir = tempdir().expect("tempdir");
    let context_dir = dir.path().join("context");
    let config_path = write_filesystem_sync_config(&dir, "docs");

    let mut create_cmd = Command::cargo_bin("quarry").expect("binary should build");
    create_cmd
        .arg("collection")
        .arg("create")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--name")
        .arg("sales_docs")
        .arg("--context-dir")
        .arg(&context_dir);
    create_cmd.assert().success();

    let mut sync_cmd = Command::cargo_bin("quarry").expect("binary should build");
    sync_cmd
        .arg("sync")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--collection")
        .arg("sales_docs")
        .arg("--connector")
        .arg("filesystem")
        .arg("--config")
        .arg(&config_path)
        .arg("--context-dir")
        .arg(&context_dir);

    sync_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"documents_seen\": 1"))
        .stdout(predicate::str::contains("\"documents_indexed\": 1"));

    let mut search_cmd = Command::cargo_bin("quarry").expect("binary should build");
    search_cmd
        .arg("search")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--collection")
        .arg("sales_docs")
        .arg("--query")
        .arg("revenue")
        .arg("--top-k")
        .arg("5")
        .arg("--context-dir")
        .arg(&context_dir);

    search_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"hits\""))
        .stdout(predicate::str::contains("\"tenant_id\": \"tenant_123\""));
}

#[test]
fn sync_second_run_reports_skipped_documents() {
    let dir = tempdir().expect("tempdir");
    let context_dir = dir.path().join("context");
    let config_path = write_filesystem_sync_config(&dir, "docs");

    let mut create_cmd = Command::cargo_bin("quarry").expect("binary should build");
    create_cmd
        .arg("collection")
        .arg("create")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--name")
        .arg("sales_docs")
        .arg("--context-dir")
        .arg(&context_dir);
    create_cmd.assert().success();

    let mut first_sync = Command::cargo_bin("quarry").expect("binary should build");
    first_sync
        .arg("sync")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--collection")
        .arg("sales_docs")
        .arg("--connector")
        .arg("filesystem")
        .arg("--config")
        .arg(&config_path)
        .arg("--context-dir")
        .arg(&context_dir);
    first_sync.assert().success();

    let mut second_sync = Command::cargo_bin("quarry").expect("binary should build");
    second_sync
        .arg("sync")
        .arg("--tenant")
        .arg("tenant_123")
        .arg("--collection")
        .arg("sales_docs")
        .arg("--connector")
        .arg("filesystem")
        .arg("--config")
        .arg(&config_path)
        .arg("--context-dir")
        .arg(&context_dir);

    second_sync
        .assert()
        .success()
        .stdout(predicate::str::contains("\"documents_skipped\": 1"));
}
