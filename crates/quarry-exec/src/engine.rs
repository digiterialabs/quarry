use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use datafusion::arrow::json::ArrayWriter;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::datasource::provider_as_source;
use datafusion::logical_expr::TableSource;
use datafusion::prelude::SessionContext;
use uuid::Uuid;

use quarry_core::{
    resolve_to_logical_plan_with_sources_and_tenant, EntitySourceProvider, QuarryCoreError,
    SemanticModel, SemanticQuery,
};

use crate::catalog::{adapter_for_kind, CatalogKind, QueryContext};
use crate::error::QuarryExecError;
use crate::result::{ColumnMeta, QueryData, QueryMeta, QuerySuccessEnvelope};

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
    let started = Instant::now();

    let session = SessionContext::new();
    let adapter = adapter_for_kind(catalog_kind);

    adapter
        .register_tables(
            &session,
            semantic_model,
            &QueryContext {
                tenant_id: tenant_id.to_string(),
                local_data_dir: local_data_dir.clone(),
            },
        )
        .await?;

    let sources = build_source_provider(&session, semantic_model).await?;
    let tenant_plan = resolve_to_logical_plan_with_sources_and_tenant(
        semantic_model,
        semantic_query,
        &sources,
        tenant_id,
    )?;

    let generated_sql = tenant_plan.display_indent().to_string();

    let dataframe = session
        .execute_logical_plan(tenant_plan)
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

    let rows = batches_to_json_rows(&batches)?;

    Ok(QuerySuccessEnvelope {
        schema_version: "v1",
        status: "ok",
        data: QueryData { columns, rows },
        meta: QueryMeta {
            row_count: batches.iter().map(|batch| batch.num_rows()).sum(),
            execution_ms: started.elapsed().as_millis(),
            generated_sql,
            tenant_id: tenant_id.to_string(),
            catalog: adapter.name().to_string(),
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
    let session = SessionContext::new();
    let adapter = adapter_for_kind(catalog_kind);

    adapter
        .register_tables(
            &session,
            semantic_model,
            &QueryContext {
                tenant_id: tenant_id.to_string(),
                local_data_dir,
            },
        )
        .await?;

    let sources = build_source_provider(&session, semantic_model).await?;
    let tenant_plan = resolve_to_logical_plan_with_sources_and_tenant(
        semantic_model,
        semantic_query,
        &sources,
        tenant_id,
    )?;
    let rendered_plan = tenant_plan.display_indent().to_string();

    Ok(QuerySuccessEnvelope {
        schema_version: "v1",
        status: "ok",
        data: QueryData {
            columns: vec![ColumnMeta {
                name: "plan".to_string(),
                r#type: "Utf8".to_string(),
            }],
            rows: vec![serde_json::json!({ "plan": rendered_plan })],
        },
        meta: QueryMeta {
            row_count: 1,
            execution_ms: 0,
            generated_sql: rendered_plan.clone(),
            tenant_id: tenant_id.to_string(),
            catalog: adapter.name().to_string(),
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
