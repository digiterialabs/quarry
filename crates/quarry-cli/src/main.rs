use std::fs;
use std::io::{self, Read};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use quarry_core::{QuarryCoreError, SemanticModel, SemanticQuery};
use quarry_exec::{
    execute_query, explain_query, match_pre_aggregation, materialize_pre_aggregation, CatalogKind,
    ErrorEnvelope, MaterializeResult, PreAggregationMatch, PreAggregationStore, QuarryExecError,
    QueryError,
};
use serde::Deserialize;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "quarry")]
#[command(about = "CLI-first semantic analytics engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Validate {
        #[arg(long)]
        model: PathBuf,
    },
    Query {
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        catalog: String,
        #[arg(long)]
        tenant: String,
        #[arg(long, env = "QUARRY_LOCAL_DATA_DIR")]
        local_data_dir: Option<PathBuf>,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long, default_value = "json")]
        format: String,
    },
    Explain {
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        catalog: String,
        #[arg(long)]
        tenant: String,
        #[arg(long, env = "QUARRY_LOCAL_DATA_DIR")]
        local_data_dir: Option<PathBuf>,
        #[arg(long)]
        input: Option<PathBuf>,
    },
    Serve {
        #[arg(long)]
        model: PathBuf,
        #[arg(long, default_value = "local")]
        catalog: String,
        #[arg(long, env = "QUARRY_LOCAL_DATA_DIR")]
        local_data_dir: Option<PathBuf>,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 4000)]
        port: u16,
    },
}

#[derive(Clone)]
struct ServerState {
    model: Arc<SemanticModel>,
    default_catalog: CatalogKind,
    default_local_data_dir: Option<PathBuf>,
    preagg_store: Arc<Mutex<PreAggregationStore>>,
}

