use crate::{ActionRequest, Principal};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub verified: bool,
    pub data: Value,
}

pub trait RestrictedExecutor {
    fn execute(
        &self,
        principal: &Principal,
        action: &ActionRequest,
    ) -> Result<ExecutionResult, &'static str>;
}
