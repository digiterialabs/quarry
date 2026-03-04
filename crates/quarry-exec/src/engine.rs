use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use datafusion::arrow::json::ArrayWriter;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::datasource::provider_as_source;
use datafusion::logical_expr::TableSource;
use datafusion::physical_plan::displayable;
use datafusion::prelude::SessionContext;
use uuid::Uuid;

use quarry_core::{
    resolve_to_logical_plan_with_sources_and_tenant, EntitySourceProvider, QuarryCoreError,
    SemanticModel, SemanticQuery,
};

use crate::catalog::{adapter_for_kind, CatalogKind, QueryContext};
use crate::error::QuarryExecError;
use crate::result::{ColumnMeta, QueryData, QueryMeta, QuerySuccessEnvelope, TableBindingMeta};

struct SessionEntitySourceProvider {
    sources_by_entity: HashMap<String, Arc<dyn TableSource>>,
}

impl EntitySourceProvider for SessionEntitySourceProvider {
    fn source_for_entity(
        &self,
        entity: &quarry_core::model::Entity,
    ) -> Result<Arc<dyn TableSource>, QuarryCoreError> {
        self.sources_by_entity
            .get(&entity.name)
            .cloned()
            .ok_or_else(|| {
                QuarryCoreError::Resolution(format!(
                    "No registered source found for entity '{}'",
                    entity.name
                ))
            })
    }
}

pub async fn execute_query(
    semantic_model: &SemanticModel,
    semantic_query: &SemanticQuery,
    catalog_kind: CatalogKind,
    tenant_id: &str,
    local_data_dir: Option<PathBuf>,
) -> Result<QuerySuccessEnvelope, QuarryExecError> {
    let request_id = Uuid::new_v4().to_string();
    let sandbox_id = Uuid::new_v4().to_string();

    let session = SessionContext::new();
    let adapter = adapter_for_kind(catalog_kind);

    let query_context = QueryContext {
        tenant_id: tenant_id.to_string(),
        sandbox_id: sandbox_id.clone(),
        local_data_dir: local_data_dir.clone(),
        iceberg_io_props: load_iceberg_io_props_from_env()?,
    };

    let registration = adapter
        .register_tables(&session, semantic_model, &query_context)
        .await?;

    let sources = build_source_provider(&session, semantic_model).await?;

    let planning_started = Instant::now();
    let logical_plan = resolve_to_logical_plan_with_sources_and_tenant(
        semantic_model,
        semantic_query,
        &sources,
        tenant_id,
    )?;
    let planning_ms = planning_started.elapsed().as_millis();

    let logical_plan_rendered = logical_plan.display_indent().to_string();
    let logical_plan_hash = hash_plan(&logical_plan_rendered);

    let optimization_started = Instant::now();
    let optimized_plan = session
        .state()
        .optimize(&logical_plan)
        .map_err(|error| QuarryExecError::Execution(error.to_string()))?;
    let optimization_ms = optimization_started.elapsed().as_millis();

    let optimized_plan_rendered = optimized_plan.display_indent().to_string();
    let optimized_plan_hash = hash_plan(&optimized_plan_rendered);

    let physical_planning_started = Instant::now();
    let physical_plan = session
        .state()
        .create_physical_plan(&optimized_plan)
        .await
        .map_err(|error| QuarryExecError::Execution(error.to_string()))?;
    let physical_planning_ms = physical_planning_started.elapsed().as_millis();

    let physical_plan_rendered = format!("{}", displayable(physical_plan.as_ref()).indent(true));
    let physical_plan_hash = hash_plan(&physical_plan_rendered);

    let execution_started = Instant::now();
    let dataframe = session
        .execute_logical_plan(optimized_plan)
        .await
        .map_err(|error| QuarryExecError::Execution(error.to_string()))?;

    let schema = dataframe.schema();
    let columns = schema
        .fields()
        .iter()
        .map(|field| ColumnMeta {
            name: field.name().clone(),
            r#type: format!("{:?}", field.data_type()),
        })
        .collect::<Vec<_>>();

    let batches = dataframe
        .collect()
        .await
        .map_err(|error| QuarryExecError::Execution(error.to_string()))?;
    let execution_ms = execution_started.elapsed().as_millis();

    let rows = batches_to_json_rows(&batches)?;

    Ok(QuerySuccessEnvelope {
        schema_version: "v1",
        status: "ok",
        data: QueryData { columns, rows },
        meta: QueryMeta {
            row_count: batches.iter().map(|batch| batch.num_rows()).sum(),
            planning_ms,
            optimization_ms,
            physical_planning_ms,
            execution_ms,
            generated_sql: logical_plan_rendered,
            optimized_plan: optimized_plan_rendered,
            physical_plan: physical_plan_rendered,
            logical_plan_hash,
            optimized_plan_hash,
            physical_plan_hash,
            tenant_id: query_context.tenant_id,
            catalog: adapter.name().to_string(),
            sandbox_id: query_context.sandbox_id,
            execution_mode: "ephemeral_per_query".to_string(),
            table_bindings: registration
                .table_bindings
                .into_iter()
                .map(|binding| TableBindingMeta {
                    entity: binding.entity,
                    table: binding.table,
                    source: binding.source,
                })
                .collect(),
            request_id,
        },
    })
}