#[derive(Debug, Deserialize)]
struct QueryApiRequest {
    tenant_id: String,
    query: SemanticQuery,
    catalog: Option<String>,
    local_data_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct PreAggregationMatchRequest {
    query: SemanticQuery,
}

#[derive(Debug, Deserialize)]
struct PreAggregationListQuery {
    tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrchestrationMaterializeRequest {
    tenant_id: String,
    pre_aggregation: Option<String>,
    query: Option<SemanticQuery>,
    force: Option<bool>,
    catalog: Option<String>,
    local_data_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct OrchestrationInvalidateRequest {
    tenant_id: Option<String>,
    pre_aggregation: Option<String>,
}

struct ApiError {
    status: StatusCode,
    error: QuarryExecError,
}

impl From<QuarryExecError> for ApiError {
    fn from(error: QuarryExecError) -> Self {
        Self {
            status: status_for_error(&error),
            error,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4().to_string();
        let (message, suggestions, details) = match &self.error {
            QuarryExecError::Core(QuarryCoreError::ModelValidation(issues))
            | QuarryExecError::Core(QuarryCoreError::QueryValidation(issues)) => (
                self.error.to_string(),
                issues
                    .iter()
                    .flat_map(|issue| issue.suggestions.clone())
                    .collect(),
                serde_json::json!({ "issues": issues }),
            ),
            QuarryExecError::Core(core) => (core.to_string(), Vec::new(), serde_json::json!({})),
            _ => (self.error.to_string(), Vec::new(), serde_json::json!({})),
        };

        let envelope = ErrorEnvelope {
            schema_version: "v1",
            status: "error",
            error: QueryError {
                code: self.error.code().to_string(),
                message,
                suggestions,
                details,
            },
            meta: quarry_exec::result::ErrorMeta { request_id },
        };

        (self.status, Json(envelope)).into_response()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Validate { model } => handle_validate(model).await,
        Commands::Query {
            model,
            catalog,
            tenant,
            local_data_dir,
            input,
            format,
        } => handle_query(model, catalog, tenant, local_data_dir, input, format).await,
        Commands::Explain {
            model,
            catalog,
            tenant,
            local_data_dir,
            input,
        } => handle_explain(model, catalog, tenant, local_data_dir, input).await,
        Commands::Serve {
            model,
            catalog,
            local_data_dir,
            host,
            port,
        } => handle_serve(model, catalog, local_data_dir, host, port).await,
    };

    if let Err(err) = result {
        print_error(err);
    }
}

async fn handle_validate(model_path: PathBuf) -> Result<(), QuarryExecError> {
    SemanticModel::load_from_path(&model_path)?;
    let output = serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": { "validated": true },
        "meta": { "model": model_path.display().to_string() }
    });
    print_json(&output)?;
    Ok(())
}

async fn handle_query(
    model_path: PathBuf,
    catalog: String,
    tenant: String,
    local_data_dir: Option<PathBuf>,
    input: Option<PathBuf>,
    format: String,
) -> Result<(), QuarryExecError> {
    if format != "json" {
        return Err(QuarryExecError::Config(
            "Only --format json is supported in v1".to_string(),
        ));
    }

    let model = SemanticModel::load_from_path(&model_path)?;
    let query = read_query(input)?;
    let catalog_kind = catalog.parse::<CatalogKind>()?;

    let result = execute_query(
        &model,
        &query,
        catalog_kind,
        tenant.as_str(),
        local_data_dir,
    )
    .await?;
    print_json(&result)?;
    Ok(())
}

async fn handle_explain(
    model_path: PathBuf,
    catalog: String,
    tenant: String,
    local_data_dir: Option<PathBuf>,
    input: Option<PathBuf>,
) -> Result<(), QuarryExecError> {
    let model = SemanticModel::load_from_path(&model_path)?;
    let query = read_query(input)?;
    let catalog_kind = catalog.parse::<CatalogKind>()?;

    let result = explain_query(
        &model,
        &query,
        catalog_kind,
        tenant.as_str(),
        local_data_dir,
    )
    .await?;
    print_json(&result)?;
    Ok(())
}

async fn handle_serve(
    model_path: PathBuf,
    catalog: String,
    local_data_dir: Option<PathBuf>,
    host: String,
    port: u16,
) -> Result<(), QuarryExecError> {
    let model = Arc::new(SemanticModel::load_from_path(&model_path)?);
    let catalog_kind = catalog.parse::<CatalogKind>()?;

    let app_state = ServerState {
        model,
        default_catalog: catalog_kind,
        default_local_data_dir: local_data_dir,
        preagg_store: Arc::new(Mutex::new(PreAggregationStore::default())),
    };

    let app = Router::new()
        .route("/health", get(api_health))
        .route("/validate", get(api_validate))
        .route("/query", post(api_query))
        .route("/explain", post(api_explain))
        .route("/semantic/export", get(api_semantic_export))
        .route("/preaggregations", get(api_preaggregations_list))
        .route("/preaggregations/match", post(api_preaggregations_match))
        .route("/orchestration/warmup", post(api_orchestration_warmup))
        .route("/orchestration/refresh", post(api_orchestration_refresh))
        .route(
            "/orchestration/invalidate",
            post(api_orchestration_invalidate),
        )
        .with_state(app_state);

    let address = format!("{}:{}", host, port)
        .parse::<SocketAddr>()
        .map_err(|error| QuarryExecError::Config(format!("Invalid host/port: {error}")))?;

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| {
            QuarryExecError::Config(format!("Failed to bind server socket: {error}"))
        })?;

    eprintln!(
        "Quarry API server listening on http://{}",
        listener
            .local_addr()
            .map_err(|error| QuarryExecError::Config(error.to_string()))?
    );

    axum::serve(listener, app)
        .await
        .map_err(|error| QuarryExecError::Execution(format!("Server runtime failed: {error}")))
}

async fn api_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": { "service": "quarry-api", "ready": true },
        "meta": { "request_id": Uuid::new_v4().to_string() }
    }))
}

async fn api_validate(
    State(state): State<ServerState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.model.validate().map_err(QuarryExecError::Core)?;
    Ok(Json(serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": { "validated": true },
        "meta": { "request_id": Uuid::new_v4().to_string() }
    })))
}

