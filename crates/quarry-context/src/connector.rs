use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use serde_json::Value;
use walkdir::WalkDir;

use crate::error::QuarryContextError;

#[derive(Debug, Clone, Copy)]
pub enum ConnectorKind {
    Filesystem,
    UrlList,
}

impl ConnectorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::UrlList => "url_list",
        }
    }
}

impl FromStr for ConnectorKind {
    type Err = QuarryContextError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "filesystem" => Ok(Self::Filesystem),
            "url_list" => Ok(Self::UrlList),
            _ => Err(QuarryContextError::invalid(format!(
                "unsupported connector '{}'; expected filesystem or url_list",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub source_uri: String,
    pub title: String,
    pub content: String,
    pub metadata: Value,
}

pub fn load_documents(
    connector: ConnectorKind,
    config: &Value,
) -> Result<Vec<SourceDocument>, QuarryContextError> {
    match connector {
        ConnectorKind::Filesystem => load_filesystem_documents(config),
        ConnectorKind::UrlList => load_url_documents(config),
    }
}

fn load_filesystem_documents(config: &Value) -> Result<Vec<SourceDocument>, QuarryContextError> {
    let obj = config
        .as_object()
        .ok_or_else(|| QuarryContextError::invalid("filesystem config must be a JSON object"))?;

    let paths = obj.get("paths").and_then(Value::as_array).ok_or_else(|| {
        QuarryContextError::invalid("filesystem config requires 'paths' array of strings")
    })?;

    if paths.is_empty() {
        return Err(QuarryContextError::invalid(
            "filesystem config paths must not be empty",
        ));
    }

    let recursive = obj
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let extensions = obj
        .get("extensions")
        .and_then(Value::as_array)
        .map(|values| parse_extensions(values.as_slice()))
        .transpose()?
        .unwrap_or_else(default_extensions);

    let mut documents = Vec::new();
    for value in paths {
        let path = value.as_str().ok_or_else(|| {
            QuarryContextError::invalid("filesystem config paths entries must be strings")
        })?;
        let root = PathBuf::from(path);
        if root.is_file() {
            if should_include_file(&root, &extensions) {
                if let Some(doc) = read_file_document(&root)? {
                    documents.push(doc);
                }
            }
            continue;
        }

        if !root.exists() {
            return Err(QuarryContextError::invalid(format!(
                "filesystem path does not exist: {}",
                root.display()
            )));
        }

        let walker = WalkDir::new(&root)
            .follow_links(false)
            .max_depth(if recursive { usize::MAX } else { 1 });
        for entry in walker {
            let entry = entry.map_err(|error| QuarryContextError::database(error.to_string()))?;
            if !entry.file_type().is_file() {
                continue;
            }

            let file_path = entry.path();
            if !should_include_file(file_path, &extensions) {
                continue;
            }

            if let Some(doc) = read_file_document(file_path)? {
                documents.push(doc);
            }
        }
    }

    Ok(documents)
}

fn parse_extensions(values: &[Value]) -> Result<HashSet<String>, QuarryContextError> {
    let mut set = HashSet::new();
    for value in values {
        let ext = value.as_str().ok_or_else(|| {
            QuarryContextError::invalid("filesystem config extensions entries must be strings")
        })?;
        let cleaned = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if !cleaned.is_empty() {
            set.insert(cleaned);
        }
    }
    Ok(set)
}

fn default_extensions() -> HashSet<String> {
    ["md", "txt", "json", "csv", "html", "htm", "xml"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn should_include_file(path: &Path, extensions: &HashSet<String>) -> bool {
    if extensions.is_empty() {
        return true;
    }

    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    extensions.contains(&ext.to_ascii_lowercase())
}

fn read_file_document(path: &Path) -> Result<Option<SourceDocument>, QuarryContextError> {
    let content = fs::read_to_string(path).map_err(|error| {
        QuarryContextError::database(format!("failed to read {}: {error}", path.display()))
    })?;
    if content.trim().is_empty() {
        return Ok(None);
    }

    let source_uri = format!(
        "file://{}",
        path.canonicalize().unwrap_or(path.to_path_buf()).display()
    );
    let title = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("untitled")
        .to_string();

    Ok(Some(SourceDocument {
        source_uri,
        title,
        content,
        metadata: serde_json::json!({
            "kind": "filesystem",
            "path": path.display().to_string(),
        }),
    }))
}

fn load_url_documents(config: &Value) -> Result<Vec<SourceDocument>, QuarryContextError> {
    let obj = config
        .as_object()
        .ok_or_else(|| QuarryContextError::invalid("url_list config must be a JSON object"))?;

    let urls = obj
        .get("urls")
        .and_then(Value::as_array)
        .ok_or_else(|| QuarryContextError::invalid("url_list config requires 'urls' array"))?;
    if urls.is_empty() {
        return Err(QuarryContextError::invalid(
            "url_list config urls must not be empty",
        ));
    }

    let timeout_seconds = obj
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(10);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|error| QuarryContextError::database(error.to_string()))?;

    let mut documents = Vec::new();
    for value in urls {
        let url = value
            .as_str()
            .ok_or_else(|| QuarryContextError::invalid("url_list urls must be strings"))?;
        let response = client.get(url).send().map_err(|error| {
            QuarryContextError::database(format!("failed to fetch {}: {error}", url))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(QuarryContextError::database(format!(
                "non-success response from {}: {}",
                url, status
            )));
        }

        let content = response.text().map_err(|error| {
            QuarryContextError::database(format!("failed to read body from {}: {error}", url))
        })?;
        if content.trim().is_empty() {
            continue;
        }

        documents.push(SourceDocument {
            source_uri: url.to_string(),
            title: url.to_string(),
            content,
            metadata: serde_json::json!({
                "kind": "url_list",
                "url": url,
                "http_status": status.as_u16(),
            }),
        });
    }

    Ok(documents)
}
