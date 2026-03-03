use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::datasource::empty::EmptyTable;
use datafusion::datasource::provider_as_source;
use datafusion::functions_aggregate::expr_fn::{avg, count, count_distinct, max, min, sum};
use datafusion::logical_expr::TableSource;
use datafusion::logical_expr::{col, lit, Expr, LogicalPlan, LogicalPlanBuilder};

use crate::errors::QuarryCoreError;
use crate::model::{Entity, MeasureAgg, MetricKind, SemanticModel};
use crate::query::{FilterOp, SemanticQuery, SortDirection, TimeGrain};

pub trait EntitySourceProvider {
    fn source_for_entity(&self, entity: &Entity) -> Result<Arc<dyn TableSource>, QuarryCoreError>;
}

pub struct EmptyEntitySourceProvider;

impl EntitySourceProvider for EmptyEntitySourceProvider {
    fn source_for_entity(&self, entity: &Entity) -> Result<Arc<dyn TableSource>, QuarryCoreError> {
        let schema = entity.schema();
        Ok(provider_as_source(Arc::new(EmptyTable::new(schema))))
    }
}

pub fn resolve_to_logical_plan(
    model: &SemanticModel,
    query: &SemanticQuery,
) -> Result<LogicalPlan, QuarryCoreError> {
    resolve_internal(model, query, &EmptyEntitySourceProvider, None)
}

pub fn resolve_to_logical_plan_with_tenant(
    model: &SemanticModel,
    query: &SemanticQuery,
    tenant_id: &str,
) -> Result<LogicalPlan, QuarryCoreError> {
    resolve_internal(
        model,
        query,
        &EmptyEntitySourceProvider,
        Some(tenant_id.to_string()),
    )
}

pub fn resolve_to_logical_plan_with_sources(
    model: &SemanticModel,
    query: &SemanticQuery,
    sources: &dyn EntitySourceProvider,
) -> Result<LogicalPlan, QuarryCoreError> {
    resolve_internal(model, query, sources, None)
}

pub fn resolve_to_logical_plan_with_sources_and_tenant(
    model: &SemanticModel,
    query: &SemanticQuery,
    sources: &dyn EntitySourceProvider,
    tenant_id: &str,
) -> Result<LogicalPlan, QuarryCoreError> {
    resolve_internal(model, query, sources, Some(tenant_id.to_string()))
}

