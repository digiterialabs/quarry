use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use quarry_core::query::{FilterOp, QueryDimension, QueryFilter};
use quarry_core::{
    PreAggregationDefinition, PreAggregationRefreshMode, SemanticModel, SemanticQuery,
};

use crate::catalog::CatalogKind;
use crate::engine::execute_query;
use crate::error::QuarryExecError;

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationMatch {
    pub name: String,
    pub score: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreAggregationState {
    pub name: String,
    pub tenant_id: String,
    pub row_count: usize,
    pub last_materialized_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
    pub source_plan_hash: u64,
    pub refresh_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeResult {
    pub status: String,
    pub reason: String,
    pub state: Option<PreAggregationState>,
}

#[derive(Debug, Default)]
pub struct PreAggregationStore {
    entries: HashMap<String, PreAggregationState>,
}

impl PreAggregationStore {
    pub fn list(&self, tenant_id: Option<&str>) -> Vec<PreAggregationState> {
        let mut out = self
            .entries
            .values()
            .filter(|entry| {
                tenant_id
                    .map(|tenant| tenant == entry.tenant_id)
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.name.cmp(&b.name).then(a.tenant_id.cmp(&b.tenant_id)));
        out
    }

    pub fn invalidate(&mut self, tenant_id: Option<&str>, pre_aggregation: Option<&str>) -> usize {
        let mut keys_to_remove = Vec::new();
        for (key, state) in &self.entries {
            let tenant_match = tenant_id
                .map(|tenant| tenant == state.tenant_id)
                .unwrap_or(true);
            let preagg_match = pre_aggregation
                .map(|name| name == state.name)
                .unwrap_or(true);

            if tenant_match && preagg_match {
                keys_to_remove.push(key.clone());
            }
        }

        let removed = keys_to_remove.len();
        for key in keys_to_remove {
            self.entries.remove(&key);
        }
        removed
    }

    fn get(&self, tenant_id: &str, pre_aggregation: &str) -> Option<&PreAggregationState> {
        self.entries.get(&store_key(tenant_id, pre_aggregation))
    }

    fn upsert(&mut self, state: PreAggregationState) {
        self.entries.insert(
            store_key(state.tenant_id.as_str(), state.name.as_str()),
            state,
        );
    }
}

pub fn match_pre_aggregation(
    model: &SemanticModel,
    query: &SemanticQuery,
) -> Option<PreAggregationMatch> {
    let query_metrics = query.metrics.iter().cloned().collect::<HashSet<_>>();
    let query_dimensions = query
        .dimensions
        .iter()
        .map(|dimension| dimension.name.clone())
        .collect::<HashSet<_>>();
    let query_filters = query
        .filters
        .iter()
        .map(normalize_query_filter)
        .collect::<HashSet<_>>();

    let mut best: Option<PreAggregationMatch> = None;

    for pre_aggregation in &model.pre_aggregations {
        let preagg_metrics = pre_aggregation
            .metrics
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        if !preagg_metrics.is_superset(&query_metrics) {
            continue;
        }

        let preagg_dimensions = pre_aggregation
            .dimensions
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        if !preagg_dimensions.is_superset(&query_dimensions) {
            continue;
        }

        let preagg_filters = pre_aggregation
            .filters
            .iter()
            .map(normalize_model_filter)
            .collect::<HashSet<_>>();

        let metric_score = if preagg_metrics == query_metrics {
            50
        } else {
            20
        };
        let dimension_score = if preagg_dimensions == query_dimensions {
            30
        } else {
            10
        };
        let filter_score = if preagg_filters == query_filters {
            20
        } else if preagg_filters.is_empty() {
            5
        } else {
            0
        };

        let score = metric_score + dimension_score + filter_score;
        let reason = format!(
            "metrics:{} dimensions:{} filters:{}",
            metric_score, dimension_score, filter_score
        );

        let candidate = PreAggregationMatch {
            name: pre_aggregation.name.clone(),
            score,
            reason,
        };

        if best
            .as_ref()
            .map(|entry| entry.score < candidate.score)
            .unwrap_or(true)
        {
            best = Some(candidate);
        }
    }

    best
}

pub async fn materialize_pre_aggregation(
    model: &SemanticModel,
    pre_aggregation: &PreAggregationDefinition,
    catalog_kind: CatalogKind,
    tenant_id: &str,
    local_data_dir: Option<PathBuf>,
    store: &mut PreAggregationStore,
    force: bool,
) -> Result<MaterializeResult, QuarryExecError> {
    let now = now_unix_ms();

    if !force {
        if let Some(existing) = store.get(tenant_id, pre_aggregation.name.as_str()) {
            match pre_aggregation.refresh.mode {
                PreAggregationRefreshMode::Manual => {
                    return Ok(MaterializeResult {
                        status: "skipped".to_string(),
                        reason: "manual refresh policy already materialized".to_string(),
                        state: Some(existing.clone()),
                    });
                }
                PreAggregationRefreshMode::Interval => {
                    if let Some(expires_at) = existing.expires_at_unix_ms {
                        if now < expires_at {
                            return Ok(MaterializeResult {
                                status: "skipped".to_string(),
                                reason: "interval refresh not due yet".to_string(),
                                state: Some(existing.clone()),
                            });
                        }
                    }
                }
            }
        }
    }

    let query = pre_aggregation_to_query(pre_aggregation)?;
    let result = execute_query(model, &query, catalog_kind, tenant_id, local_data_dir).await?;

    let expires_at = match pre_aggregation.refresh.mode {
        PreAggregationRefreshMode::Manual => None,
        PreAggregationRefreshMode::Interval => Some(
            now.saturating_add(
                pre_aggregation
                    .refresh
                    .interval_seconds
                    .saturating_mul(1000),
            ),
        ),
    };

    let state = PreAggregationState {
        name: pre_aggregation.name.clone(),
        tenant_id: tenant_id.to_string(),
        row_count: result.meta.row_count,
        last_materialized_unix_ms: now,
        expires_at_unix_ms: expires_at,
        source_plan_hash: result.meta.optimized_plan_hash,
        refresh_mode: match pre_aggregation.refresh.mode {
            PreAggregationRefreshMode::Manual => "manual",
            PreAggregationRefreshMode::Interval => "interval",
        }
        .to_string(),
    };

    store.upsert(state.clone());

    Ok(MaterializeResult {
        status: "materialized".to_string(),
        reason: "pre-aggregation executed and cached".to_string(),
        state: Some(state),
    })
}

pub fn pre_aggregation_to_query(
    pre_aggregation: &PreAggregationDefinition,
) -> Result<SemanticQuery, QuarryExecError> {
    let mut filters = Vec::new();
    for filter in &pre_aggregation.filters {
        filters.push(QueryFilter {
            field: filter.field.clone(),
            op: parse_filter_op(filter.op.as_str())?,
            value: filter.value.clone(),
        });
    }

    Ok(SemanticQuery {
        metrics: pre_aggregation.metrics.clone(),
        dimensions: pre_aggregation
            .dimensions
            .iter()
            .map(|name| QueryDimension {
                name: name.clone(),
                time_grain: None,
            })
            .collect(),
        filters,
        limit: None,
        order_by: Vec::new(),
    })
}

fn parse_filter_op(op: &str) -> Result<FilterOp, QuarryExecError> {
    match op {
        "eq" => Ok(FilterOp::Eq),
        "neq" => Ok(FilterOp::Neq),
        "gt" => Ok(FilterOp::Gt),
        "gte" => Ok(FilterOp::Gte),
        "lt" => Ok(FilterOp::Lt),
        "lte" => Ok(FilterOp::Lte),
        "in" => Ok(FilterOp::In),
        "between" => Ok(FilterOp::Between),
        _ => Err(QuarryExecError::Config(format!(
            "Unsupported pre-aggregation filter operator '{}'",
            op
        ))),
    }
}

fn normalize_query_filter(filter: &QueryFilter) -> String {
    format!(
        "{}:{}:{}",
        filter.field,
        match filter.op {
            FilterOp::Eq => "eq",
            FilterOp::Neq => "neq",
            FilterOp::Gt => "gt",
            FilterOp::Gte => "gte",
            FilterOp::Lt => "lt",
            FilterOp::Lte => "lte",
            FilterOp::In => "in",
            FilterOp::Between => "between",
        },
        filter.value
    )
}

fn normalize_model_filter(filter: &quarry_core::model::MetricFilter) -> String {
    format!(
        "{}:{}:{}",
        filter.field,
        filter.op.to_lowercase(),
        filter.value
    )
}

fn store_key(tenant_id: &str, pre_aggregation: &str) -> String {
    format!("{}:{}", tenant_id, pre_aggregation)
}

fn now_unix_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(u128::from(u64::MAX)) as u64,
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quarry_core::SemanticModel;

    fn model() -> SemanticModel {
        SemanticModel::load_from_path("../../models/example/model.yml").expect("model")
    }

    #[test]
    fn matcher_prefers_exact_metric_and_dimension_match() {
        let model = model();
        let query =
            SemanticQuery::parse_json(include_str!("../../../models/example/query_by_region.json"))
                .expect("query should parse");

        let matched = match_pre_aggregation(&model, &query).expect("should match pre-aggregation");
        assert_eq!(matched.name, "revenue_by_region_completed");
    }
}
