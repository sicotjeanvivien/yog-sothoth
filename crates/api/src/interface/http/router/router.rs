use std::collections::HashMap;

use crate::interface::{ErrorHandler, Handler, HttpMethod, HttpRequest, HttpResponse, Route};

pub(crate) struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub(crate) fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }

    pub(crate) fn add_route(mut self, route: Route) -> Self {
        self.routes.push(route);
        self
    }

    pub(crate) fn find_handler(
        &self,
        method: &HttpMethod,
        path: &str,
    ) -> Option<(&Handler, HashMap<String, String>)> {
        for route in self.routes.iter() {
            if route.method != *method {
                continue;
            }

            let path_segments: Vec<&str> = path.split('/').collect();
            let route_segments: Vec<&str> = route.path.split('/').collect();

            if path_segments.len() != route_segments.len() {
                continue;
            }

            let mut params = HashMap::new();
            let mut matched = true;

            for (route_seg, path_seg) in route_segments.iter().zip(path_segments.iter()) {
                if route_seg.starts_with(':') {
                    params.insert(route_seg[1..].to_string(), path_seg.to_string());
                } else if route_seg != path_seg {
                    matched = false;
                    break;
                }
            }

            if matched {
                return Some((&route.handler, params));
            }
        }
        None
    }

    pub(crate) async fn handler(&self, mut request: HttpRequest) -> HttpResponse {
        match self.find_handler(&request.method, &request.path) {
            Some((handler, params)) => {
                request.params.extend(params);
                handler(request).await.unwrap_or_else(|err| err)
            }
            None => ErrorHandler::not_found("Page Not Found."),
        }
    }
}
