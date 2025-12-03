use crate::{
    built_info,
    domain::{
        customer::CustomerRepository,
        device::DeviceRepository,
        entry_log::EntryLogRepository,
        staff::{StaffRepository, StaffService},
    },
};
use anyhow::Context;
use customer_api::CustomerApi;
use device_api::DeviceApi;
use entry_api::EntryLogApi;
use poem::{listener::TcpListener, middleware::Tracing, web::Data, EndpointExt, Route, Server};
use poem_openapi::{payload::Json as OpenApiJson, OpenApi, OpenApiService};
use rumqttc::AsyncClient;
use serde_json::json;
use sqlx::PgPool;
use staff_api::StaffApi;
use tokio::signal::{self, unix::SignalKind};

pub mod customer_api;
pub mod device_api;
pub mod entry_api;
pub mod error;
pub mod staff_api;

pub type HttpResult<T> = core::result::Result<T, error::ApiError>;

struct HealthApi;

#[OpenApi]
impl HealthApi {
    #[oai(path = "/", method = "get")]
    async fn health(
        &self,
        Data(pool): Data<&PgPool>,
    ) -> HttpResult<OpenApiJson<serde_json::Value>> {
        sqlx::query("select 1").execute(pool).await?;
        Ok(OpenApiJson(json!({"ok": true})))
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

    let api_service = OpenApiService::new(
        (
            // API Modules
            HealthApi,
            CustomerApi,
            StaffApi,
            DeviceApi,
            EntryLogApi,
        ),
        built_info::PKG_NAME,
        built_info::PKG_VERSION,
    )
    .server("http://localhost:3000");

    let ui = api_service.swagger_ui();

    let app = Route::new()
        .nest("/", api_service)
        .nest("/docs", ui)
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

