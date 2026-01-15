use rmcp::{
    ErrorData,
    handler::server::{common::FromContextPart, tool::ToolCallContext},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

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
        return Some("Use get_context or list_task_attempts to get a valid session_id UUID.");
    }
    None
}

impl<S, P> FromContextPart<ToolCallContext<'_, S>> for Parameters<P>
where
    P: DeserializeOwned,
{
    fn from_context_part(
        context: &mut ToolCallContext<'_, S>,
    ) -> Result<Self, ErrorData> {
        let arguments = context.arguments.take().unwrap_or_default();
        let value = Value::Object(arguments);
        let payload = serde_json::to_string(&value).unwrap_or_default();
        let mut deserializer = serde_json::Deserializer::from_str(&payload);
        let parsed: Result<P, serde_path_to_error::Error<serde_json::Error>> =
            serde_path_to_error::deserialize(&mut deserializer);

        match parsed {
            Ok(params) => Ok(Parameters(params)),
            Err(err) => {
                let path = err.path().to_string();
                let inner = err.into_inner();
                let hint = hint_for_path(&path);

                let mut data = serde_json::Map::new();
                data.insert("tool".to_string(), json!(context.name.to_string()));
                data.insert("code".to_string(), json!("invalid_params"));
                if path != "." {
                    data.insert("path".to_string(), json!(path));
                }
                data.insert("error".to_string(), json!(inner.to_string()));
                if let Some(hint) = hint {
                    data.insert("hint".to_string(), json!(hint));
                }

                Err(ErrorData::invalid_params(
                    "Invalid tool parameters",
                    Some(Value::Object(data)),
                ))
            }
        }
    }
}