fn resolve_internal(
    model: &SemanticModel,
    query: &SemanticQuery,
    sources: &dyn EntitySourceProvider,
    tenant_id: Option<String>,
) -> Result<LogicalPlan, QuarryCoreError> {
    query.validate(model)?;

    let first_metric = query.metrics.first().ok_or_else(|| {
        QuarryCoreError::Resolution("Query requires at least one metric".to_string())
    })?;

    let metric = model
        .metric_by_name(first_metric)
        .ok_or_else(|| QuarryCoreError::Resolution(format!("Unknown metric '{}'", first_metric)))?;

    let base_entity = model.entity_by_name(&metric.entity).ok_or_else(|| {
        QuarryCoreError::Resolution(format!(
            "Metric '{}' has unknown entity '{}",
            metric.name, metric.entity
        ))
    })?;

    ensure_single_entity_scope(model, query, base_entity.name.as_str())?;

    let source = sources.source_for_entity(base_entity)?;
    let mut builder = LogicalPlanBuilder::scan(base_entity.table.as_str(), source, None)
        .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;

    if let Some(tenant_id) = tenant_id {
        let tenant_col = format!("{}.{}", base_entity.table, base_entity.tenant_column);
        builder = builder
            .filter(col(tenant_col).eq(lit(tenant_id)))
            .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;
    }

    for filter in &query.filters {
        let filter_expr = filter_to_expr(model, base_entity.table.as_str(), filter)?;
        builder = builder
            .filter(filter_expr)
            .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;
    }

    for metric_name in &query.metrics {
        let definition = model.metric_by_name(metric_name).ok_or_else(|| {
            QuarryCoreError::Resolution(format!("Unknown metric '{}'", metric_name))
        })?;

        if let Some(metric_filter) = &definition.filter {
            let op = parse_metric_filter_op(metric_filter.op.as_str())?;
            let filter_expr = build_filter_expr(
                model,
                base_entity.table.as_str(),
                &metric_filter.field,
                op,
                &metric_filter.value,
            )?;

            builder = builder
                .filter(filter_expr)
                .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;
        }
    }

    let group_exprs: Vec<Expr> = query
        .dimensions
        .iter()
        .map(|dimension| {
            let (entity_name, dim_name) = model.parse_ref(&dimension.name).ok_or_else(|| {
                QuarryCoreError::Resolution(format!("Invalid dimension '{}'", dimension.name))
            })?;

            let entity = model.entity_by_name(entity_name).ok_or_else(|| {
                QuarryCoreError::Resolution(format!("Unknown entity '{}'", entity_name))
            })?;

            let dimension_def = entity
                .dimensions
                .iter()
                .find(|entry| entry.name == dim_name)
                .ok_or_else(|| {
                    QuarryCoreError::Resolution(format!("Unknown dimension '{}'", dimension.name))
                })?;

            let qualified = format!("{}.{}", entity.table, dimension_def.column);
            let expr = match dimension.time_grain {
                Some(TimeGrain::Day)
                | Some(TimeGrain::Week)
                | Some(TimeGrain::Month)
                | Some(TimeGrain::Quarter)
                | Some(TimeGrain::Year) => col(&qualified),
                None => col(&qualified),
            };

            Ok(expr.alias(dimension.name.clone()))
        })
        .collect::<Result<Vec<_>, QuarryCoreError>>()?;

    let aggregate_exprs = query
        .metrics
        .iter()
        .map(|metric_name| {
            let metric = model.metric_by_name(metric_name).ok_or_else(|| {
                QuarryCoreError::Resolution(format!("Unknown metric '{}'", metric_name))
            })?;

            if !matches!(metric.kind, MetricKind::Simple) {
                return Err(QuarryCoreError::Unsupported(format!(
                    "Metric '{}' kind '{:?}' is not yet supported in execution",
                    metric.name, metric.kind
                )));
            }

            let entity = model.entity_by_name(&metric.entity).ok_or_else(|| {
                QuarryCoreError::Resolution(format!("Unknown metric entity '{}'", metric.entity))
            })?;

            let measure = entity
                .measures
                .iter()
                .find(|measure| measure.name == metric.measure)
                .ok_or_else(|| {
                    QuarryCoreError::Resolution(format!(
                        "Metric '{}' references unknown measure '{}'",
                        metric.name, metric.measure
                    ))
                })?;

            let qualified = format!("{}.{}", entity.table, measure.column);
            let expr = match measure.agg {
                MeasureAgg::Sum => sum(col(&qualified)),
                MeasureAgg::Count => count(col(&qualified)),
                MeasureAgg::Avg => avg(col(&qualified)),
                MeasureAgg::Min => min(col(&qualified)),
                MeasureAgg::Max => max(col(&qualified)),
                MeasureAgg::CountDistinct => count_distinct(col(&qualified)),
            };

            Ok(expr.alias(metric.name.clone()))
        })
        .collect::<Result<Vec<Expr>, QuarryCoreError>>()?;

    if !group_exprs.is_empty() || !aggregate_exprs.is_empty() {
        builder = builder
            .aggregate(group_exprs.clone(), aggregate_exprs.clone())
            .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;
    }

    if !query.order_by.is_empty() {
        let sort_exprs = query
            .order_by
            .iter()
            .map(|order| {
                let expr = col(order.field.as_str());
                match order.direction {
                    SortDirection::Asc => expr.sort(true, true),
                    SortDirection::Desc => expr.sort(false, true),
                }
            })
            .collect::<Vec<_>>();

        builder = builder
            .sort(sort_exprs)
            .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;
    }

    if let Some(limit) = query.limit {
        builder = builder
            .limit(0, Some(limit as usize))
            .map_err(|error| QuarryCoreError::Resolution(error.to_string()))?;
    }

    builder
        .build()
        .map_err(|error| QuarryCoreError::Resolution(error.to_string()))
}

fn ensure_single_entity_scope(
    model: &SemanticModel,
    query: &SemanticQuery,
    base_entity: &str,
) -> Result<(), QuarryCoreError> {
    for dimension in &query.dimensions {
        let (entity, _) = model.parse_ref(&dimension.name).ok_or_else(|| {
            QuarryCoreError::Resolution(format!("Invalid dimension '{}'", dimension.name))
        })?;

        if entity != base_entity {
            return Err(QuarryCoreError::Unsupported(format!(
                "Cross-entity dimensions are not yet supported in v1 execution path ({} -> {})",
                base_entity, entity
            )));
        }
    }

    for filter in &query.filters {
        let (entity, _) = model.parse_ref(&filter.field).ok_or_else(|| {
            QuarryCoreError::Resolution(format!("Invalid filter field '{}'", filter.field))
        })?;

        if entity != base_entity {
            return Err(QuarryCoreError::Unsupported(format!(
                "Cross-entity filters are not yet supported in v1 execution path ({} -> {})",
                base_entity, entity
            )));
        }
    }

    Ok(())
}

