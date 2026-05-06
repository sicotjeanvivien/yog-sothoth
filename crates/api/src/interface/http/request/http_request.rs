use std::collections::HashMap;

use crate::interface::{HttpError, HttpMethod, HttpResponse};

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct HttpRequest {
    pub(crate) method: HttpMethod,
    pub(crate) path: String,
    pub(crate) params: HashMap<String, String>,
    pub(crate) http_version: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Option<String>,
}

impl HttpRequest {
    pub(crate) fn new(
        method: HttpMethod,
        path: String,
        params: HashMap<String, String>,
        http_version: String,
        headers: HashMap<String, String>,
        body: Option<String>,
    ) -> Self {
        HttpRequest {
            method,
            path,
            params,
            http_version,
            headers,
            body,
        }
    }

    pub(crate) fn get_value_by_key(&self, key: String) -> Result<&String, HttpError> {
        if self.params.contains_key(&key) {
            if let Some(x) = self.params.get(&key) {
                return Ok(x);
            }
        }
        Err(HttpError::ParamNotFound(format!(
            "{} not found in path",
            key
        )))
    }

    pub(crate) fn get_body(&self) -> Result<String, HttpResponse> {
        self.body
            .clone()
            .ok_or_else(|| HttpError::BadRequest("body is not found".to_string()).into())
    }
}
