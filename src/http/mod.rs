use crate::domain::{
    customer::CustomerRepository,
    device::DeviceRepository,
    entry_log::EntryLogRepository,
    staff::{StaffRepository, StaffService},
};
use anyhow::Context;
use poem::{
    error::ResponseError,
    get, handler,
    http::StatusCode,
    listener::TcpListener,
    middleware::Tracing,
    post, put,
    web::{Data, Json},
    Body, EndpointExt, IntoResponse, Response, Route, Server,
};
use rumqttc::AsyncClient;
use serde_json::json;
use sqlx::PgPool;
use tokio::signal::{self, unix::SignalKind};

pub mod customer_handler;
pub mod device_handler;
pub mod entry_handler;
pub mod staff_handler;

pub type HttpResult<T> = core::result::Result<T, AppError>;

#[derive(Debug)]
pub struct AppError(anyhow::Error);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl ResponseError for AppError {
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    fn as_response(&self) -> Response {
        let payload = json!({
            "code": 500,
            "success": false,
            "msg": format!("{}", self.0)
        });
        tracing::error!("request error: {:?}", self);
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from_json(payload).unwrap())
            .into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self(err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self(err.into())
    }
}

impl From<postcard::Error> for AppError {
    fn from(err: postcard::Error) -> Self {
        Self(err.into())
    }
}

impl From<rumqttc::ClientError> for AppError {
    fn from(err: rumqttc::ClientError) -> Self {
        Self(err.into())
    }
}

pub async fn serve(pool: PgPool, mqtt_client: AsyncClient) -> anyhow::Result<()> {
    let customer_repo = CustomerRepository { pool: pool.clone() };
    let staff_repo = StaffRepository { pool: pool.clone() };
    let entry_log_repo = EntryLogRepository { pool: pool.clone() };
    let device_repo = DeviceRepository { pool: pool.clone() };
    let staff_service = StaffService {
        staff_repo: staff_repo.clone(),
        mqtt_client: mqtt_client.clone(),
    };

    let app = Route::new()
        .at("/", get(health))
        .at(
            "/customers",
            get(customer_handler::list).post(customer_handler::create),
        )
        .at(
            "/customers/:id",
            get(customer_handler::get).put(customer_handler::update),
        )
        .at(
            "/customers/:id/status",
            put(customer_handler::update_status),
        )
        .at("/customers/:id/staff", get(staff_handler::list))
        .at("/staff", post(staff_handler::create))
        .at(
            "/staff/:id",
            get(staff_handler::get)
                .put(staff_handler::update)
                .delete(staff_handler::delete),
        )
        .at("/staff/:id/pin", post(staff_handler::update_pin))
        .at("/staff/:id/status", put(staff_handler::update_status))
        .at("/devices", get(device_handler::list))
        .at("/entry_logs", get(entry_handler::list))
        .at("/admin/bulk", post(staff_handler::bulk_load_codes))
        .with(Tracing)
        .data(pool)
        .data(mqtt_client)
        .data(customer_repo)
        .data(staff_repo)
        .data(entry_log_repo)
        .data(device_repo)
        .data(staff_service);

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run_with_graceful_shutdown(app, shutdown_signal(), None)
        .await
        .context("error running HTTP server")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[handler]
async fn health(Data(pool): Data<&PgPool>) -> HttpResult<Json<serde_json::Value>> {
    sqlx::query("select 1").execute(pool).await?;
    Ok(Json(json!({"ok": true})))
}

