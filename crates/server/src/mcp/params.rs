use rmcp::{
    ErrorData,
    handler::server::{common::FromContextPart, tool::ToolCallContext},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Parameters<P>(pub P);

// Backward-compatible alias for internal callers.
pub type VkParameters<P> = Parameters<P>;

impl<P: JsonSchema> JsonSchema for Parameters<P> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        P::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        P::json_schema(generator)
    }
}

fn hint_for_path(path: &str) -> Option<&'static str> {
    if path.ends_with("project_id") {
        return Some("Use list_projects to get a valid project_id UUID.");
    }
    if path.ends_with("task_id") {
        return Some("Use list_tasks to get a valid task_id UUID.");
    }
    if path.ends_with("repo_id") {
        return Some("Use list_repos to get a valid repo_id UUID.");
    }
    if path.ends_with("attempt_id") {
        return Some("Use list_task_attempts to get a valid attempt_id UUID.");
    }
    if path.ends_with("session_id") {
        return Some(
            "Use tail_attempt_feed to get latest_session_id, or use list_task_attempts to locate an attempt first.",
        );
    }
    None
}

#[derive(Debug, Clone)]
struct NextTool {
    tool: &'static str,
    args: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct FieldGuidance {
    placeholder: &'static str,
    next_tools: Vec<NextTool>,
}

fn field_guidance(field: &str) -> Option<FieldGuidance> {
    match field {
        "project_id" => Some(FieldGuidance {
            placeholder: "<project_id>",
            next_tools: vec![NextTool {
                tool: "list_projects",
                args: Map::new(),
            }],
        }),
        "repo_id" => Some(FieldGuidance {
            placeholder: "<repo_id>",
            next_tools: vec![
                NextTool {
                    tool: "list_projects",
                    args: Map::new(),
                },
                NextTool {
                    tool: "list_repos",
                    args: Map::from_iter([(
                        "project_id".to_string(),
                        Value::String("<project_id>".to_string()),
                    )]),
                },
            ],
        }),
        "task_id" => Some(FieldGuidance {
            placeholder: "<task_id>",
            next_tools: vec![
                NextTool {
                    tool: "list_projects",
                    args: Map::new(),
                },
                NextTool {
                    tool: "list_tasks",
                    args: Map::from_iter([(
                        "project_id".to_string(),
                        Value::String("<project_id>".to_string()),
                    )]),
                },
            ],
        }),
        "attempt_id" => Some(FieldGuidance {
            placeholder: "<attempt_id>",
            next_tools: vec![
                NextTool {
                    tool: "list_task_attempts",
                    args: Map::from_iter([(
                        "task_id".to_string(),
                        Value::String("<task_id>".to_string()),
                    )]),
                },
                NextTool {
                    tool: "tail_attempt_feed",
                    args: Map::from_iter([(
                        "attempt_id".to_string(),
                        Value::String("<attempt_id>".to_string()),
                    )]),
                },
            ],
        }),
        "session_id" => Some(FieldGuidance {
            placeholder: "<session_id>",
            next_tools: vec![NextTool {
                tool: "tail_attempt_feed",
                args: Map::from_iter([(
                    "attempt_id".to_string(),
                    Value::String("<attempt_id>".to_string()),
                )]),
            }],
        }),
        "executor" => Some(FieldGuidance {
            placeholder: "<executor>",
            next_tools: vec![NextTool {
                tool: "list_executors",
                args: Map::new(),
            }],
        }),
        "approval_id" | "execution_process_id" => Some(FieldGuidance {
            placeholder: if field == "approval_id" {
                "<approval_id>"
            } else {
                "<execution_process_id>"
            },
            next_tools: vec![NextTool {
                tool: "list_approvals",
                args: Map::from_iter([(
                    "attempt_id".to_string(),
                    Value::String("<attempt_id>".to_string()),
                )]),
            }],
        }),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn guidance_referenced_tool_names() -> Vec<&'static str> {
    let fields = [
        "project_id",
        "repo_id",
        "task_id",
        "attempt_id",
        "session_id",
        "executor",
        "approval_id",
        "execution_process_id",
    ];

    let mut out: Vec<&'static str> = Vec::new();
    for field in fields {
        let Some(guidance) = field_guidance(field) else {
            continue;
        };
        for step in guidance.next_tools {
            if !out.contains(&step.tool) {
                out.push(step.tool);
            }
        }
    }
    out
}

fn next_tools_json(tools: Vec<NextTool>) -> Value {
    Value::Array(
        tools
            .into_iter()
            .map(|tool| {
                json!({
                    "tool": tool.tool,
                    "args": Value::Object(tool.args),
                })
            })
            .collect(),
    )
}

fn is_sensitive_field_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "apikey",
        "authorization",
        "bearer",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn redact_value_for_example_args(field: &str, value: &Value) -> Value {
    if is_sensitive_field_name(field) {
        return Value::String("<redacted>".to_string());
    }
    value.clone()
}

fn redact_example_args(provided: &Map<String, Value>) -> Map<String, Value> {
    provided
        .iter()
        .map(|(key, value)| (key.clone(), redact_value_for_example_args(key, value)))
        .collect()
}

fn summarize_value_for_details(path: &str, value: &Value) -> Value {
    if is_sensitive_field_name(path) {
        return Value::String("<redacted>".to_string());
    }

    match value {
        Value::String(s) => {
            const PREVIEW: usize = 120;
            if s.len() <= 200 {
                Value::String(s.clone())
            } else {
                json!({
                    "value_len": s.len(),
                    "value_preview": s.chars().take(PREVIEW).collect::<String>(),
                    "truncated": true,
                })
            }
        }
        other => other.clone(),
    }
}

fn example_args_for_retry(
    provided: &Map<String, Value>,
    missing_fields: &[String],
) -> Map<String, Value> {
    let mut out = redact_example_args(provided);
    for field in missing_fields {
        if out.contains_key(field) {
            continue;
        }
        let placeholder = field_guidance(field.as_str())
            .map(|g| g.placeholder)
            .unwrap_or("<value>");
        out.insert(field.clone(), Value::String(placeholder.to_string()));
    }
    out
}

pub(crate) fn invalid_params_payload(
    code: &'static str,
    hint: String,
    details: Map<String, Value>,
) -> Value {
    json!({
        "code": code,
        "retryable": false,
        "hint": hint,
        "details": Value::Object(details),
    })
}

fn best_effort_field_from_path(path: &str) -> Option<&str> {
    if path == "." || path.is_empty() {
        return None;
    }

    let last = path.split('.').last().unwrap_or(path);
    Some(last.split('[').next().unwrap_or(last))
}

fn parse_missing_field_name(error: &str) -> Option<String> {
    let prefix = "missing field `";
    if let Some(start) = error.find(prefix) {
        let rest = &error[start + prefix.len()..];
        if let Some(end) = rest.find('`') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn parse_unknown_field_name(error: &str) -> Option<String> {
    let prefix = "unknown field `";
    if let Some(start) = error.find(prefix) {
        let rest = &error[start + prefix.len()..];
        if let Some(end) = rest.find('`') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn parse_expected_fields(error: &str) -> Vec<String> {
    let expected = "expected one of `";
    if let Some(start) = error.find(expected) {
        let rest = &error[start + expected.len()..];
        let mut out = Vec::new();
        let mut remaining = rest;
        while let Some(end) = remaining.find('`') {
            let field = &remaining[..end];
            out.push(field.to_string());
            let next = &remaining[end + 1..];
            if let Some(next_tick) = next.find('`') {
                remaining = &next[next_tick + 1..];
            } else {
                break;
            }
        }
        return out;
    }
    Vec::new()
}

fn classify_invalid_params<P: DeserializeOwned + JsonSchema>(
    tool_name: &str,
    provided: &Map<String, Value>,
    path: &str,
    error_text: &str,
) -> ErrorData {
    let unknown_field = parse_unknown_field_name(error_text);
    if let Some(unknown) = unknown_field {
        let expected = parse_expected_fields(error_text);
        let hint = if expected.is_empty() {
            format!("Remove unknown field '{unknown}' and retry.")
        } else {
            format!(
                "Unknown field '{unknown}'. Use one of: {}.",
                expected.join(", ")
            )
        };

        let mut details = Map::new();
        details.insert("tool".to_string(), json!(tool_name));
        details.insert("unknown_fields".to_string(), json!([unknown]));
        if !expected.is_empty() {
            details.insert("expected_fields".to_string(), json!(expected));
        }
        details.insert("error".to_string(), json!(error_text));
        details.insert("next_tools".to_string(), json!([]));
        details.insert(
            "example_args".to_string(),
            Value::Object(redact_example_args(provided)),
        );

        return ErrorData::invalid_params(
            "Unknown field(s) provided",
            Some(invalid_params_payload("unknown_field", hint, details)),
        );
    }

    let missing_fields = if error_text.contains("missing field") {
        parse_missing_field_name(error_text).into_iter().collect()
    } else {
        Vec::new()
    };

    if !missing_fields.is_empty() {
        let mut next_tools = Vec::new();
        for field in &missing_fields {
            if let Some(guidance) = field_guidance(field) {
                next_tools.extend(guidance.next_tools);
            }
        }
        let hint = if missing_fields.len() == 1 {
            let field = &missing_fields[0];
            if let Some(guidance) = field_guidance(field) {
                if guidance.next_tools.len() == 1 {
                    format!(
                        "Call {} to obtain {}, then retry {}.",
                        guidance.next_tools[0].tool, field, tool_name
                    )
                } else {
                    let steps = guidance
                        .next_tools
                        .iter()
                        .map(|t| t.tool)
                        .collect::<Vec<_>>()
                        .join(", then ");
                    format!("Call {steps} to obtain {field}, then retry {tool_name}.")
                }
            } else {
                format!("Provide '{}' and retry {}.", field, tool_name)
            }
        } else {
            format!(
                "Provide missing required fields ({}) and retry {}.",
                missing_fields.join(", "),
                tool_name
            )
        };

        let mut details = Map::new();
        details.insert("tool".to_string(), json!(tool_name));
        details.insert("missing_fields".to_string(), json!(missing_fields));
        details.insert("error".to_string(), json!(error_text));
        details.insert("next_tools".to_string(), next_tools_json(next_tools));
        details.insert(
            "example_args".to_string(),
            Value::Object(example_args_for_retry(provided, &missing_fields)),
        );

        let headline = format!("Missing required field(s): {}", missing_fields.join(", "));

        return ErrorData::invalid_params(
            headline,
            Some(invalid_params_payload("missing_required", hint, details)),
        );
    }

    if error_text.to_ascii_lowercase().contains("uuid")
        || best_effort_field_from_path(path)
            .is_some_and(|field| field.ends_with("_id") || field.ends_with("_token"))
            && (error_text.contains("invalid length")
                || error_text.contains("expected")
                || error_text.contains("UUID"))
    {
        let field = best_effort_field_from_path(path).unwrap_or(path);
        let hint = if let Some(guidance) = field_guidance(field) {
            if guidance.next_tools.is_empty() {
                format!("Provide a valid UUID for '{field}' and retry.")
            } else {
                format!(
                    "Provide a valid UUID for '{field}'. Use {} to obtain one.",
                    guidance.next_tools[0].tool
                )
            }
        } else {
            format!("Provide a valid UUID for '{field}' and retry.")
        };

        let mut details = Map::new();
        details.insert("tool".to_string(), json!(tool_name));
        details.insert("path".to_string(), json!(path));
        details.insert("error".to_string(), json!(error_text));
        if let Some(field_value) = provided.get(field) {
            details.insert(
                "value".to_string(),
                summarize_value_for_details(field, field_value),
            );
        }
        if let Some(guidance) = field_guidance(field) {
            details.insert(
                "next_tools".to_string(),
                next_tools_json(guidance.next_tools),
            );
        } else {
            details.insert("next_tools".to_string(), json!([]));
        }
        details.insert(
            "example_args".to_string(),
            Value::Object(redact_example_args(provided)),
        );

        return ErrorData::invalid_params(
            "Invalid UUID value",
            Some(invalid_params_payload("invalid_uuid", hint, details)),
        );
    }

    let hint = hint_for_path(path)
        .map(|h| h.to_string())
        .unwrap_or_else(|| "Check tool parameters and retry.".to_string());

    let mut details = Map::new();
    details.insert("tool".to_string(), json!(tool_name));
    if path != "." {
        details.insert("path".to_string(), json!(path));
    }
    details.insert("error".to_string(), json!(error_text));
    details.insert("next_tools".to_string(), json!([]));
    details.insert(
        "example_args".to_string(),
        Value::Object(redact_example_args(provided)),
    );

    ErrorData::invalid_params(
        "Invalid tool parameters",
        Some(invalid_params_payload("invalid_params", hint, details)),
    )
}

fn parse_parameters<P: DeserializeOwned + JsonSchema>(
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<P, ErrorData> {
    let value = Value::Object(arguments.clone());
    let payload = serde_json::to_string(&value).unwrap_or_default();
    let mut deserializer = serde_json::Deserializer::from_str(&payload);
    let parsed: Result<P, serde_path_to_error::Error<serde_json::Error>> =
        serde_path_to_error::deserialize(&mut deserializer);

    match parsed {
        Ok(params) => Ok(params),
        Err(err) => {
            let path = err.path().to_string();
            let inner = err.into_inner();
            let error_text = inner.to_string();
            Err(classify_invalid_params::<P>(
                tool_name,
                &arguments,
                &path,
                &error_text,
            ))
        }
    }
}

impl<S, P> FromContextPart<ToolCallContext<'_, S>> for Parameters<P>
where
    P: DeserializeOwned + JsonSchema,
{
    fn from_context_part(context: &mut ToolCallContext<'_, S>) -> Result<Self, ErrorData> {
        let arguments = context.arguments.take().unwrap_or_default();
        parse_parameters::<P>(&context.name.to_string(), arguments).map(Parameters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct TestListTasksRequest {
        project_id: uuid::Uuid,
        limit: Option<i32>,
    }

    #[test]
    fn guidance_registry_project_id() {
        let guidance = field_guidance("project_id").expect("guidance");
        assert_eq!(guidance.placeholder, "<project_id>");
        assert_eq!(guidance.next_tools.len(), 1);
        assert_eq!(guidance.next_tools[0].tool, "list_projects");
        assert!(guidance.next_tools[0].args.is_empty());
    }

    #[test]
    fn guidance_registry_repo_id() {
        let guidance = field_guidance("repo_id").expect("guidance");
        assert_eq!(guidance.placeholder, "<repo_id>");
        assert_eq!(guidance.next_tools.len(), 2);
        assert_eq!(guidance.next_tools[0].tool, "list_projects");
        assert_eq!(guidance.next_tools[1].tool, "list_repos");
        assert_eq!(
            guidance.next_tools[1]
                .args
                .get("project_id")
                .and_then(|v| v.as_str()),
            Some("<project_id>")
        );
    }

    #[test]
    fn invalid_params_missing_required_includes_next_tools_and_example_args() {
        let args = Map::new();
        let err = parse_parameters::<TestListTasksRequest>("list_tasks", args)
            .expect_err("expected invalid params");

        assert!(err.message.contains("Missing required field(s):"));
        let payload = err.data.expect("data");
        assert_eq!(
            payload.get("code").and_then(|v| v.as_str()),
            Some("missing_required")
        );
        assert_eq!(
            payload.get("retryable").and_then(|v| v.as_bool()),
            Some(false)
        );
        let details = payload
            .get("details")
            .and_then(|v| v.as_object())
            .expect("details");
        assert_eq!(
            details
                .get("missing_fields")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()),
            Some(vec!["project_id"])
        );
        assert!(details.get("next_tools").is_some());
        assert!(details.get("example_args").is_some());
    }

    #[test]
    fn invalid_params_unknown_field_is_structured() {
        let mut args = Map::new();
        args.insert("projectId".to_string(), Value::String("x".to_string()));
        let err = parse_parameters::<TestListTasksRequest>("list_tasks", args)
            .expect_err("expected invalid params");
        let payload = err.data.expect("data");
        assert_eq!(
            payload.get("code").and_then(|v| v.as_str()),
            Some("unknown_field")
        );
        let details = payload
            .get("details")
            .and_then(|v| v.as_object())
            .expect("details");
        assert!(
            details
                .get("unknown_fields")
                .and_then(|v| v.as_array())
                .is_some()
        );
    }

    #[test]
    fn invalid_params_invalid_uuid_echoes_value() {
        let mut args = Map::new();
        args.insert(
            "project_id".to_string(),
            Value::String("not-a-uuid".to_string()),
        );
        let err = parse_parameters::<TestListTasksRequest>("list_tasks", args)
            .expect_err("expected invalid params");
        let payload = err.data.expect("data");
        assert_eq!(
            payload.get("code").and_then(|v| v.as_str()),
            Some("invalid_uuid")
        );
        let details = payload
            .get("details")
            .and_then(|v| v.as_object())
            .expect("details");
        assert_eq!(
            details.get("path").and_then(|v| v.as_str()),
            Some("project_id")
        );
        assert_eq!(
            details.get("value").and_then(|v| v.as_str()),
            Some("not-a-uuid")
        );
    }
}
