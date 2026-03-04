use std::collections::HashMap;

use datafusion::config::ConfigOptions;
use datafusion::error::Result as DataFusionResult;
use datafusion::logical_expr::{col, lit, LogicalPlan, LogicalPlanBuilder};
use datafusion::optimizer::analyzer::AnalyzerRule;

use crate::model::SemanticModel;

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
}

#[derive(Debug)]
pub struct TenantIsolationRule {
    tenant_context: TenantContext,
    table_tenant_columns: HashMap<String, String>,
}

impl TenantIsolationRule {
    pub fn new(model: &SemanticModel, tenant_context: TenantContext) -> Self {
        let table_tenant_columns = model
            .entities
            .iter()
            .map(|entity| (entity.table.clone(), entity.tenant_column.clone()))
            .collect::<HashMap<_, _>>();

        Self {
            tenant_context,
            table_tenant_columns,
        }
    }

    pub fn tenant_context(&self) -> &TenantContext {
        &self.tenant_context
    }

    pub fn apply_plan(&self, plan: LogicalPlan) -> DataFusionResult<LogicalPlan> {
        if let Some((table_name, tenant_col)) = self.table_tenant_columns.iter().next() {
            let predicate = col(format!("{}.{}", table_name, tenant_col))
                .eq(lit(self.tenant_context.tenant_id.clone()));
            return LogicalPlanBuilder::from(plan).filter(predicate)?.build();
        }

        Ok(plan)
    }
}

impl AnalyzerRule for TenantIsolationRule {
    fn name(&self) -> &str {
        "tenant_isolation"
    }

    fn analyze(&self, plan: LogicalPlan, _config: &ConfigOptions) -> DataFusionResult<LogicalPlan> {
        self.apply_plan(plan)
    }
}
