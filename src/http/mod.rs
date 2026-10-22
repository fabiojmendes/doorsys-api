use crate::{
    db::HealthService,
    modules::{customers, devices, entry_logs, staff},
    state::AppState,
};
use aide::{
    axum::{ApiRouter, IntoApiResponse, routing::get},
    openapi::{Info, OpenApi},
};
use axum::{Extension, Json, Router, extract::State};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
mod error;
pub use error::{HttpError, HttpResult};

/// Build the HTTP adapter without binding a socket or starting background tasks.
pub fn router(state: AppState) -> Router {
    let mut api = OpenApi {
        info: Info {
            title: "Doorsys API".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Info::default()
        },
        ..OpenApi::default()
    };
    ApiRouter::new()
        .api_route("/", get(health))
        .api_route("/openapi", get(serve_api))
        .merge(customers::routes::router())
        .merge(staff::routes::router())
        .merge(devices::routes::router())
        .merge(entry_logs::routes::router())
        .finish_api(&mut api)
        .layer(TraceLayer::new_for_http())
        .layer(Extension(Arc::new(api)))
        .with_state(state)
}
async fn health(State(service): State<HealthService>) -> HttpResult<Json<serde_json::Value>> {
    service.check().await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
async fn serve_api(Extension(api): Extension<Arc<OpenApi>>) -> impl IntoApiResponse {
    Json(api)
}
