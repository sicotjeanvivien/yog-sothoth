use std::future::Future;
use std::pin::Pin;

use crate::interface::{HttpMethod, HttpRequest, HttpResponse};

pub(crate) type HandlerResult = Result<HttpResponse, HttpResponse>;
pub(crate) type Handler = dyn Fn(HttpRequest) -> Pin<Box<dyn Future<Output = HandlerResult> + Send>>
    + Send
    + Sync
    + 'static;
pub(crate) struct Route {
    pub(crate) method: HttpMethod,
    pub(crate) path: String,
    pub(crate) handler: Box<Handler>,
}

impl Route {
    pub(crate) fn new(method: HttpMethod, path: String, handler: Box<Handler>) -> Self {
        Self {
            method,
            path,
            handler,
        }
    }
}
