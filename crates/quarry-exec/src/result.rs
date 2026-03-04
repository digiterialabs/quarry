use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct QuerySuccessEnvelope {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub data: QueryData,
    pub meta: QueryMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryData {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryMeta {
    pub row_count: usize,
    pub planning_ms: u128,
    pub optimization_ms: u128,
    pub physical_planning_ms: u128,
    pub execution_ms: u128,
    pub generated_sql: String,
    pub optimized_plan: String,
    pub physical_plan: String,
    pub logical_plan_hash: u64,
    pub optimized_plan_hash: u64,
    pub physical_plan_hash: u64,
    pub tenant_id: String,
    pub catalog: String,
    pub sandbox_id: String,
    pub execution_mode: String,
    pub table_bindings: Vec<TableBindingMeta>,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableBindingMeta {
    pub entity: String,
    pub table: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorEnvelope {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub error: QueryError,
    pub meta: ErrorMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryError {
    pub code: String,
    pub message: String,
    pub suggestions: Vec<String>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorMeta {
    pub request_id: String,
}
