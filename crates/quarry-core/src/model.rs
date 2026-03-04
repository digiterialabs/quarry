use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use serde::{Deserialize, Serialize};

use crate::errors::{QuarryCoreError, ValidationIssue};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SemanticModel {
    pub schema_version: String,
    pub entities: Vec<Entity>,
    pub metrics: Vec<MetricDefinition>,
    pub pre_aggregations: Vec<PreAggregationDefinition>,
}

impl Default for SemanticModel {
    fn default() -> Self {
        Self {
            schema_version: "v1".to_string(),
            entities: Vec::new(),
            metrics: Vec::new(),
            pre_aggregations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Entity {
    pub name: String,
    pub table: String,
    pub physical: Option<PhysicalSource>,
    pub tenant_column: String,
    pub primary_key: String,
    pub relationships: Vec<Relationship>,
    pub dimensions: Vec<Dimension>,
    pub measures: Vec<Measure>,
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            name: String::new(),
            table: String::new(),
            physical: None,
            tenant_column: "tenant_id".to_string(),
            primary_key: "id".to_string(),
            relationships: Vec::new(),
            dimensions: Vec::new(),
            measures: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PhysicalSource {
    pub format: PhysicalFormat,
    pub metadata_path: String,
    pub location: String,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalFormat {
    #[default]
    Auto,
    Iceberg,
    Parquet,
    Csv,
}

impl Entity {
    pub fn schema(&self) -> SchemaRef {
        let mut fields = Vec::new();
        fields.push(Field::new(&self.primary_key, DataType::Utf8, false));
        fields.push(Field::new(&self.tenant_column, DataType::Utf8, false));

        for rel in &self.relationships {
            if !rel.local_key.is_empty() {
                fields.push(Field::new(&rel.local_key, DataType::Utf8, true));
            }
        }

        for dim in &self.dimensions {
            fields.push(Field::new(&dim.column, dim.data_type.to_arrow(), true));
        }

        for measure in &self.measures {
            fields.push(Field::new(
                &measure.column,
                measure.data_type.to_arrow(),
                true,
            ));
        }

        std::sync::Arc::new(Schema::new(fields))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub to: String,
    #[serde(default)]
    pub kind: RelationshipKind,
    pub local_key: String,
    pub remote_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    #[default]
    ManyToOne,
    OneToMany,
    OneToOne,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub name: String,
    pub column: String,
    #[serde(default)]
    pub kind: DimensionKind,
    #[serde(default)]
    pub data_type: DataTypeName,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DimensionKind {
    #[default]
    Categorical,
    Temporal,
    Boolean,
    Numeric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measure {
    pub name: String,
    pub column: String,
    #[serde(default)]
    pub agg: MeasureAgg,
    #[serde(default = "DataTypeName::numeric_default")]
    pub data_type: DataTypeName,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeasureAgg {
    #[default]
    Sum,
    Count,
    Avg,
    Min,
    Max,
    CountDistinct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub name: String,
    #[serde(default)]
    pub kind: MetricKind,
    pub entity: String,
    #[serde(default)]
    pub measure: String,
    #[serde(default)]
    pub expression: String,
    #[serde(default)]
    pub numerator: String,
    #[serde(default)]
    pub denominator: String,
    #[serde(default)]
    pub filter: Option<MetricFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    #[default]
    Simple,
    Derived,
    Cumulative,
    Ratio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricFilter {
    pub field: String,
    pub op: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreAggregationDefinition {
    pub name: String,
    pub metrics: Vec<String>,
    pub dimensions: Vec<String>,
    pub filters: Vec<MetricFilter>,
    pub refresh: PreAggregationRefreshPolicy,
}

impl Default for PreAggregationDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            metrics: Vec::new(),
            dimensions: Vec::new(),
            filters: Vec::new(),
            refresh: PreAggregationRefreshPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreAggregationRefreshPolicy {
    pub mode: PreAggregationRefreshMode,
    pub interval_seconds: u64,
}

impl Default for PreAggregationRefreshPolicy {
    fn default() -> Self {
        Self {
            mode: PreAggregationRefreshMode::Interval,
            interval_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreAggregationRefreshMode {
    Manual,
    #[default]
    Interval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataTypeName {
    Utf8,
    Int64,
    Float64,
    Boolean,
    Timestamp,
}

impl Default for DataTypeName {
    fn default() -> Self {
        Self::Utf8
    }
}

impl DataTypeName {
    pub fn numeric_default() -> Self {
        Self::Float64
    }

    pub fn to_arrow(&self) -> DataType {
        match self {
            Self::Utf8 => DataType::Utf8,
            Self::Int64 => DataType::Int64,
            Self::Float64 => DataType::Float64,
            Self::Boolean => DataType::Boolean,
            Self::Timestamp => {
                DataType::Timestamp(datafusion::arrow::datatypes::TimeUnit::Millisecond, None)
            }
        }
    }
}

impl SemanticModel {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, QuarryCoreError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .map_err(|e| QuarryCoreError::ModelLoad(format!("{}: {e}", path.display())))?;
        let model: SemanticModel = serde_yaml_ng::from_str(&raw)
            .map_err(|e| QuarryCoreError::ModelLoad(format!("{}: {e}", path.display())))?;
        model.validate()?;
        Ok(model)
    }

    pub fn validate(&self) -> Result<(), QuarryCoreError> {
        let mut issues = Vec::new();

        if self.entities.is_empty() {
            issues.push(ValidationIssue {
                code: "EMPTY_ENTITIES",
                path: "entities".to_string(),
                message: "At least one entity is required".to_string(),
                suggestions: vec!["Add an entity under entities".to_string()],
            });
        }

        let mut seen_entities = HashSet::new();
        for entity in &self.entities {
            if entity.name.is_empty() {
                issues.push(ValidationIssue {
                    code: "EMPTY_ENTITY_NAME",
                    path: "entities[].name".to_string(),
                    message: "Entity name cannot be empty".to_string(),
                    suggestions: vec![],
                });
            }

            if !seen_entities.insert(entity.name.clone()) {
                issues.push(ValidationIssue {
                    code: "DUPLICATE_ENTITY",
                    path: format!("entities.{}", entity.name),
                    message: format!("Duplicate entity '{}'", entity.name),
                    suggestions: vec!["Use unique entity names".to_string()],
                });
            }

            if entity.table.is_empty() {
                issues.push(ValidationIssue {
                    code: "EMPTY_TABLE",
                    path: format!("entities.{}.table", entity.name),
                    message: "Entity table cannot be empty".to_string(),
                    suggestions: vec![],
                });
            }

            if let Some(physical) = &entity.physical {
                match physical.format {
                    PhysicalFormat::Iceberg => {
                        if physical.metadata_path.trim().is_empty() {
                            issues.push(ValidationIssue {
                                code: "MISSING_ICEBERG_METADATA_PATH",
                                path: format!("entities.{}.physical.metadata_path", entity.name),
                                message: "Iceberg physical source requires metadata_path"
                                    .to_string(),
                                suggestions: vec![
                                    "Set physical.metadata_path to metadata.json".to_string()
                                ],
                            });
                        }
                    }
                    PhysicalFormat::Parquet | PhysicalFormat::Csv => {
                        if physical.location.trim().is_empty() {
                            issues.push(ValidationIssue {
                                code: "MISSING_PHYSICAL_LOCATION",
                                path: format!("entities.{}.physical.location", entity.name),
                                message: "Physical source requires location for csv/parquet"
                                    .to_string(),
                                suggestions: vec![
                                    "Set physical.location to a file path or URI".to_string()
                                ],
                            });
                        }
                    }
                    PhysicalFormat::Auto => {
                        if physical.metadata_path.trim().is_empty()
                            && physical.location.trim().is_empty()
                        {
                            issues.push(ValidationIssue {
                                code: "MISSING_PHYSICAL_SOURCE",
                                path: format!("entities.{}.physical", entity.name),
                                message:
                                    "Auto physical source requires metadata_path or location"
                                        .to_string(),
                                suggestions: vec![
                                    "Set physical.metadata_path (iceberg) or physical.location (csv/parquet)".to_string(),
                                ],
                            });
                        }
                    }
                }
            }

            let mut seen_dims = HashSet::new();
            for dim in &entity.dimensions {
                if !seen_dims.insert(dim.name.clone()) {
                    issues.push(ValidationIssue {
                        code: "DUPLICATE_DIMENSION",
                        path: format!("entities.{}.dimensions.{}", entity.name, dim.name),
                        message: format!("Duplicate dimension '{}'", dim.name),
                        suggestions: vec!["Use unique dimension names per entity".to_string()],
                    });
                }
            }

            let mut seen_measures = HashSet::new();
            for measure in &entity.measures {
                if !seen_measures.insert(measure.name.clone()) {
                    issues.push(ValidationIssue {
                        code: "DUPLICATE_MEASURE",
                        path: format!("entities.{}.measures.{}", entity.name, measure.name),
                        message: format!("Duplicate measure '{}'", measure.name),
                        suggestions: vec!["Use unique measure names per entity".to_string()],
                    });
                }
            }
        }

        let mut seen_metrics = HashSet::new();
        for metric in &self.metrics {
            if !seen_metrics.insert(metric.name.clone()) {
                issues.push(ValidationIssue {
                    code: "DUPLICATE_METRIC",
                    path: format!("metrics.{}", metric.name),
                    message: format!("Duplicate metric '{}'", metric.name),
                    suggestions: vec!["Use unique metric names".to_string()],
                });
            }

            let Some(entity) = self.entity_by_name(&metric.entity) else {
                issues.push(ValidationIssue {
                    code: "UNKNOWN_METRIC_ENTITY",
                    path: format!("metrics.{}.entity", metric.name),
                    message: format!(
                        "Metric '{}' references unknown entity '{}",
                        metric.name, metric.entity
                    ),
                    suggestions: self.entities.iter().map(|e| e.name.clone()).collect(),
                });
                continue;
            };

            if matches!(metric.kind, MetricKind::Simple) {
                if metric.measure.is_empty() {
                    issues.push(ValidationIssue {
                        code: "MISSING_MEASURE",
                        path: format!("metrics.{}.measure", metric.name),
                        message: "Simple metrics must define a measure".to_string(),
                        suggestions: entity.measures.iter().map(|m| m.name.clone()).collect(),
                    });
                } else if entity
                    .measures
                    .iter()
                    .all(|measure| measure.name != metric.measure)
                {
                    issues.push(ValidationIssue {
                        code: "UNKNOWN_MEASURE",
                        path: format!("metrics.{}.measure", metric.name),
                        message: format!(
                            "Metric '{}' references unknown measure '{}' on entity '{}'",
                            metric.name, metric.measure, entity.name
                        ),
                        suggestions: entity.measures.iter().map(|m| m.name.clone()).collect(),
                    });
                }
            }
        }

        let mut seen_pre_aggregations = HashSet::new();
        for pre_aggregation in &self.pre_aggregations {
            if pre_aggregation.name.trim().is_empty() {
                issues.push(ValidationIssue {
                    code: "EMPTY_PRE_AGGREGATION_NAME",
                    path: "pre_aggregations[].name".to_string(),
                    message: "Pre-aggregation name cannot be empty".to_string(),
                    suggestions: vec!["Set pre_aggregation.name".to_string()],
                });
                continue;
            }

            if !seen_pre_aggregations.insert(pre_aggregation.name.clone()) {
                issues.push(ValidationIssue {
                    code: "DUPLICATE_PRE_AGGREGATION",
                    path: format!("pre_aggregations.{}", pre_aggregation.name),
                    message: format!(
                        "Duplicate pre-aggregation '{}' definition",
                        pre_aggregation.name
                    ),
                    suggestions: vec!["Use unique pre-aggregation names".to_string()],
                });
            }

            if pre_aggregation.metrics.is_empty() {
                issues.push(ValidationIssue {
                    code: "EMPTY_PRE_AGGREGATION_METRICS",
                    path: format!("pre_aggregations.{}.metrics", pre_aggregation.name),
                    message: "Pre-aggregation must define at least one metric".to_string(),
                    suggestions: self.metrics.iter().map(|m| m.name.clone()).collect(),
                });
                continue;
            }

            let mut base_entity: Option<&str> = None;
            for metric_name in &pre_aggregation.metrics {
                let Some(metric) = self.metric_by_name(metric_name) else {
                    issues.push(ValidationIssue {
                        code: "UNKNOWN_PRE_AGGREGATION_METRIC",
                        path: format!(
                            "pre_aggregations.{}.metrics.{}",
                            pre_aggregation.name, metric_name
                        ),
                        message: format!(
                            "Pre-aggregation '{}' references unknown metric '{}'",
                            pre_aggregation.name, metric_name
                        ),
                        suggestions: self.metrics.iter().map(|m| m.name.clone()).collect(),
                    });
                    continue;
                };

                if base_entity.is_none() {
                    base_entity = Some(metric.entity.as_str());
                } else if base_entity != Some(metric.entity.as_str()) {
                    issues.push(ValidationIssue {
                        code: "CROSS_ENTITY_PRE_AGGREGATION_METRICS",
                        path: format!("pre_aggregations.{}.metrics", pre_aggregation.name),
                        message: format!(
                            "Pre-aggregation '{}' mixes metrics across entities",
                            pre_aggregation.name
                        ),
                        suggestions: vec![
                            "Use metrics from one base entity per pre-aggregation".to_string()
                        ],
                    });
                }
            }

            for dimension_ref in &pre_aggregation.dimensions {
                let Some((entity_name, dim_name)) = self.parse_ref(dimension_ref) else {
                    issues.push(ValidationIssue {
                        code: "INVALID_PRE_AGGREGATION_DIMENSION_REF",
                        path: format!(
                            "pre_aggregations.{}.dimensions.{}",
                            pre_aggregation.name, dimension_ref
                        ),
                        message: format!(
                            "Pre-aggregation dimension '{}' must be <entity>.<dimension>",
                            dimension_ref
                        ),
                        suggestions: vec!["Example: orders.created_at".to_string()],
                    });
                    continue;
                };

                if self.entity_dimension(entity_name, dim_name).is_none() {
                    issues.push(ValidationIssue {
                        code: "UNKNOWN_PRE_AGGREGATION_DIMENSION",
                        path: format!(
                            "pre_aggregations.{}.dimensions.{}",
                            pre_aggregation.name, dimension_ref
                        ),
                        message: format!(
                            "Pre-aggregation '{}' references unknown dimension '{}'",
                            pre_aggregation.name, dimension_ref
                        ),
                        suggestions: self
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
                }
            }

            for filter in &pre_aggregation.filters {
                let Some((entity_name, field_name)) = self.parse_ref(&filter.field) else {
                    issues.push(ValidationIssue {
                        code: "INVALID_PRE_AGGREGATION_FILTER_REF",
                        path: format!(
                            "pre_aggregations.{}.filters.{}",
                            pre_aggregation.name, filter.field
                        ),
                        message: format!(
                            "Pre-aggregation filter '{}' must be <entity>.<field>",
                            filter.field
                        ),
                        suggestions: vec!["Example: orders.status".to_string()],
                    });
                    continue;
                };

                let known_dimension = self.entity_dimension(entity_name, field_name).is_some();
                let known_measure = self.entity_measure(entity_name, field_name).is_some();
                if !known_dimension && !known_measure {
                    issues.push(ValidationIssue {
                        code: "UNKNOWN_PRE_AGGREGATION_FILTER_FIELD",
                        path: format!(
                            "pre_aggregations.{}.filters.{}",
                            pre_aggregation.name, filter.field
                        ),
                        message: format!(
                            "Pre-aggregation '{}' references unknown filter field '{}'",
                            pre_aggregation.name, filter.field
                        ),
                        suggestions: vec![
                            "Use <entity>.<dimension> or <entity>.<measure>".to_string()
                        ],
                    });
                }
            }

            if matches!(
                pre_aggregation.refresh.mode,
                PreAggregationRefreshMode::Interval
            ) && pre_aggregation.refresh.interval_seconds == 0
            {
                issues.push(ValidationIssue {
                    code: "INVALID_PRE_AGGREGATION_REFRESH_INTERVAL",
                    path: format!(
                        "pre_aggregations.{}.refresh.interval_seconds",
                        pre_aggregation.name
                    ),
                    message: "interval refresh mode requires interval_seconds > 0".to_string(),
                    suggestions: vec!["Set interval_seconds to at least 1".to_string()],
                });
            }
        }

        for entity in &self.entities {
            for rel in &entity.relationships {
                if self.entity_by_name(&rel.to).is_none() {
                    issues.push(ValidationIssue {
                        code: "UNKNOWN_REL_ENTITY",
                        path: format!("entities.{}.relationships.{}.to", entity.name, rel.to),
                        message: format!(
                            "Relationship from '{}' references unknown entity '{}'",
                            entity.name, rel.to
                        ),
                        suggestions: self.entities.iter().map(|e| e.name.clone()).collect(),
                    });
                }
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(QuarryCoreError::ModelValidation(issues))
        }
    }

    pub fn entity_by_name(&self, name: &str) -> Option<&Entity> {
        self.entities.iter().find(|entity| entity.name == name)
    }

    pub fn metric_by_name(&self, name: &str) -> Option<&MetricDefinition> {
        self.metrics.iter().find(|metric| metric.name == name)
    }

    pub fn pre_aggregation_by_name(&self, name: &str) -> Option<&PreAggregationDefinition> {
        self.pre_aggregations
            .iter()
            .find(|entry| entry.name == name)
    }

    pub fn entity_dimension(&self, entity_name: &str, dimension_name: &str) -> Option<&Dimension> {
        self.entity_by_name(entity_name).and_then(|entity| {
            entity
                .dimensions
                .iter()
                .find(|dimension| dimension.name == dimension_name)
        })
    }

    pub fn entity_measure(&self, entity_name: &str, measure_name: &str) -> Option<&Measure> {
        self.entity_by_name(entity_name).and_then(|entity| {
            entity
                .measures
                .iter()
                .find(|measure| measure.name == measure_name)
        })
    }

    pub fn parse_ref<'a>(&self, value: &'a str) -> Option<(&'a str, &'a str)> {
        let mut split = value.split('.');
        match (split.next(), split.next(), split.next()) {
            (Some(entity), Some(name), None) => Some((entity, name)),
            _ => None,
        }
    }

    pub fn relationship_map(&self) -> HashMap<String, Vec<(String, Relationship)>> {
        let mut map: HashMap<String, Vec<(String, Relationship)>> = HashMap::new();
        for entity in &self.entities {
            for rel in &entity.relationships {
                map.entry(entity.name.clone())
                    .or_default()
                    .push((rel.to.clone(), rel.clone()));
            }
        }
        map
    }

    pub fn export_catalog(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": self.schema_version,
            "entities": self.entities.iter().map(|entity| {
                serde_json::json!({
                    "name": entity.name,
                    "table": entity.table,
                    "tenant_column": entity.tenant_column,
                    "primary_key": entity.primary_key,
                    "dimensions": entity.dimensions.iter().map(|dim| {
                        serde_json::json!({
                            "name": dim.name,
                            "column": dim.column,
                            "kind": format!("{:?}", dim.kind).to_lowercase(),
                            "data_type": format!("{:?}", dim.data_type).to_lowercase(),
                        })
                    }).collect::<Vec<_>>(),
                    "measures": entity.measures.iter().map(|measure| {
                        serde_json::json!({
                            "name": measure.name,
                            "column": measure.column,
                            "agg": format!("{:?}", measure.agg).to_lowercase(),
                            "data_type": format!("{:?}", measure.data_type).to_lowercase(),
                        })
                    }).collect::<Vec<_>>(),
                    "relationships": entity.relationships.iter().map(|rel| {
                        serde_json::json!({
                            "to": rel.to,
                            "kind": format!("{:?}", rel.kind).to_lowercase(),
                            "local_key": rel.local_key,
                            "remote_key": rel.remote_key,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "metrics": self.metrics.iter().map(|metric| {
                serde_json::json!({
                    "name": metric.name,
                    "kind": format!("{:?}", metric.kind).to_lowercase(),
                    "entity": metric.entity,
                    "measure": metric.measure,
                    "expression": metric.expression,
                    "numerator": metric.numerator,
                    "denominator": metric.denominator,
                    "filter": metric.filter,
                })
            }).collect::<Vec<_>>(),
            "pre_aggregations": self.pre_aggregations.iter().map(|preagg| {
                serde_json::json!({
                    "name": preagg.name,
                    "metrics": preagg.metrics,
                    "dimensions": preagg.dimensions,
                    "filters": preagg.filters,
                    "refresh": {
                        "mode": format!("{:?}", preagg.refresh.mode).to_lowercase(),
                        "interval_seconds": preagg.refresh.interval_seconds,
                    }
                })
            }).collect::<Vec<_>>(),
        })
    }
}
