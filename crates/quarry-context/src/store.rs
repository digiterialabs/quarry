use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hex::encode as hex_encode;
use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::chunking::chunk_text;
use crate::connector::{load_documents, ConnectorKind, SourceDocument};
use crate::error::QuarryContextError;
use crate::model::{Collection, SearchHit, SearchResult, SyncSummary};

#[derive(Debug)]
pub struct ContextStore {
    db_path: PathBuf,
    conn: Connection,
}

impl ContextStore {
    pub fn open(context_dir: &Path) -> Result<Self, QuarryContextError> {
        fs::create_dir_all(context_dir).map_err(|error| {
            QuarryContextError::database(format!(
                "failed to create context directory {}: {error}",
                context_dir.display()
            ))
        })?;

        let db_path = context_dir.join("context.db");
        let conn = Connection::open(&db_path).map_err(map_sqlite_err)?;

        let store = Self { db_path, conn };
        store.initialize_schema()?;
        Ok(store)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn create_collection(
        &self,
        tenant_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Collection, QuarryContextError> {
        let tenant = validate_required("tenant_id", tenant_id)?;
        let collection_name = validate_required("name", name)?;
        let cleaned_description = description
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let now = now_unix_ms();
        let id = Uuid::new_v4().to_string();

        let inserted = self.conn.execute(
            "INSERT INTO collections (id, tenant_id, name, description, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                tenant,
                collection_name,
                cleaned_description,
                now,
                now
            ],
        );

        match inserted {
            Ok(_) => Ok(Collection {
                id,
                tenant_id: tenant.to_string(),
                name: collection_name.to_string(),
                description: cleaned_description,
                created_at_unix_ms: now,
                updated_at_unix_ms: now,
            }),
            Err(rusqlite::Error::SqliteFailure(sql_error, _))
                if sql_error.code == ErrorCode::ConstraintViolation =>
            {
                Err(QuarryContextError::invalid(format!(
                    "collection '{}' already exists for tenant '{}'",
                    collection_name, tenant
                )))
            }
            Err(error) => Err(map_sqlite_err(error)),
        }
    }

    pub fn list_collections(&self, tenant_id: &str) -> Result<Vec<Collection>, QuarryContextError> {
        let tenant = validate_required("tenant_id", tenant_id)?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, tenant_id, name, description, created_at_unix_ms, updated_at_unix_ms
                 FROM collections
                 WHERE tenant_id = ?1
                 ORDER BY name ASC",
            )
            .map_err(map_sqlite_err)?;

        let rows = stmt
            .query_map(params![tenant], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    created_at_unix_ms: row.get(4)?,
                    updated_at_unix_ms: row.get(5)?,
                })
            })
            .map_err(map_sqlite_err)?;

        let mut collections = Vec::new();
        for row in rows {
            collections.push(row.map_err(map_sqlite_err)?);
        }
        Ok(collections)
    }

    pub fn sync_collection(
        &self,
        tenant_id: &str,
        collection: &str,
        connector: ConnectorKind,
        connector_config: &Value,
    ) -> Result<SyncSummary, QuarryContextError> {
        let tenant = validate_required("tenant_id", tenant_id)?;
        let collection_name = validate_required("collection", collection)?;
        self.ensure_collection_exists(tenant, collection_name)?;

        let sync_run_id = Uuid::new_v4().to_string();
        let started = now_unix_ms();

        self.insert_sync_run(
            sync_run_id.as_str(),
            tenant,
            collection_name,
            connector.as_str(),
            connector_config,
            "running",
            started,
        )?;

        let documents = match load_documents(connector, connector_config) {
            Ok(docs) => docs,
            Err(error) => {
                self.finish_sync_run(
                    sync_run_id.as_str(),
                    "failed",
                    0,
                    0,
                    0,
                    0,
                    Some(error.to_string()),
                )?;
                return Err(error);
            }
        };

        let documents_seen = documents.len();
        let mut documents_indexed = 0usize;
        let mut documents_skipped = 0usize;
        let mut chunks_indexed = 0usize;

        for document in &documents {
            match self.upsert_document(tenant, collection_name, document)? {
                UpsertOutcome::Indexed { chunks } => {
                    documents_indexed += 1;
                    chunks_indexed += chunks;
                }
                UpsertOutcome::Skipped => documents_skipped += 1,
            }
        }

        let finished = now_unix_ms();
        self.finish_sync_run(
            sync_run_id.as_str(),
            "ok",
            documents_seen,
            documents_indexed,
            documents_skipped,
            chunks_indexed,
            None,
        )?;

        Ok(SyncSummary {
            sync_run_id,
            connector: connector.as_str().to_string(),
            tenant_id: tenant.to_string(),
            collection: collection_name.to_string(),
            documents_seen,
            documents_indexed,
            documents_skipped,
            chunks_indexed,
            status: "ok".to_string(),
            started_at_unix_ms: started,
            finished_at_unix_ms: finished,
        })
    }

    pub fn search(
        &self,
        tenant_id: &str,
        collection: &str,
        query: &str,
        top_k: usize,
        hybrid: bool,
    ) -> Result<SearchResult, QuarryContextError> {
        let tenant = validate_required("tenant_id", tenant_id)?;
        let collection_name = validate_required("collection", collection)?;
        let query_text = validate_required("query", query)?;

        if top_k == 0 {
            return Err(QuarryContextError::invalid("top_k must be greater than 0"));
        }

        self.ensure_collection_exists(tenant, collection_name)?;
        let fts_query = to_fts_query(query_text)?;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.id, c.document_id, d.title, c.text, d.source_uri, d.metadata_json, bm25(chunks_fts)
                 FROM chunks_fts
                 JOIN chunks c ON c.id = chunks_fts.chunk_id
                 JOIN documents d ON d.id = c.document_id
                 WHERE chunks_fts MATCH ?1
                   AND c.tenant_id = ?2
                   AND c.collection_name = ?3
                 ORDER BY bm25(chunks_fts) ASC
                 LIMIT ?4",
            )
            .map_err(map_sqlite_err)?;

        let rows = stmt
            .query_map(
                params![fts_query, tenant, collection_name, top_k as i64],
                |row| {
                    let metadata_json: String = row.get(5)?;
                    let metadata = serde_json::from_str::<Value>(&metadata_json)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let bm25_score: f64 = row.get(6)?;
                    Ok(SearchHit {
                        chunk_id: row.get(0)?,
                        document_id: row.get(1)?,
                        title: row.get(2)?,
                        snippet: to_snippet(&row.get::<_, String>(3)?),
                        source_uri: row.get(4)?,
                        score: -bm25_score,
                        bm25_score,
                        vector_score: None,
                        metadata,
                    })
                },
            )
            .map_err(map_sqlite_err)?;

        let mut hits = Vec::new();
        for row in rows {
            hits.push(row.map_err(map_sqlite_err)?);
        }

        Ok(SearchResult {
            total_hits: hits.len(),
            hits,
            query: query_text.to_string(),
            collection: collection_name.to_string(),
            tenant_id: tenant.to_string(),
            hybrid_used: false && hybrid,
        })
    }

    fn ensure_collection_exists(
        &self,
        tenant_id: &str,
        collection: &str,
    ) -> Result<(), QuarryContextError> {
        let exists = self
            .conn
            .query_row(
                "SELECT 1 FROM collections WHERE tenant_id = ?1 AND name = ?2 LIMIT 1",
                params![tenant_id, collection],
                |_row| Ok(()),
            )
            .optional()
            .map_err(map_sqlite_err)?;

        if exists.is_none() {
            return Err(QuarryContextError::invalid(format!(
                "collection '{}' does not exist for tenant '{}'",
                collection, tenant_id
            )));
        }
        Ok(())
    }

    fn insert_sync_run(
        &self,
        sync_run_id: &str,
        tenant_id: &str,
        collection: &str,
        connector: &str,
        config: &Value,
        status: &str,
        started_at_unix_ms: u64,
    ) -> Result<(), QuarryContextError> {
        let config_json = serde_json::to_string(config)
            .map_err(|error| QuarryContextError::database(error.to_string()))?;
        self.conn
            .execute(
                "INSERT INTO sync_runs (
                   id, tenant_id, collection_name, connector, config_json,
                   status, started_at_unix_ms, finished_at_unix_ms,
                   documents_seen, documents_indexed, documents_skipped, chunks_indexed, error_message
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 0, 0, 0, 0, NULL)",
                params![
                    sync_run_id,
                    tenant_id,
                    collection,
                    connector,
                    config_json,
                    status,
                    started_at_unix_ms
                ],
            )
            .map_err(map_sqlite_err)?;
        Ok(())
    }

    fn finish_sync_run(
        &self,
        sync_run_id: &str,
        status: &str,
        documents_seen: usize,
        documents_indexed: usize,
        documents_skipped: usize,
        chunks_indexed: usize,
        error_message: Option<String>,
    ) -> Result<(), QuarryContextError> {
        self.conn
            .execute(
                "UPDATE sync_runs
                 SET status = ?2,
                     finished_at_unix_ms = ?3,
                     documents_seen = ?4,
                     documents_indexed = ?5,
                     documents_skipped = ?6,
                     chunks_indexed = ?7,
                     error_message = ?8
                 WHERE id = ?1",
                params![
                    sync_run_id,
                    status,
                    now_unix_ms(),
                    documents_seen as i64,
                    documents_indexed as i64,
                    documents_skipped as i64,
                    chunks_indexed as i64,
                    error_message
                ],
            )
            .map_err(map_sqlite_err)?;
        Ok(())
    }

    fn upsert_document(
        &self,
        tenant_id: &str,
        collection: &str,
        document: &SourceDocument,
    ) -> Result<UpsertOutcome, QuarryContextError> {
        if document.content.trim().is_empty() {
            return Ok(UpsertOutcome::Skipped);
        }

        let content_hash = hash_content(document.content.as_bytes());
        let now = now_unix_ms();
        let metadata_json = serde_json::to_string(&document.metadata)
            .map_err(|error| QuarryContextError::database(error.to_string()))?;

        let existing = self
            .conn
            .query_row(
                "SELECT id, content_hash
                 FROM documents
                 WHERE tenant_id = ?1 AND collection_name = ?2 AND source_uri = ?3
                 LIMIT 1",
                params![tenant_id, collection, document.source_uri.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(map_sqlite_err)?;

        let document_id = match existing {
            Some((existing_id, existing_hash)) => {
                if existing_hash == content_hash {
                    return Ok(UpsertOutcome::Skipped);
                }

                self.conn
                    .execute(
                        "UPDATE documents
                         SET title = ?2,
                             content_hash = ?3,
                             content = ?4,
                             metadata_json = ?5,
                             updated_at_unix_ms = ?6
                         WHERE id = ?1",
                        params![
                            existing_id,
                            document.title,
                            content_hash,
                            document.content,
                            metadata_json,
                            now
                        ],
                    )
                    .map_err(map_sqlite_err)?;
                existing_id
            }
            None => {
                let new_id = Uuid::new_v4().to_string();
                self.conn
                    .execute(
                        "INSERT INTO documents (
                           id, tenant_id, collection_name, source_uri, title,
                           content_hash, content, metadata_json, created_at_unix_ms, updated_at_unix_ms
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            new_id,
                            tenant_id,
                            collection,
                            document.source_uri,
                            document.title,
                            content_hash,
                            document.content,
                            metadata_json,
                            now,
                            now
                        ],
                    )
                    .map_err(map_sqlite_err)?;
                new_id
            }
        };

        self.delete_document_chunks(document_id.as_str())?;
        let chunks = chunk_text(document.content.as_str(), 1200, 180);
        if chunks.is_empty() {
            return Ok(UpsertOutcome::Skipped);
        }

        for (index, chunk) in chunks.iter().enumerate() {
            let chunk_id = Uuid::new_v4().to_string();
            self.conn
                .execute(
                    "INSERT INTO chunks (
                       id, document_id, tenant_id, collection_name, chunk_index, text, created_at_unix_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        chunk_id,
                        document_id,
                        tenant_id,
                        collection,
                        index as i64,
                        chunk,
                        now
                    ],
                )
                .map_err(map_sqlite_err)?;
            self.conn
                .execute(
                    "INSERT INTO chunks_fts (chunk_id, tenant_id, collection_name, title, content, source_uri)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        chunk_id,
                        tenant_id,
                        collection,
                        document.title,
                        chunk,
                        document.source_uri
                    ],
                )
                .map_err(map_sqlite_err)?;
        }

        Ok(UpsertOutcome::Indexed {
            chunks: chunks.len(),
        })
    }

    fn delete_document_chunks(&self, document_id: &str) -> Result<(), QuarryContextError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM chunks WHERE document_id = ?1")
            .map_err(map_sqlite_err)?;
        let chunk_ids = stmt
            .query_map(params![document_id], |row| row.get::<_, String>(0))
            .map_err(map_sqlite_err)?;

        for chunk_id in chunk_ids {
            let chunk_id = chunk_id.map_err(map_sqlite_err)?;
            self.conn
                .execute(
                    "DELETE FROM chunks_fts WHERE chunk_id = ?1",
                    params![chunk_id.as_str()],
                )
                .map_err(map_sqlite_err)?;
        }

        self.conn
            .execute(
                "DELETE FROM chunks WHERE document_id = ?1",
                params![document_id],
            )
            .map_err(map_sqlite_err)?;
        Ok(())
    }

    fn initialize_schema(&self) -> Result<(), QuarryContextError> {
        self.conn
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE IF NOT EXISTS collections (
                   id TEXT PRIMARY KEY,
                   tenant_id TEXT NOT NULL,
                   name TEXT NOT NULL,
                   description TEXT,
                   created_at_unix_ms INTEGER NOT NULL,
                   updated_at_unix_ms INTEGER NOT NULL,
                   UNIQUE(tenant_id, name)
                 );
                 CREATE INDEX IF NOT EXISTS idx_collections_tenant ON collections (tenant_id);

                 CREATE TABLE IF NOT EXISTS sync_runs (
                   id TEXT PRIMARY KEY,
                   tenant_id TEXT NOT NULL,
                   collection_name TEXT NOT NULL,
                   connector TEXT NOT NULL,
                   config_json TEXT NOT NULL,
                   status TEXT NOT NULL,
                   started_at_unix_ms INTEGER NOT NULL,
                   finished_at_unix_ms INTEGER,
                   documents_seen INTEGER NOT NULL,
                   documents_indexed INTEGER NOT NULL,
                   documents_skipped INTEGER NOT NULL,
                   chunks_indexed INTEGER NOT NULL,
                   error_message TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_sync_runs_tenant_collection
                   ON sync_runs (tenant_id, collection_name, started_at_unix_ms DESC);

                 CREATE TABLE IF NOT EXISTS documents (
                   id TEXT PRIMARY KEY,
                   tenant_id TEXT NOT NULL,
                   collection_name TEXT NOT NULL,
                   source_uri TEXT NOT NULL,
                   title TEXT NOT NULL,
                   content_hash TEXT NOT NULL,
                   content TEXT NOT NULL,
                   metadata_json TEXT NOT NULL,
                   created_at_unix_ms INTEGER NOT NULL,
                   updated_at_unix_ms INTEGER NOT NULL,
                   UNIQUE(tenant_id, collection_name, source_uri)
                 );
                 CREATE INDEX IF NOT EXISTS idx_documents_tenant_collection
                   ON documents (tenant_id, collection_name);

                 CREATE TABLE IF NOT EXISTS chunks (
                   id TEXT PRIMARY KEY,
                   document_id TEXT NOT NULL,
                   tenant_id TEXT NOT NULL,
                   collection_name TEXT NOT NULL,
                   chunk_index INTEGER NOT NULL,
                   text TEXT NOT NULL,
                   created_at_unix_ms INTEGER NOT NULL,
                   UNIQUE(document_id, chunk_index)
                 );
                 CREATE INDEX IF NOT EXISTS idx_chunks_tenant_collection
                   ON chunks (tenant_id, collection_name);

                 CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                   chunk_id UNINDEXED,
                   tenant_id UNINDEXED,
                   collection_name UNINDEXED,
                   title,
                   content,
                   source_uri UNINDEXED
                 );",
            )
            .map_err(map_sqlite_err)
    }
}

