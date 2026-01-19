use axum::{Json, Router, response::Json as ResponseJson, routing::post};
use serde::{Deserialize, Serialize};
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

const KANBAN_OPENAI_API_BASE: &str = "KANBAN_OPENAI_API_BASE";
const KANBAN_OPENAI_API_KEY: &str = "KANBAN_OPENAI_API_KEY";
const KANBAN_OPENAI_DEFAULT_MODEL: &str = "KANBAN_OPENAI_DEFAULT_MODEL";
const OPENAI_API_BASE: &str = "OPENAI_API_BASE";
const OPENAI_API_KEY: &str = "OPENAI_API_KEY";
const OPENAI_DEFAULT_MODEL: &str = "OPENAI_DEFAULT_MODEL";

#[derive(Debug, Deserialize)]
pub struct TranslationRequest {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize)]
pub struct TranslationResponse {
    pub translated_text: String,
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMessageResponse>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessageResponse {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorResponse {
    error: Option<OpenAiError>,
}

#[derive(Debug, Deserialize)]
struct OpenAiError {
    message: Option<String>,
}

struct LlmConfig {
    base_url: String,
    api_key: String,
    model: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/translation", post(translate))
}

async fn translate(
    Json(payload): Json<TranslationRequest>,
) -> Result<ResponseJson<ApiResponse<TranslationResponse>>, ApiError> {
    if payload.text.trim().is_empty() {
        return Err(ApiError::BadRequest("Translation text is empty".to_string()));
    }

    let config = resolve_llm_config()?;
    let url = format_openai_url(&config.base_url);
    let system_prompt = build_system_prompt(&payload.source_lang, &payload.target_lang);

    let request_body = OpenAiChatRequest {
        model: config.model,
        messages: vec![
            OpenAiMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            OpenAiMessage {
                role: "user".to_string(),
                content: payload.text,
            },
        ],
        temperature: 0.2,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .bearer_auth(config.api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|err| ApiError::BadRequest(format!("Translation request failed: {}", err)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_openai_error(&body)
            .unwrap_or_else(|| body.trim().to_string())
            .trim()
            .to_string();
        let fallback = format!("Translation failed with status {}", status);
        let message = if message.is_empty() { fallback } else { message };
        return Err(ApiError::BadRequest(message));
    }

    let data = response
        .json::<OpenAiChatResponse>()
        .await
        .map_err(|err| ApiError::BadRequest(format!("Translation response invalid: {}", err)))?;

    let translated_text = data
        .choices
        .iter()
        .find_map(|choice| choice.message.as_ref()?.content.as_ref())
        .map(|text| text.to_string())
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| ApiError::BadRequest("Translation unavailable".to_string()))?;

    Ok(ResponseJson(ApiResponse::success(TranslationResponse {
        translated_text,
    })))
}

fn resolve_llm_config() -> Result<LlmConfig, ApiError> {
    let base_url = resolve_env(KANBAN_OPENAI_API_BASE, OPENAI_API_BASE)
        .ok_or_else(|| ApiError::BadRequest("Missing OpenAI API base URL".to_string()))?;
    let api_key = resolve_env(KANBAN_OPENAI_API_KEY, OPENAI_API_KEY)
        .ok_or_else(|| ApiError::BadRequest("Missing OpenAI API key".to_string()))?;
    let model = resolve_env(KANBAN_OPENAI_DEFAULT_MODEL, OPENAI_DEFAULT_MODEL)
        .ok_or_else(|| ApiError::BadRequest("Missing OpenAI default model".to_string()))?;

    Ok(LlmConfig {
        base_url,
        api_key,
        model,
    })
}

fn resolve_env(primary: &str, fallback: &str) -> Option<String> {
    std::env::var(primary)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var(fallback)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn format_openai_url(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{}/chat/completions", trimmed)
    } else {
        format!("{}/v1/chat/completions", trimmed)
    }
}

fn build_system_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        "You are a translation engine. Translate from {source} to {target}. \
Return only the translated text with original formatting preserved. Do not add commentary.",
        source = source_lang,
        target = target_lang
    )
}

fn parse_openai_error(body: &str) -> Option<String> {
    let parsed: OpenAiErrorResponse = serde_json::from_str(body).ok()?;
    parsed.error.and_then(|err| err.message)
}

#[cfg(test)]
mod tests {
    use super::{build_system_prompt, format_openai_url};

    #[test]
    fn format_openai_url_appends_v1() {
        assert_eq!(
            format_openai_url("https://example.com"),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            format_openai_url("https://example.com/"),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn format_openai_url_respects_existing_v1() {
        assert_eq!(
            format_openai_url("https://example.com/v1"),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            format_openai_url("https://example.com/v1/"),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn build_system_prompt_includes_languages() {
        let prompt = build_system_prompt("en", "zh-CN");
        assert!(prompt.contains("en"));
        assert!(prompt.contains("zh-CN"));
    }
}