async fn api_query(
    State(state): State<ServerState>,
    Json(request): Json<QueryApiRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let catalog_kind = resolve_catalog_kind(request.catalog.as_deref(), state.default_catalog)?;
    let local_data_dir = request
        .local_data_dir
        .or_else(|| state.default_local_data_dir.clone());

    let result = execute_query(
        &state.model,
        &request.query,
        catalog_kind,
        request.tenant_id.as_str(),
        local_data_dir,
    )
    .await?;

    let value = serde_json::to_value(result)
        .map_err(|error| QuarryExecError::Serialization(error.to_string()))?;
    Ok(Json(value))
}

async fn api_explain(
    State(state): State<ServerState>,
    Json(request): Json<QueryApiRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let catalog_kind = resolve_catalog_kind(request.catalog.as_deref(), state.default_catalog)?;
    let local_data_dir = request
        .local_data_dir
        .or_else(|| state.default_local_data_dir.clone());

    let result = explain_query(
        &state.model,
        &request.query,
        catalog_kind,
        request.tenant_id.as_str(),
        local_data_dir,
    )
    .await?;

    let value = serde_json::to_value(result)
        .map_err(|error| QuarryExecError::Serialization(error.to_string()))?;
    Ok(Json(value))
}

async fn api_semantic_export(
    State(state): State<ServerState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": state.model.export_catalog(),
        "meta": { "request_id": Uuid::new_v4().to_string() }
    })))
}

async fn api_preaggregations_list(
    State(state): State<ServerState>,
    Query(params): Query<PreAggregationListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.preagg_store.lock().await;
    let materializations = store.list(params.tenant_id.as_deref());

    Ok(Json(serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": {
            "definitions": &state.model.pre_aggregations,
            "materializations": materializations,
        },
        "meta": { "request_id": Uuid::new_v4().to_string() }
    })))
}

async fn api_preaggregations_match(
    State(state): State<ServerState>,
    Json(request): Json<PreAggregationMatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let matched: Option<PreAggregationMatch> = match_pre_aggregation(&state.model, &request.query);

    Ok(Json(serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": { "match": matched },
        "meta": { "request_id": Uuid::new_v4().to_string() }
    })))
}

async fn api_orchestration_warmup(
    State(state): State<ServerState>,
    Json(request): Json<OrchestrationMaterializeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = run_materialization(&state, &request, false).await?;
    Ok(Json(orchestration_success("warmup", result)))
}

async fn api_orchestration_refresh(
    State(state): State<ServerState>,
    Json(request): Json<OrchestrationMaterializeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = run_materialization(&state, &request, true).await?;
    Ok(Json(orchestration_success("refresh", result)))
}

async fn api_orchestration_invalidate(
    State(state): State<ServerState>,
    Json(request): Json<OrchestrationInvalidateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = state.preagg_store.lock().await;
    let removed = store.invalidate(
        request.tenant_id.as_deref(),
        request.pre_aggregation.as_deref(),
    );

    Ok(Json(serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": {
            "event": "invalidate",
            "removed": removed,
            "tenant_id": request.tenant_id,
            "pre_aggregation": request.pre_aggregation,
        },
        "meta": { "request_id": Uuid::new_v4().to_string() }
    })))
}

async fn run_materialization(
    state: &ServerState,
    request: &OrchestrationMaterializeRequest,
    force: bool,
) -> Result<MaterializeResult, ApiError> {
    let pre_aggregation_name = if let Some(name) = &request.pre_aggregation {
        name.clone()
    } else if let Some(query) = &request.query {
        let matched = match_pre_aggregation(&state.model, query).ok_or_else(|| {
            QuarryExecError::Config(
                "No matching pre-aggregation for provided query; pass pre_aggregation explicitly"
                    .to_string(),
            )
        })?;
        matched.name
    } else {
        return Err(QuarryExecError::Config(
            "Orchestration requires pre_aggregation or query".to_string(),
        )
        .into());
    };

    let pre_aggregation = state
        .model
        .pre_aggregation_by_name(pre_aggregation_name.as_str())
        .ok_or_else(|| {
            QuarryExecError::Config(format!(
                "Unknown pre-aggregation '{}'",
                pre_aggregation_name
            ))
        })?;

    let catalog_kind = resolve_catalog_kind(request.catalog.as_deref(), state.default_catalog)?;
    let local_data_dir = request
        .local_data_dir
        .clone()
        .or_else(|| state.default_local_data_dir.clone());

    let mut store = state.preagg_store.lock().await;
    let result = materialize_pre_aggregation(
        &state.model,
        pre_aggregation,
        catalog_kind,
        request.tenant_id.as_str(),
        local_data_dir,
        &mut store,
        force || request.force.unwrap_or(false),
    )
    .await?;

    Ok(result)
}