pub async fn explain_query(
    semantic_model: &SemanticModel,
    semantic_query: &SemanticQuery,
    catalog_kind: CatalogKind,
    tenant_id: &str,
    local_data_dir: Option<PathBuf>,
) -> Result<QuerySuccessEnvelope, QuarryExecError> {
    let request_id = Uuid::new_v4().to_string();
    let sandbox_id = Uuid::new_v4().to_string();

    let session = SessionContext::new();
    let adapter = adapter_for_kind(catalog_kind);

    let query_context = QueryContext {
        tenant_id: tenant_id.to_string(),
        sandbox_id: sandbox_id.clone(),
        local_data_dir,
        iceberg_io_props: load_iceberg_io_props_from_env()?,
    };

    let registration = adapter
        .register_tables(&session, semantic_model, &query_context)
        .await?;

    let sources = build_source_provider(&session, semantic_model).await?;

    let planning_started = Instant::now();
    let logical_plan = resolve_to_logical_plan_with_sources_and_tenant(
        semantic_model,
        semantic_query,
        &sources,
        tenant_id,
    )?;
    let planning_ms = planning_started.elapsed().as_millis();

    let logical_plan_rendered = logical_plan.display_indent().to_string();
    let logical_plan_hash = hash_plan(&logical_plan_rendered);

    let optimization_started = Instant::now();
    let optimized_plan = session
        .state()
        .optimize(&logical_plan)
        .map_err(|error| QuarryExecError::Execution(error.to_string()))?;
    let optimization_ms = optimization_started.elapsed().as_millis();

    let optimized_plan_rendered = optimized_plan.display_indent().to_string();
    let optimized_plan_hash = hash_plan(&optimized_plan_rendered);

    let physical_planning_started = Instant::now();
    let physical_plan = session
        .state()
        .create_physical_plan(&optimized_plan)
        .await
        .map_err(|error| QuarryExecError::Execution(error.to_string()))?;
    let physical_planning_ms = physical_planning_started.elapsed().as_millis();

    let physical_plan_rendered = format!("{}", displayable(physical_plan.as_ref()).indent(true));
    let physical_plan_hash = hash_plan(&physical_plan_rendered);

    Ok(QuerySuccessEnvelope {
        schema_version: "v1",
        status: "ok",
        data: QueryData {
            columns: vec![ColumnMeta {
                name: "plan".to_string(),
                r#type: "Utf8".to_string(),
            }],
            rows: vec![serde_json::json!({
                "logical_plan": logical_plan_rendered,
                "optimized_plan": optimized_plan_rendered,
                "physical_plan": physical_plan_rendered
            })],
        },
        meta: QueryMeta {
            row_count: 1,
            planning_ms,
            optimization_ms,
            physical_planning_ms,
            execution_ms: 0,
            generated_sql: logical_plan_rendered,
            optimized_plan: optimized_plan_rendered,
            physical_plan: physical_plan_rendered,
            logical_plan_hash,
            optimized_plan_hash,
            physical_plan_hash,
            tenant_id: query_context.tenant_id,
            catalog: adapter.name().to_string(),
            sandbox_id: query_context.sandbox_id,
            execution_mode: "ephemeral_per_query".to_string(),
            table_bindings: registration
                .table_bindings
                .into_iter()
                .map(|binding| TableBindingMeta {
                    entity: binding.entity,
                    table: binding.table,
                    source: binding.source,
                })
                .collect(),
            request_id,
        },
    })
}

