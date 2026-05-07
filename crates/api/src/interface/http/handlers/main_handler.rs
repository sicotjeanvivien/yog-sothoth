use std::collections::HashMap;

use crate::interface::{HttpRequest, HttpResponse, StatusCode};

#[derive(Clone)]
pub(crate) struct MainHandler {}

impl MainHandler {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) async fn index(&self, _request: HttpRequest) -> Result<HttpResponse, HttpResponse> {
        Ok(HttpResponse::new(
            StatusCode::OK,
            Self::build_header(),
            Some("Api conneted".to_string()),
        ))
    }

    fn build_header() -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Content-Type-Options".to_string(), "nosniff".to_string());
        headers.insert("X-Frame-Options".to_string(), "DENY".to_string());
        headers.insert("Access-Control-Allow-Origin".to_string(), "*".to_string());
        headers.insert(
            "Access-Control-Allow-Methods".to_string(),
            "GET, POST, PATCH, DELETE".to_string(),
        );
        headers.insert(
            "Access-Control-Allow-Headers".to_string(),
            "Content-Type".to_string(),
        );
        headers.insert("Cache-Control".to_string(), "no-store".to_string());
        headers
    }
}
