use quarry_core::{QuarryCoreError, SemanticModel, SemanticQuery};

fn load_model() -> SemanticModel {
    SemanticModel::load_from_path("../../models/example/model.yml").expect("model should load")
}

#[test]
fn valid_query_passes_validation() {
    let model = load_model();
    let query = SemanticQuery::parse_json(include_str!("../../../models/example/query.json"))
        .expect("query should parse");

    query.validate(&model).expect("query should validate");
}

#[test]
fn unknown_metric_fails_validation() {
    let model = load_model();
    let query = SemanticQuery::parse_json(
        r#"{"metrics":["not_a_metric"],"dimensions":[],"filters":[],"order_by":[]}"#,
    )
    .expect("query should parse");

    let err = query
        .validate(&model)
        .expect_err("query should fail validation");

    match err {
        QuarryCoreError::QueryValidation(issues) => {
            assert!(issues.iter().any(|issue| issue.code == "UNKNOWN_METRIC"));
        }
        other => panic!("expected QueryValidation, got {other:?}"),
    }
}
