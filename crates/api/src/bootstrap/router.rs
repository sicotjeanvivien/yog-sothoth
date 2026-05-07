use std::sync::Arc;

use crate::bootstrap::AppState;
use crate::interface::http::handlers::main_handler::MainHandler;
use crate::interface::{Handler, HandlerResult, HttpMethod, HttpRequest, Route, Router};
use crate::routes;

/// Build the application router from the app_state.
///
/// Each route handler captures only the dependencies it actually needs —
/// `MainHandler` is stateless, future handlers (`PoolHandler`, etc.) will
/// hold an `Arc<dyn SomeRepository>` cloned from the app_state.
pub(crate) async fn build_router(app_state: Arc<AppState>) -> Arc<Router> {
    let main_handler = Arc::new(MainHandler::new());

    // Future:
    // let pool_handler = Arc::new(PoolHandler::new(app_state.pool_repository.clone()));

    let _ = app_state; // silence unused warning until handlers consume it

    let router = routes![
        GET "/" => {
            let h = main_handler.clone();
            route_handler(move |req| {
                let h = h.clone();
                async move { h.index(req).await }
            })
        },
        // Future:
        // GET "/api/pools" => {
        //     let h = pool_handler.clone();
        //     route_handler(move |req| {
        //         let h = h.clone();
        //         async move { h.get_all(req).await }
        //     })
        // },
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
