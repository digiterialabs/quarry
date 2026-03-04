use serde::{Deserialize, Serialize};

use crate::errors::{QuarryCoreError, ValidationIssue};
use crate::model::{DimensionKind, SemanticModel};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SemanticQuery {
    pub metrics: Vec<String>,
    pub dimensions: Vec<QueryDimension>,
    pub filters: Vec<QueryFilter>,
    pub limit: Option<u64>,
    pub order_by: Vec<OrderBy>,
}

impl Default for SemanticQuery {
    fn default() -> Self {
        Self {
            metrics: Vec::new(),
            dimensions: Vec::new(),
            filters: Vec::new(),
            limit: Some(1000),
            order_by: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDimension {
    pub name: String,
    #[serde(default)]
    pub time_grain: Option<TimeGrain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Between,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBy {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeGrain {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl SemanticQuery {
    pub fn parse_json(input: &str) -> Result<Self, QuarryCoreError> {
        serde_json::from_str(input).map_err(|error| {
            QuarryCoreError::QueryValidation(vec![ValidationIssue {
                code: "INVALID_QUERY_JSON",
                path: "$".to_string(),
                message: format!("Invalid query JSON: {error}"),
                suggestions: vec!["Ensure query matches the SemanticQuery v1 schema".to_string()],
            }])
        })
    }

    pub fn validate(&self, model: &SemanticModel) -> Result<(), QuarryCoreError> {
        let mut issues = Vec::new();

        if self.metrics.is_empty() {
            issues.push(ValidationIssue {
                code: "EMPTY_METRICS",
                path: "metrics".to_string(),
                message: "At least one metric is required".to_string(),
                suggestions: model
                    .metrics
                    .iter()
                    .map(|metric| metric.name.clone())
                    .collect(),
            });
        }

        let mut metric_entity: Option<&str> = None;
        for metric_name in &self.metrics {
            let Some(metric) = model.metric_by_name(metric_name) else {
                issues.push(ValidationIssue {
                    code: "UNKNOWN_METRIC",
                    path: format!("metrics.{}", metric_name),
                    message: format!("Unknown metric '{}'", metric_name),
                    suggestions: model
                        .metrics
                        .iter()
                        .map(|entry| entry.name.clone())
                        .collect(),
                });
                continue;
            };

            if metric_entity.is_none() {
                metric_entity = Some(metric.entity.as_str());
            } else if metric_entity != Some(metric.entity.as_str()) {
                issues.push(ValidationIssue {
                    code: "CROSS_ENTITY_METRICS",
                    path: "metrics".to_string(),
                    message: "All metrics in v1 must belong to the same base entity".to_string(),
                    suggestions: vec![
                        "Split this into multiple queries, one per base entity".to_string()
                    ],
                });
            }
        }

        for dimension in &self.dimensions {
            let Some((entity_name, dimension_name)) = model.parse_ref(&dimension.name) else {
                issues.push(ValidationIssue {
                    code: "INVALID_DIMENSION_REF",
                    path: format!("dimensions.{}.name", dimension.name),
                    message: format!(
                        "Dimension reference '{}' must be <entity>.<dimension>",
                        dimension.name
                    ),
                    suggestions: vec!["Example: orders.created_at".to_string()],
                });
                continue;
            };

            let Some(definition) = model.entity_dimension(entity_name, dimension_name) else {
                issues.push(ValidationIssue {
                    code: "UNKNOWN_DIMENSION",
                    path: format!("dimensions.{}.name", dimension.name),
                    message: format!("Unknown dimension '{}'", dimension.name),
                    suggestions: model
                        .entities
                        .iter()
                        .flat_map(|entity| {
                            entity
                                .dimensions
                                .iter()
                                .map(move |dim| format!("{}.{}", entity.name, dim.name))
                        })
                        .collect(),
                });
                continue;
            };

            if dimension.time_grain.is_some() && !matches!(definition.kind, DimensionKind::Temporal)
            {
                issues.push(ValidationIssue {
                    code: "INVALID_TIME_GRAIN",
                    path: format!("dimensions.{}.time_grain", dimension.name),
                    message: format!(
                        "Time grain is only valid for temporal dimensions (got '{}')",
                        dimension.name
                    ),
                    suggestions: vec!["Remove time_grain or use a temporal dimension".to_string()],
                });
            }
        }

        for filter in &self.filters {
            let Some((entity_name, field_name)) = model.parse_ref(&filter.field) else {
                issues.push(ValidationIssue {
                    code: "INVALID_FILTER_REF",
                    path: format!("filters.{}.field", filter.field),
                    message: format!("Filter field '{}' must be <entity>.<field>", filter.field),
                    suggestions: vec!["Example: orders.status".to_string()],
                });
                continue;
            };

            let known_dimension = model.entity_dimension(entity_name, field_name).is_some();
            let known_measure = model.entity_measure(entity_name, field_name).is_some();
            if !known_dimension && !known_measure {
                issues.push(ValidationIssue {
                    code: "UNKNOWN_FILTER_FIELD",
                    path: format!("filters.{}.field", filter.field),
                    message: format!("Unknown filter field '{}'", filter.field),
                    suggestions: vec!["Use <entity>.<dimension> or <entity>.<measure>".to_string()],
                });
            }

            if matches!(filter.op, FilterOp::Between)
                && !matches!(&filter.value, serde_json::Value::Array(v) if v.len() == 2)
            {
                issues.push(ValidationIssue {
                    code: "INVALID_BETWEEN",
                    path: format!("filters.{}.value", filter.field),
                    message: "between filter value must be [start, end]".to_string(),
                    suggestions: vec!["Example: [\"2025-01-01\", \"2025-12-31\"]".to_string()],
                });
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(QuarryCoreError::QueryValidation(issues))
        }
    }
}
