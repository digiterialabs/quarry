use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::datasource::MemTable;
use datafusion::prelude::{CsvReadOptions, ParquetReadOptions, SessionContext};
use iceberg::io::FileIO;
use iceberg::table::StaticTable;
use iceberg::TableIdent;
use iceberg_datafusion::IcebergStaticTableProvider;

use quarry_core::model::PhysicalFormat;
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
    pub sandbox_id: String,
    pub local_data_dir: Option<PathBuf>,
    pub iceberg_io_props: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct CatalogTableBinding {
    pub entity: String,
    pub table: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct CatalogRegistration {
    pub table_bindings: Vec<CatalogTableBinding>,
}

#[async_trait]
pub trait CatalogAdapter: Send + Sync {
    async fn register_tables(
        &self,
        session: &SessionContext,
        semantic_model: &SemanticModel,
        query_context: &QueryContext,
    ) -> Result<CatalogRegistration, QuarryExecError>;

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
    ) -> Result<CatalogRegistration, QuarryExecError> {
        let mut registration = CatalogRegistration::default();

        for entity in &semantic_model.entities {
            if let Some(source) =
                register_from_local_data_dir(session, entity, query_context).await?
            {
                registration.table_bindings.push(CatalogTableBinding {
                    entity: entity.name.clone(),
                    table: entity.table.clone(),
                    source,
                });
                continue;
            }

            if let Some(source) =
                register_from_physical_source(session, entity, query_context).await?
            {
                registration.table_bindings.push(CatalogTableBinding {
                    entity: entity.name.clone(),
                    table: entity.table.clone(),
                    source,
                });
                continue;
            }

            if query_context.local_data_dir.is_some() {
                let data_dir = query_context
                    .local_data_dir
                    .as_ref()
                    .expect("local_data_dir was checked");
                return Err(QuarryExecError::Config(format!(
                    "No source file for table '{}' in '{}'. Provide '{}'/'{}' or configure entities.{}.physical",
                    entity.table,
                    data_dir.display(),
                    data_dir.join(format!("{}.csv", entity.table)).display(),
                    data_dir.join(format!("{}.parquet", entity.table)).display(),
                    entity.name
                )));
            }

            // Deterministic fallback mode for contract tests and dry runs.
            let schema = entity.schema();
            let batch = RecordBatch::new_empty(schema.clone());
            let table = MemTable::try_new(schema, vec![vec![batch]])
                .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

            session
                .register_table(entity.table.as_str(), Arc::new(table))
                .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

            registration.table_bindings.push(CatalogTableBinding {
                entity: entity.name.clone(),
                table: entity.table.clone(),
                source: "in_memory_empty".to_string(),
            });
        }

        Ok(registration)
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
    ) -> Result<CatalogRegistration, QuarryExecError> {
        let aws_region = std::env::var("AWS_REGION").map_err(|_| {
            QuarryExecError::Config(
                "Glue catalog requires AWS_REGION in environment for execution".to_string(),
            )
        })?;

        // Glue path currently uses static Iceberg table loading and AWS credentials from environment.
        // This enforces catalog-level auth config while keeping local deterministic test behavior.
        let mut ctx = query_context.clone();
        ctx.iceberg_io_props
            .entry("s3.region".to_string())
            .or_insert(aws_region);

        LocalCatalogAdapter
            .register_tables(session, semantic_model, &ctx)
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

async fn register_from_local_data_dir(
    session: &SessionContext,
    entity: &quarry_core::model::Entity,
    query_context: &QueryContext,
) -> Result<Option<String>, QuarryExecError> {
    let Some(data_dir) = &query_context.local_data_dir else {
        return Ok(None);
    };

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
        return Ok(Some(format!("csv:{}", csv_path.display())));
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
        return Ok(Some(format!("parquet:{}", parquet_path.display())));
    }

    Ok(None)
}

async fn register_from_physical_source(
    session: &SessionContext,
    entity: &quarry_core::model::Entity,
    query_context: &QueryContext,
) -> Result<Option<String>, QuarryExecError> {
    let Some(physical) = &entity.physical else {
        return Ok(None);
    };

    match physical.format {
        PhysicalFormat::Iceberg => {
            let metadata_location = resolve_location(
                physical.metadata_path.as_str(),
                query_context.local_data_dir.as_deref(),
            )?;
            register_iceberg_table(
                session,
                entity.table.as_str(),
                metadata_location.as_str(),
                &query_context.iceberg_io_props,
                &physical.options,
            )
            .await?;
            Ok(Some(format!("iceberg:{}", metadata_location)))
        }
        PhysicalFormat::Csv => {
            let location = resolve_location(
                physical.location.as_str(),
                query_context.local_data_dir.as_deref(),
            )?;
            session
                .register_csv(
                    entity.table.as_str(),
                    location.as_str(),
                    CsvReadOptions::new().has_header(true),
                )
                .await
                .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
            Ok(Some(format!("csv:{}", location)))
        }
        PhysicalFormat::Parquet => {
            let location = resolve_location(
                physical.location.as_str(),
                query_context.local_data_dir.as_deref(),
            )?;
            session
                .register_parquet(
                    entity.table.as_str(),
                    location.as_str(),
                    ParquetReadOptions::default(),
                )
                .await
                .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
            Ok(Some(format!("parquet:{}", location)))
        }
        PhysicalFormat::Auto => {
            if !physical.metadata_path.trim().is_empty() {
                let metadata_location = resolve_location(
                    physical.metadata_path.as_str(),
                    query_context.local_data_dir.as_deref(),
                )?;
                register_iceberg_table(
                    session,
                    entity.table.as_str(),
                    metadata_location.as_str(),
                    &query_context.iceberg_io_props,
                    &physical.options,
                )
                .await?;
                return Ok(Some(format!("iceberg:{}", metadata_location)));
            }

            if !physical.location.trim().is_empty() {
                let location = resolve_location(
                    physical.location.as_str(),
                    query_context.local_data_dir.as_deref(),
                )?;

                if location.ends_with(".parquet") {
                    session
                        .register_parquet(
                            entity.table.as_str(),
                            location.as_str(),
                            ParquetReadOptions::default(),
                        )
                        .await
                        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
                    return Ok(Some(format!("parquet:{}", location)));
                }

                session
                    .register_csv(
                        entity.table.as_str(),
                        location.as_str(),
                        CsvReadOptions::new().has_header(true),
                    )
                    .await
                    .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;
                return Ok(Some(format!("csv:{}", location)));
            }

            Ok(None)
        }
    }
}

async fn register_iceberg_table(
    session: &SessionContext,
    table_name: &str,
    metadata_location: &str,
    global_io_props: &HashMap<String, String>,
    model_io_props: &HashMap<String, String>,
) -> Result<(), QuarryExecError> {
    let mut io_props = global_io_props.clone();
    for (key, value) in model_io_props {
        io_props.insert(key.clone(), value.clone());
    }

    let file_io = FileIO::from_path(metadata_location)
        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?
        .with_props(io_props)
        .build()
        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

    let table_ident = TableIdent::from_strs(["quarry", table_name])
        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

    let static_table = StaticTable::from_metadata_file(metadata_location, table_ident, file_io)
        .await
        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

    let provider = IcebergStaticTableProvider::try_new_from_table(static_table.into_table())
        .await
        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

    session
        .register_table(table_name, Arc::new(provider))
        .map_err(|error| QuarryExecError::Catalog(error.to_string()))?;

    Ok(())
}

fn resolve_location(raw: &str, local_data_dir: Option<&Path>) -> Result<String, QuarryExecError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(QuarryExecError::Config(
            "physical source path is empty".to_string(),
        ));
    }

    if value.contains("://") {
        return Ok(value.to_string());
    }

    let path = PathBuf::from(value);
    if path.is_absolute() {
        return Ok(path.to_string_lossy().to_string());
    }

    if let Some(base_dir) = local_data_dir {
        return Ok(base_dir.join(path).to_string_lossy().to_string());
    }

    Ok(path.to_string_lossy().to_string())
}

// Keep explicit references to pinned crates to make version intent visible at compile time.
#[allow(dead_code)]
fn _iceberg_version_anchor() {
    let _ = std::any::type_name::<iceberg::table::Table>();
    let _ = std::any::type_name::<iceberg_datafusion::table::IcebergTableProvider>();
}
