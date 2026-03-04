use quarry_core::{QuarryCoreError, SemanticModel};

#[test]
fn valid_model_loads_and_validates() {
    let model = SemanticModel::load_from_path("../../models/example/model.yml");
    assert!(model.is_ok());
}

#[test]
fn duplicate_metric_names_fail_validation() {
    let yaml = r#"
schema_version: v1
entities:
  - name: orders
    table: orders
    tenant_column: tenant_id
    primary_key: id
    dimensions: []
    measures:
      - name: amount
        column: amount
        agg: sum
        data_type: float64
metrics:
  - name: revenue
    kind: simple
    entity: orders
    measure: amount
  - name: revenue
    kind: simple
    entity: orders
    measure: amount
"#;

    let model: SemanticModel = serde_yaml_ng::from_str(yaml).expect("yaml should parse");
    let err = model.validate().expect_err("model should fail");

    match err {
        QuarryCoreError::ModelValidation(issues) => {
            assert!(issues.iter().any(|issue| issue.code == "DUPLICATE_METRIC"));
        }
        other => panic!("expected ModelValidation, got {other:?}"),
    }
}

#[test]
fn iceberg_physical_source_requires_metadata_path() {
    let yaml = r#"
schema_version: v1
entities:
  - name: orders
    table: orders
    physical:
      format: iceberg
    tenant_column: tenant_id
    primary_key: id
    dimensions: []
    measures:
      - name: amount
        column: amount
        agg: sum
        data_type: float64
metrics:
  - name: revenue
    kind: simple
    entity: orders
    measure: amount
"#;

    let model: SemanticModel = serde_yaml_ng::from_str(yaml).expect("yaml should parse");
    let err = model.validate().expect_err("model should fail");

    match err {
        QuarryCoreError::ModelValidation(issues) => {
            assert!(issues
                .iter()
                .any(|issue| issue.code == "MISSING_ICEBERG_METADATA_PATH"));
        }
        other => panic!("expected ModelValidation, got {other:?}"),
    }
}