enum UpsertOutcome {
    Indexed { chunks: usize },
    Skipped,
}

fn validate_required<'a>(field: &str, value: &'a str) -> Result<&'a str, QuarryContextError> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return Err(QuarryContextError::invalid(format!(
            "{field} must be a non-empty string"
        )));
    }
    Ok(cleaned)
}

fn hash_content(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(hasher.finalize())
}

fn to_fts_query(query: &str) -> Result<String, QuarryContextError> {
    let mut terms = Vec::new();
    for term in query.split_whitespace() {
        let cleaned = term
            .chars()
            .filter(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
            .collect::<String>();
        if !cleaned.is_empty() {
            terms.push(cleaned);
        }
    }

    if terms.is_empty() {
        return Err(QuarryContextError::invalid(
            "query must contain searchable terms",
        ));
    }

    Ok(terms.join(" OR "))
}

fn to_snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 240 {
        return collapsed;
    }

    let short = collapsed.chars().take(237).collect::<String>();
    format!("{short}...")
}

fn now_unix_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(u128::from(u64::MAX)) as u64,
        Err(_) => 0,
    }
}

fn map_sqlite_err(error: rusqlite::Error) -> QuarryContextError {
    QuarryContextError::database(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;
    use crate::connector::ConnectorKind;

    fn temp_context_dir() -> PathBuf {
        std::env::temp_dir().join(format!("quarry-context-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn create_and_list_collection() {
        let context_dir = temp_context_dir();
        let store = ContextStore::open(&context_dir).expect("store");

        store
            .create_collection("tenant_123", "sales_docs", Some("Sales reports"))
            .expect("create");

        let listed = store.list_collections("tenant_123").expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "sales_docs");
        assert_eq!(listed[0].tenant_id, "tenant_123");

        fs::remove_dir_all(context_dir).ok();
    }

    #[test]
    fn sync_and_search_filesystem_collection() {
        let tmp = std::env::temp_dir().join(format!("quarry-context-sync-{}", Uuid::new_v4()));
        let docs_dir = tmp.join("docs");
        fs::create_dir_all(&docs_dir).expect("create docs dir");
        fs::write(
            docs_dir.join("sales.txt"),
            "Enterprise revenue playbook for EMEA and NA teams.",
        )
        .expect("write");

        let context_dir = tmp.join("context");
        let store = ContextStore::open(&context_dir).expect("store");
        store
            .create_collection("tenant_123", "sales_docs", None)
            .expect("collection");

        let summary = store
            .sync_collection(
                "tenant_123",
                "sales_docs",
                ConnectorKind::Filesystem,
                &serde_json::json!({
                    "paths": [docs_dir.to_string_lossy().to_string()]
                }),
            )
            .expect("sync");
        assert_eq!(summary.documents_seen, 1);
        assert_eq!(summary.documents_indexed, 1);

        let result = store
            .search("tenant_123", "sales_docs", "revenue", 5, false)
            .expect("search");
        assert!(!result.hits.is_empty());
        assert_eq!(result.tenant_id, "tenant_123");
        assert_eq!(result.collection, "sales_docs");

        fs::remove_dir_all(tmp).ok();
    }

    #[test]
    fn sync_and_search_url_list_collection() {
        let tmp = std::env::temp_dir().join(format!("quarry-context-url-{}", Uuid::new_v4()));
        let context_dir = tmp.join("context");
        let store = ContextStore::open(&context_dir).expect("store");
        store
            .create_collection("tenant_123", "web_docs", None)
            .expect("collection");

        let (url, handle) = start_single_response_server(
            "Revenue operations handbook for enterprise regions.".to_string(),
        );

        let summary = store
            .sync_collection(
                "tenant_123",
                "web_docs",
                ConnectorKind::UrlList,
                &serde_json::json!({
                    "urls": [url],
                    "timeout_seconds": 5
                }),
            )
            .expect("sync");
        handle.join().expect("server join");
        assert_eq!(summary.documents_seen, 1);

        let result = store
            .search("tenant_123", "web_docs", "revenue", 5, false)
            .expect("search");
        assert!(!result.hits.is_empty());

        fs::remove_dir_all(tmp).ok();
    }

    fn start_single_response_server(body: String) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");

        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        (format!("http://{}", addr), handle)
    }
}
