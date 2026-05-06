use crate::interface::{ErrorHandler, HttpError, StatusCode};
use std::{collections::HashMap, fmt::Display, num::ParseIntError};

pub(crate) struct HttpResponse {
    status_code: StatusCode,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl HttpResponse {
    pub(crate) fn new(
        status_code: StatusCode,
        headers: HashMap<String, String>,
        body: Option<String>,
    ) -> Self {
        Self {
            status_code,
            headers,
            body,
        }
    }

    fn generate_status_line(&self, response: &mut String) {
        response.push_str(&format!(
            "HTTP/1.1 {} {}\r\n",
            self.status_code.to_u16(),
            self.status_code.to_text()
        ))
    }

    fn generate_headers(&self, response: &mut String, body_len: usize) {
        self.headers
            .iter()
            .for_each(|h| response.push_str(&format!("{}: {}\r\n", h.0, h.1)));
        response.push_str(&format!("Content-Length: {}\r\n", body_len));
        response.push_str("\r\n");
    }

    fn generate_body(&self, response: &mut String) {
        if let Some(v) = &self.body {
            response.push_str(v);
        }
    }
}

impl Display for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut response: String = String::new();
        let body_len = match &self.body {
            Some(b) => b.len(),
            None => 0,
        };
        self.generate_status_line(&mut response);
        self.generate_headers(&mut response, body_len);
        self.generate_body(&mut response);
        write!(f, "{}", response)
    }
}

impl From<HttpError> for HttpResponse {
    fn from(err: HttpError) -> Self {
        match err {
            HttpError::BadRequest(msg) => ErrorHandler::bad_request(&msg),
            HttpError::MethodNotFound(msg) => ErrorHandler::method_not_found(&msg),
            HttpError::ParamNotFound(msg) => ErrorHandler::bad_request(&msg),
            HttpError::InternalServerError(msg) => ErrorHandler::internal_server_error(&msg),
            HttpError::Timeout => ErrorHandler::timeout(&err.to_string()),
            HttpError::InvalidEncoding => ErrorHandler::bad_request(&err.to_string()),
            HttpError::MalformedRequest => ErrorHandler::bad_request(&err.to_string()),
            HttpError::IoError => ErrorHandler::internal_server_error(&err.to_string()),
        }
    }
}

impl From<serde_json::Error> for HttpResponse {
    fn from(err: serde_json::Error) -> Self {
        ErrorHandler::bad_request(&err.to_string())
    }
}

impl From<ParseIntError> for HttpResponse {
    fn from(err: ParseIntError) -> Self {
        ErrorHandler::bad_request(&err.to_string())
    }
}