async fn build_source_provider(
    session: &SessionContext,
    semantic_model: &SemanticModel,
) -> Result<SessionEntitySourceProvider, QuarryExecError> {
    let mut sources_by_entity = HashMap::new();

    for entity in &semantic_model.entities {
        let provider = session
            .table_provider(entity.table.as_str())
            .await
            .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
        sources_by_entity.insert(entity.name.clone(), provider_as_source(provider));
    }

    Ok(SessionEntitySourceProvider { sources_by_entity })
}

fn batches_to_json_rows(
    batches: &[RecordBatch],
) -> Result<Vec<serde_json::Value>, QuarryExecError> {
    let mut buf = Vec::new();
    {
        let mut writer = ArrayWriter::new(&mut buf);
        for batch in batches {
            writer
                .write(batch)
                .map_err(|error| QuarryExecError::Serialization(error.to_string()))?;
        }
        writer
            .finish()
            .map_err(|error| QuarryExecError::Serialization(error.to_string()))?;
    }

    let parsed: serde_json::Value = serde_json::from_slice(&buf)
        .map_err(|error| QuarryExecError::Serialization(error.to_string()))?;

    match parsed {
        serde_json::Value::Array(rows) => Ok(rows),
        _ => Err(QuarryExecError::Serialization(
            "arrow JSON writer did not produce an array".to_string(),
        )),
    }
}

fn load_iceberg_io_props_from_env() -> Result<HashMap<String, String>, QuarryExecError> {
    let mut props = HashMap::new();

    copy_env_prop(&mut props, "s3.region", "AWS_REGION");
    copy_env_prop(&mut props, "s3.endpoint", "QUARRY_S3_ENDPOINT");
    copy_env_prop(&mut props, "s3.access-key-id", "AWS_ACCESS_KEY_ID");
    copy_env_prop(&mut props, "s3.secret-access-key", "AWS_SECRET_ACCESS_KEY");
    copy_env_prop(&mut props, "s3.session-token", "AWS_SESSION_TOKEN");
    copy_env_prop(
        &mut props,
        "s3.path-style-access",
        "QUARRY_S3_PATH_STYLE_ACCESS",
    );

    if let Ok(raw_json) = env::var("QUARRY_ICEBERG_IO_PROPS_JSON") {
        if !raw_json.trim().is_empty() {
            let parsed: serde_json::Value =
                serde_json::from_str(raw_json.as_str()).map_err(|error| {
                    QuarryExecError::Config(format!(
                        "QUARRY_ICEBERG_IO_PROPS_JSON must be a JSON object: {error}"
                    ))
                })?;

            let Some(map) = parsed.as_object() else {
                return Err(QuarryExecError::Config(
                    "QUARRY_ICEBERG_IO_PROPS_JSON must be a JSON object".to_string(),
                ));
            };

            for (key, value) in map {
                if let Some(as_string) = value.as_str() {
                    props.insert(key.clone(), as_string.to_string());
                }
            }
        }
    }

    Ok(props)
}

fn copy_env_prop(props: &mut HashMap<String, String>, target_key: &str, env_key: &str) {
    if let Ok(value) = env::var(env_key) {
        if !value.trim().is_empty() {
            props.insert(target_key.to_string(), value);
        }
    }
}

fn hash_plan(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
