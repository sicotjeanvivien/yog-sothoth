use crate::bootstrap::Container;
use crate::interface::http::handlers::main_handler::MainHandler;
use crate::interface::{Handler, HandlerResult, HttpMethod, HttpRequest, Route, Router};
use crate::routes;
use std::sync::Arc;

pub(crate) async fn build_router(container: &Container) -> Arc<Router> {
    let main_handler = Arc::new(MainHandler::new());
    let router = routes![
      GET "/" => {
        let h = main_handler.clone();
        route_handler(move |req| {
          let h = h.clone();
          async move { h.index(req).await }
        })
      },
    ];

    Arc::new(router)
}

fn route_handler<F, Fut>(f: F) -> Box<Handler>
where
    F: Fn(HttpRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = HandlerResult> + Send + 'static,
{
    Box::new(move |req| Box::pin(f(req)))
}
