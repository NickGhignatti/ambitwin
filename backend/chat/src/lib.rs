use axum::Router;
use axum::routing::{get, post};

pub mod adapters;
pub mod domain;
pub mod service;
pub mod state;

use state::AppState;

pub fn build_router(state: AppState) -> Router {
    use adapters::driving::http_api::controllers::*;

    Router::new()
        .route("/health", get(adapters::metrics::health))
        .route("/metrics", get(adapters::metrics::metrics_handler))
        .route(
            "/conversations",
            post(create_conversation).get(list_conversations),
        )
        .route(
            "/conversations/{id}",
            get(get_conversation)
                .patch(rename_conversation)
                .delete(delete_conversation),
        )
        .route("/conversations/{id}/messages", post(send_message))
        .layer(axum::middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            rate_limit::rate_limit,
        ))
        .layer(axum::middleware::from_fn(adapters::metrics::track_metrics))
        .with_state(state)
}