fn orchestration_success(event: &str, result: MaterializeResult) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "v1",
        "status": "ok",
        "data": {
            "event": event,
            "materialization": result,
        },
        "meta": { "request_id": Uuid::new_v4().to_string() }
    })
}

fn status_for_error(error: &QuarryExecError) -> StatusCode {
    match error {
        QuarryExecError::Core(QuarryCoreError::ModelLoad(_))
        | QuarryExecError::Core(QuarryCoreError::ModelValidation(_))
        | QuarryExecError::Core(QuarryCoreError::QueryValidation(_))
        | QuarryExecError::Core(QuarryCoreError::Unsupported(_))
        | QuarryExecError::Core(QuarryCoreError::Resolution(_))
        | QuarryExecError::Config(_) => StatusCode::BAD_REQUEST,
        QuarryExecError::Catalog(_) => StatusCode::BAD_GATEWAY,
        QuarryExecError::Execution(_) | QuarryExecError::Serialization(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn resolve_catalog_kind(
    value: Option<&str>,
    default: CatalogKind,
) -> Result<CatalogKind, QuarryExecError> {
    match value {
        Some(raw) => raw.parse::<CatalogKind>(),
        None => Ok(default),
    }
}

fn read_query(input: Option<PathBuf>) -> Result<SemanticQuery, QuarryExecError> {
    let payload = if let Some(path) = input {
        fs::read_to_string(&path)
            .map_err(|error| QuarryExecError::Config(format!("{}: {error}", path.display())))?
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|error| QuarryExecError::Config(error.to_string()))?;

        if buf.trim().is_empty() {
            return Err(QuarryExecError::Config(
                "Missing semantic query input. Pass --input <file> or pipe JSON via stdin."
                    .to_string(),
            ));
        }

        buf
    };

    SemanticQuery::parse_json(&payload).map_err(QuarryExecError::Core)
}

fn print_json(value: &impl serde::Serialize) -> Result<(), QuarryExecError> {
    let output = serde_json::to_string_pretty(value)
        .map_err(|error| QuarryExecError::Serialization(error.to_string()))?;
    println!("{output}");
    Ok(())
}

fn print_error(err: QuarryExecError) -> ! {
    let request_id = Uuid::new_v4().to_string();
    let (message, suggestions, details) = match &err {
        QuarryExecError::Core(QuarryCoreError::ModelValidation(issues))
        | QuarryExecError::Core(QuarryCoreError::QueryValidation(issues)) => (
            err.to_string(),
            issues
                .iter()
                .flat_map(|issue| issue.suggestions.clone())
                .collect(),
            serde_json::json!({ "issues": issues }),
        ),
        QuarryExecError::Core(core) => (core.to_string(), Vec::new(), serde_json::json!({})),
        _ => (err.to_string(), Vec::new(), serde_json::json!({})),
    };

    let envelope = ErrorEnvelope {
        schema_version: "v1",
        status: "error",
        error: QueryError {
            code: err.code().to_string(),
            message,
            suggestions,
            details,
        },
        meta: quarry_exec::result::ErrorMeta { request_id },
    };

    let fallback = format!(
        "{{\"schema_version\":\"v1\",\"status\":\"error\",\"error\":{{\"code\":\"{}\",\"message\":\"{}\",\"suggestions\":[],\"details\":{{}}}},\"meta\":{{\"request_id\":\"unknown\"}}}}",
        err.code(),
        err.to_string().replace('"', "\\\"")
    );

    let rendered = serde_json::to_string_pretty(&envelope).unwrap_or(fallback);
    eprintln!("{rendered}");
    std::process::exit(err.exit_code());
}
