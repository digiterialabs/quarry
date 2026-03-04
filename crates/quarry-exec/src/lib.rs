pub mod catalog;
pub mod engine;
pub mod error;
pub mod preagg;
pub mod result;

pub use catalog::{CatalogAdapter, CatalogKind, GlueCatalogAdapter, LocalCatalogAdapter};
pub use engine::{execute_query, explain_query};
pub use error::QuarryExecError;
pub use preagg::{
    match_pre_aggregation, materialize_pre_aggregation, MaterializeResult, PreAggregationMatch,
    PreAggregationState, PreAggregationStore,
};
pub use result::{ErrorEnvelope, QueryError, QueryMeta, QuerySuccessEnvelope};
