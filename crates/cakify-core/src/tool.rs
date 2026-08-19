use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};
use thiserror::Error;

use crate::ToolDefinition;

pub const CURRENT_TIME_TOOL_NAME: &str = "get_current_time";

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ToolExecutionError {
    message: String,
}

impl ToolExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn public_message(&self) -> &str {
        &self.message
    }
}

pub trait ToolExecutor: Send + Sync + 'static {
    fn execute(
        &self,
        name: &str,
        arguments_json: &str,
        cancellation: Arc<AtomicBool>,
    ) -> Result<String, ToolExecutionError>;
}

pub struct BuiltinToolExecutor;

impl ToolExecutor for BuiltinToolExecutor {
    fn execute(
        &self,
        name: &str,
        arguments_json: &str,
        cancellation: Arc<AtomicBool>,
    ) -> Result<String, ToolExecutionError> {
        if cancellation.load(Ordering::Acquire) {
            return Err(ToolExecutionError::new("工具调用已取消"));
        }
        if name != CURRENT_TIME_TOOL_NAME {
            return Err(ToolExecutionError::new("请求的工具不可用"));
        }
        let arguments: Value = serde_json::from_str(arguments_json)
            .map_err(|_| ToolExecutionError::new("工具参数不是有效 JSON"))?;
        if !arguments.is_object() {
            return Err(ToolExecutionError::new("工具参数必须是 JSON object"));
        }
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ToolExecutionError::new("系统时间不可用"))?;
        let unix_milliseconds = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Ok(json!({
            "timezone": "UTC",
            "unix_milliseconds": unix_milliseconds,
        })
        .to_string())
    }
}

pub fn builtin_tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: CURRENT_TIME_TOOL_NAME.to_owned(),
        description: "Return the current UTC time as a Unix timestamp in milliseconds.".to_owned(),
        parameters_json: r#"{"type":"object","properties":{},"additionalProperties":false}"#
            .to_owned(),
    }]
}

pub(crate) struct DisabledToolExecutor;

impl ToolExecutor for DisabledToolExecutor {
    fn execute(
        &self,
        _name: &str,
        _arguments_json: &str,
        _cancellation: Arc<AtomicBool>,
    ) -> Result<String, ToolExecutionError> {
        Err(ToolExecutionError::new("工具执行器未启用"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_clock_returns_bounded_json_without_external_processes() {
        let output = BuiltinToolExecutor
            .execute(
                CURRENT_TIME_TOOL_NAME,
                "{}",
                Arc::new(AtomicBool::new(false)),
            )
            .expect("clock output");
        let value: Value = serde_json::from_str(&output).expect("clock JSON");
        assert_eq!(value["timezone"], "UTC");
        assert!(value["unix_milliseconds"].as_u64().is_some());
        assert!(output.len() < 128);
    }

    #[test]
    fn builtin_executor_rejects_unknown_tools_and_cancelled_calls() {
        assert!(BuiltinToolExecutor
            .execute("shell", "{}", Arc::new(AtomicBool::new(false)))
            .is_err());
        assert!(BuiltinToolExecutor
            .execute(
                CURRENT_TIME_TOOL_NAME,
                "{}",
                Arc::new(AtomicBool::new(true)),
            )
            .is_err());
    }
}