fn parse_metric_filter_op(value: &str) -> Result<FilterOp, QuarryCoreError> {
    match value {
        "eq" => Ok(FilterOp::Eq),
        "neq" => Ok(FilterOp::Neq),
        "gt" => Ok(FilterOp::Gt),
        "gte" => Ok(FilterOp::Gte),
        "lt" => Ok(FilterOp::Lt),
        "lte" => Ok(FilterOp::Lte),
        "in" => Ok(FilterOp::In),
        "between" => Ok(FilterOp::Between),
        _ => Err(QuarryCoreError::Resolution(format!(
            "Unsupported metric filter operator '{}'",
            value
        ))),
    }
}

fn filter_to_expr(
    model: &SemanticModel,
    base_table: &str,
    filter: &crate::query::QueryFilter,
) -> Result<Expr, QuarryCoreError> {
    build_filter_expr(model, base_table, &filter.field, filter.op, &filter.value)
}

fn build_filter_expr(
    model: &SemanticModel,
    _base_table: &str,
    field: &str,
    op: FilterOp,
    value: &serde_json::Value,
) -> Result<Expr, QuarryCoreError> {
    let (entity_name, field_name) = model.parse_ref(field).ok_or_else(|| {
        QuarryCoreError::Resolution(format!("Invalid field reference '{}'", field))
    })?;

    let entity = model
        .entity_by_name(entity_name)
        .ok_or_else(|| QuarryCoreError::Resolution(format!("Unknown entity '{}'", entity_name)))?;

    let column_name =
        if let Some(dimension) = entity.dimensions.iter().find(|dim| dim.name == field_name) {
            dimension.column.clone()
        } else if let Some(measure) = entity
            .measures
            .iter()
            .find(|measure| measure.name == field_name)
        {
            measure.column.clone()
        } else {
            return Err(QuarryCoreError::Resolution(format!(
                "Unknown field '{}' on entity '{}'",
                field_name, entity_name
            )));
        };

    let qualified = format!("{}.{}", entity.table, column_name);
    let col_expr = col(&qualified);

    let expr = match op {
        FilterOp::Eq => col_expr.eq(lit_from_json(value)),
        FilterOp::Neq => col_expr.not_eq(lit_from_json(value)),
        FilterOp::Gt => col_expr.gt(lit_from_json(value)),
        FilterOp::Gte => col_expr.gt_eq(lit_from_json(value)),
        FilterOp::Lt => col_expr.lt(lit_from_json(value)),
        FilterOp::Lte => col_expr.lt_eq(lit_from_json(value)),
        FilterOp::In => {
            let values = value
                .as_array()
                .ok_or_else(|| {
                    QuarryCoreError::Resolution("in operator expects array value".to_string())
                })?
                .iter()
                .map(lit_from_json)
                .collect::<Vec<_>>();
            col_expr.in_list(values, false)
        }
        FilterOp::Between => {
            let arr = value.as_array().ok_or_else(|| {
                QuarryCoreError::Resolution("between operator expects [start, end]".to_string())
            })?;
            if arr.len() != 2 {
                return Err(QuarryCoreError::Resolution(
                    "between operator expects exactly two values".to_string(),
                ));
            }
            col_expr.between(lit_from_json(&arr[0]), lit_from_json(&arr[1]))
        }
    };

    Ok(expr)
}

fn lit_from_json(value: &serde_json::Value) -> Expr {
    match value {
        serde_json::Value::Bool(v) => lit(*v),
        serde_json::Value::Number(num) => {
            if let Some(v) = num.as_i64() {
                lit(v)
            } else if let Some(v) = num.as_u64() {
                lit(v as i64)
            } else if let Some(v) = num.as_f64() {
                lit(v)
            } else {
                lit(num.to_string())
            }
        }
        serde_json::Value::String(v) => lit(v.clone()),
        serde_json::Value::Null => lit(""),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => lit(value.to_string()),
    }
}

pub fn model_schema_for_entity(entity: &Entity) -> SchemaRef {
    entity.schema()
}
