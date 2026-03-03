use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    ModelValidation,
    QueryValidation,
    Resolver,
    Unsupported,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ModelValidation => "MODEL_VALIDATION_ERROR",
            Self::QueryValidation => "QUERY_VALIDATION_ERROR",
            Self::Resolver => "RESOLUTION_ERROR",
            Self::Unsupported => "UNSUPPORTED_FEATURE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidationIssue {
    pub code: &'static str,
    pub path: String,
    pub message: String,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Error)]
pub enum QuarryCoreError {
    #[error("failed to load semantic model: {0}")]
    ModelLoad(String),

    #[error("model validation failed")]
    ModelValidation(Vec<ValidationIssue>),

    #[error("query validation failed")]
    QueryValidation(Vec<ValidationIssue>),

    #[error("resolution failed: {0}")]
    Resolution(String),

    #[error("unsupported feature: {0}")]
    Unsupported(String),
}

impl QuarryCoreError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::ModelLoad(_) | Self::ModelValidation(_) => ErrorCode::ModelValidation,
            Self::QueryValidation(_) => ErrorCode::QueryValidation,
            Self::Resolution(_) => ErrorCode::Resolver,
            Self::Unsupported(_) => ErrorCode::Unsupported,
        }
    }

    pub fn issues(&self) -> Vec<ValidationIssue> {
        match self {
            Self::ModelValidation(issues) | Self::QueryValidation(issues) => issues.clone(),
            _ => Vec::new(),
        }
    }
}
