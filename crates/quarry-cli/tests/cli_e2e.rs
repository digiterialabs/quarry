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
