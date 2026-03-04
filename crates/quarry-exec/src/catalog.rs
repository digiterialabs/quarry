use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::datasource::MemTable;
use datafusion::prelude::{CsvReadOptions, ParquetReadOptions, SessionContext};

use quarry_core::SemanticModel;

use crate::error::QuarryExecError;

#[derive(Debug, Clone, Copy)]
pub enum CatalogKind {
    Local,
    Glue,
}

impl CatalogKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Glue => "glue",
        }
    }
}

impl std::str::FromStr for CatalogKind {
    type Err = QuarryExecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local" => Ok(Self::Local),
            "glue" => Ok(Self::Glue),
            _ => Err(QuarryExecError::Config(format!(
                "Unsupported catalog '{}'; expected local|glue",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryContext {
    pub tenant_id: String,
    pub local_data_dir: Option<PathBuf>,
}

#[async_trait]
pub trait CatalogAdapter: Send + Sync {
    async fn register_tables(
        &self,
        session: &SessionContext,
        semantic_model: &SemanticModel,
        query_context: &QueryContext,
    ) -> Result<(), QuarryExecError>;

    fn name(&self) -> &'static str;
}

#[derive(Debug, Default)]
pub struct LocalCatalogAdapter;

#[async_trait]
impl CatalogAdapter for LocalCatalogAdapter {
    async fn register_tables(
        &self,
        session: &SessionContext,
        semantic_model: &SemanticModel,
        query_context: &QueryContext,
    ) -> Result<(), QuarryExecError> {
        for entity in &semantic_model.entities {
            if let Some(data_dir) = &query_context.local_data_dir {
                let csv_path = data_dir.join(format!("{}.csv", entity.table));
                let parquet_path = data_dir.join(format!("{}.parquet", entity.table));

                if csv_path.exists() {
                    session
                        .register_csv(
                            entity.table.as_str(),
                            csv_path.to_string_lossy().as_ref(),
                            CsvReadOptions::new().has_header(true),
                        )
                        .await
                        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
                    continue;
                }

                if parquet_path.exists() {
                    session
                        .register_parquet(
                            entity.table.as_str(),
                            parquet_path.to_string_lossy().as_ref(),
                            ParquetReadOptions::default(),
                        )
                        .await
                        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
                    continue;
                }

                return Err(QuarryExecError::Config(format!(
                    "No source file for table '{}' in '{}'. Expected '{}' or '{}'",
                    entity.table,
                    data_dir.display(),
                    csv_path.display(),
                    parquet_path.display()
                )));
            } else {
                let schema = entity.schema();
                let batch = RecordBatch::new_empty(schema.clone());
                let table = MemTable::try_new(schema, vec![vec![batch]])
                    .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

                session
                    .register_table(entity.table.as_str(), Arc::new(table))
                    .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "local"
    }
}

#[derive(Debug, Default)]
pub struct GlueCatalogAdapter;

#[async_trait]
impl CatalogAdapter for GlueCatalogAdapter {
    async fn register_tables(
        &self,
        session: &SessionContext,
        semantic_model: &SemanticModel,
        query_context: &QueryContext,
    ) -> Result<(), QuarryExecError> {
        let _ = std::env::var("AWS_REGION").map_err(|_| {
            QuarryExecError::Config(
                "Glue catalog requires AWS_REGION in environment for v1 execution".to_string(),
            )
        })?;

        // v1 uses schema-driven empty table registration for deterministic testability.
        // Real Iceberg/Glue provider wiring remains behind this adapter boundary.
        LocalCatalogAdapter
            .register_tables(session, semantic_model, query_context)
            .await
    }

    fn name(&self) -> &'static str {
        "glue"
    }
}

pub fn adapter_for_kind(kind: CatalogKind) -> Box<dyn CatalogAdapter> {
    match kind {
        CatalogKind::Local => Box::<LocalCatalogAdapter>::default(),
        CatalogKind::Glue => Box::<GlueCatalogAdapter>::default(),
    }
}

// Keep explicit references to pinned crates to make version intent visible at compile time.
#[allow(dead_code)]
fn _iceberg_version_anchor() {
    let _ = std::any::type_name::<iceberg::table::Table>();
    let _ = std::any::type_name::<iceberg_datafusion::table::IcebergTableProvider>();
}
