use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct Collection {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub chunk_id: String,
    pub document_id: String,
    pub title: String,
    pub snippet: String,
    pub source_uri: String,
    pub score: f64,
    pub bm25_score: f64,
    pub vector_score: Option<f64>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total_hits: usize,
    pub query: String,
    pub collection: String,
    pub tenant_id: String,
    pub hybrid_used: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncSummary {
    pub sync_run_id: String,
    pub connector: String,
    pub tenant_id: String,
    pub collection: String,
    pub documents_seen: usize,
    pub documents_indexed: usize,
    pub documents_skipped: usize,
    pub chunks_indexed: usize,
    pub status: String,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
}
