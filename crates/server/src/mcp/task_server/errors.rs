use super::*;

pub(super) const MCP_CODE_AMBIGUOUS_TARGET: &str = "ambiguous_target";
pub(super) const MCP_CODE_NO_SESSION_YET: &str = "no_session_yet";
pub(super) const MCP_CODE_BLOCKED_GUARDRAILS: &str = "blocked_guardrails";
pub(super) const MCP_CODE_MIXED_PAGINATION: &str = "mixed_pagination";
pub(super) const MCP_CODE_IDEMPOTENCY_CONFLICT: &str = "idempotency_conflict";
pub(super) const MCP_CODE_IDEMPOTENCY_IN_PROGRESS: &str = "idempotency_in_progress";
pub(super) const MCP_CODE_WAIT_MS_TOO_LARGE: &str = "wait_ms_too_large";
pub(super) const MCP_CODE_WAIT_MS_REQUIRES_AFTER_LOG_INDEX: &str =
    "wait_ms_requires_after_log_index";
pub(super) const MCP_CODE_ATTEMPT_CLAIM_REQUIRED: &str = "attempt_claim_required";
pub(super) const MCP_CODE_ATTEMPT_CLAIM_CONFLICT: &str = "attempt_claim_conflict";
pub(super) const MCP_CODE_INVALID_CONTROL_TOKEN: &str = "invalid_control_token";
pub(super) const MCP_CODE_PROFILE_POLICY_REJECTED: &str = "profile_policy_rejected";

#[derive(Debug)]
pub(super) enum ToolOrRpcError {
    Tool(CallToolResult),
    Rpc(ErrorData),
}

impl From<ErrorData> for ToolOrRpcError {
    fn from(err: ErrorData) -> Self {
        Self::Rpc(err)
    }
}

impl TaskServer {
    fn json_pretty_for_content(value: &Value) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    }

    pub(super) fn structured_ok(value: Value) -> CallToolResult {
        let pretty = Self::json_pretty_for_content(&value);
        CallToolResult {
            content: vec![Content::text(pretty)],
            structured_content: Some(value),
            is_error: Some(false),
            meta: None,
        }
    }

    pub(super) fn structured_error(value: Value) -> CallToolResult {
        let pretty = Self::json_pretty_for_content(&value);
        CallToolResult {
            content: vec![Content::text(pretty)],
            structured_content: Some(value),
            is_error: Some(true),
            meta: None,
        }
    }

    pub(super) fn success<T: Serialize>(data: &T) -> Result<CallToolResult, ErrorData> {
        let value = serde_json::to_value(data).map_err(|e| {
            ErrorData::internal_error(
                "Failed to serialize response",
                Some(json!({ "error": e.to_string() })),
            )
        })?;
        Ok(Self::structured_ok(value))
    }

    pub(super) fn err_value(v: Value) -> Result<CallToolResult, ErrorData> {
        Ok(Self::structured_error(v))
    }

    pub(super) fn err_payload<S: Into<String>>(
        msg: S,
        details: Option<Value>,
        hint: Option<String>,
        code: Option<&'static str>,
        retryable: Option<bool>,
    ) -> Value {
        let msg = msg.into();
        let code = code.unwrap_or("unknown_error");
        let retryable = retryable.unwrap_or(false);
        let hint = hint.unwrap_or_else(|| msg.clone());

        let mut details = match details {
            Some(Value::Object(map)) => Value::Object(map),
            Some(other) => json!({ "context": other }),
            None => json!({}),
        };
        if let Value::Object(map) = &mut details {
            map.entry("message".to_string())
                .or_insert_with(|| json!(msg));
        }

        json!({
            "code": code,
            "retryable": retryable,
            "hint": hint,
            "details": details,
        })
    }

    pub(super) fn err_with<S: Into<String>>(
        msg: S,
        details: Option<Value>,
        hint: Option<String>,
        code: Option<&'static str>,
        retryable: Option<bool>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::err_value(Self::err_payload(msg, details, hint, code, retryable))
    }

    pub(super) fn tool_error_from_api_error(
        tool: &'static str,
        err: ApiError,
        details: Value,
    ) -> Result<CallToolResult, ErrorData> {
        match err {
            ApiError::BadRequest(message) => Self::err_with(
                message,
                Some(details),
                Some("请求参数不合法。".to_string()),
                Some("invalid_argument"),
                Some(false),
            ),
            ApiError::Conflict(message) => Self::err_with(
                message,
                Some(details),
                Some(
                    "操作被阻止：请先解决冲突条件（例如停止运行中的进程或先还原任务）。"
                        .to_string(),
                ),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            ),
            ApiError::NotFound(message) => Self::err_with(
                message,
                Some(details),
                Some("目标不存在：请确认 id 是否正确。".to_string()),
                Some("not_found"),
                Some(false),
            ),
            ApiError::Forbidden(message) => Self::err_with(
                message,
                Some(details),
                Some("无权限执行此操作。".to_string()),
                Some("forbidden"),
                Some(false),
            ),
            ApiError::Database(DbErr::RecordNotFound(message)) => Self::err_with(
                message,
                Some(details),
                Some("目标不存在：请确认 id 是否正确。".to_string()),
                Some("not_found"),
                Some(false),
            ),
            other => Err(ErrorData::internal_error(
                format!("Tool {tool} failed"),
                Some(json!({ "error": other.to_string(), "tool": tool, "details": details })),
            )),
        }
    }
}
