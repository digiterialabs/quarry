use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use quarry_core::{QuarryCoreError, SemanticModel, SemanticQuery};
use quarry_exec::{
    execute_query, explain_query, CatalogKind, ErrorEnvelope, QuarryExecError, QueryError,
};

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
    let request_id = uuid::Uuid::new_v4().to_string();
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
