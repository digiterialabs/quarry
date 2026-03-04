use thiserror::Error;

use quarry_core::QuarryCoreError;

#[derive(Debug, Error)]
pub enum QuarryExecError {
    #[error("core error: {0}")]
    Core(#[from] QuarryCoreError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("catalog error: {0}")]
    Catalog(String),

    #[error("execution error: {0}")]
    Execution(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl QuarryExecError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Core(core) => match core {
                QuarryCoreError::ModelLoad(_)
                | QuarryCoreError::ModelValidation(_)
                | QuarryCoreError::QueryValidation(_) => 2,
                QuarryCoreError::Resolution(_) => 4,
                QuarryCoreError::Unsupported(_) => 4,
            },
            Self::Config(_) | Self::Catalog(_) => 3,
            Self::Execution(_) => 4,
            Self::Serialization(_) => 5,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Core(core) => core.code().as_str(),
            Self::Config(_) => "CONFIG_ERROR",
            Self::Catalog(_) => "CATALOG_ERROR",
            Self::Execution(_) => "EXECUTION_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
        }
    }
}
