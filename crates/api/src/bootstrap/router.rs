use std::sync::Arc;

use crate::bootstrap::AppState;
use crate::interface::http::handlers::{main_handler::MainHandler, pool_handler::PoolHandler};
use crate::interface::{Handler, HandlerResult, HttpMethod, HttpRequest, Route, Router};
use crate::routes;

pub(crate) async fn build_router(state: Arc<AppState>) -> Arc<Router> {
    let main_handler = Arc::new(MainHandler::new());
    let pool_handler = Arc::new(PoolHandler::new(state.pool_repository.clone()));

    let router = routes![
        GET "/" => {
            let h = main_handler.clone();
            route_handler(move |req| {
                let h = h.clone();
                async move { h.index(req).await }
            })
        },
        GET "/api/pools" => {
            let h = pool_handler.clone();
            route_handler(move |req| {
                let h = h.clone();
                async move { h.list(req).await }
            })
        },
    ];

    Arc::new(router)
}

fn route_handler<F, Fut>(f: F) -> Box<Handler>
where
    F: Fn(HttpRequest) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = HandlerResult> + Send + 'static,
{
    Box::new(move |req| Box::pin(f(req)))
}
