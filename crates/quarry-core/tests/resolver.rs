use quarry_core::{resolve_to_logical_plan, QuarryCoreError, SemanticModel, SemanticQuery};

fn load_model() -> SemanticModel {
    SemanticModel::load_from_path("../../models/example/model.yml").expect("model should load")
}

#[test]
fn resolver_returns_logical_plan_for_single_entity_query() {
    let model = load_model();
    let query = SemanticQuery::parse_json(include_str!("../../../models/example/query.json"))
        .expect("query should parse");

    let plan = resolve_to_logical_plan(&model, &query).expect("plan should build");
    let display = plan.display_indent().to_string();
    assert!(display.contains("Aggregate"));
}

#[test]
fn resolver_rejects_cross_entity_dimension_for_now() {
    let model = load_model();
    let query = SemanticQuery::parse_json(
        r#"
{
  "metrics": ["revenue"],
  "dimensions": [{"name": "customers.region"}],
  "filters": [],
  "order_by": []
}
"#,
    )
    .expect("query should parse");

    let err = resolve_to_logical_plan(&model, &query).expect_err("cross entity should fail");
    match err {
        QuarryCoreError::Unsupported(message) => {
            assert!(message.contains("Cross-entity dimensions"));
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}
