use axum::{
    body::Body,
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use reqwest::{StatusCode, header};
use rust_embed::RustEmbed;
use std::path::Path;

const HASHED_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const DEFAULT_CACHE_CONTROL: &str = "public, max-age=300";

#[derive(RustEmbed)]
#[folder = "../../frontend/dist"]
pub struct Assets;

pub async fn serve_frontend(uri: axum::extract::Path<String>) -> impl IntoResponse {
    let path = uri.trim_start_matches('/');
    serve_file(path).await
}

pub async fn serve_frontend_root() -> impl IntoResponse {
    serve_file("index.html").await
}

async fn serve_file(path: &str) -> impl IntoResponse + use<> {
    let file = Assets::get(path);

    match file {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let cache_control = cache_control_for_path(path);

            Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.as_ref()).unwrap(),
                )
                .header(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(cache_control),
                )
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => {
            // For SPA routing, serve index.html for unknown routes
            if let Some(index) = Assets::get("index.html") {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html"))
                    .header(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static(cache_control_for_path("index.html")),
                    )
                    .body(Body::from(index.data.into_owned()))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("404 Not Found"))
                    .unwrap()
            }
        }
    }
}

fn cache_control_for_path(path: &str) -> &'static str {
    if is_hashed_asset(path) {
        HASHED_CACHE_CONTROL
    } else {
        DEFAULT_CACHE_CONTROL
    }
}

fn is_hashed_asset(path: &str) -> bool {
    let file_name = match Path::new(path).file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return false,
    };

    let (stem, _ext) = match file_name.rsplit_once('.') {
        Some(parts) => parts,
        None => return false,
    };

    let (_prefix, hash) = match stem.rsplit_once('-') {
        Some(parts) => parts,
        None => return false,
    };

    if hash.len() < 8 {
        return false;
    }

    hash.chars().all(|ch| ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_assets_use_long_cache_control() {
        assert!(is_hashed_asset("assets/index-C2bHdHKB.js"));
        assert_eq!(
            cache_control_for_path("assets/index-C2bHdHKB.js"),
            HASHED_CACHE_CONTROL
        );
    }

    #[test]
    fn non_hashed_assets_use_short_cache_control() {
        assert!(!is_hashed_asset("ide/vscode-light.svg"));
        assert_eq!(
            cache_control_for_path("ide/vscode-light.svg"),
            DEFAULT_CACHE_CONTROL
        );
        assert_eq!(cache_control_for_path("index.html"), DEFAULT_CACHE_CONTROL);
    }
}
