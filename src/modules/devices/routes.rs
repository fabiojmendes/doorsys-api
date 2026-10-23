use crate::http::HttpResult;
use crate::modules::devices::{Device, DeviceService};
use axum::{Json, extract::State};

pub async fn list(State(service): State<DeviceService>) -> HttpResult<Json<Vec<Device>>> {
    let device_list = service.list().await?;
    Ok(Json(device_list))
}

pub(crate) fn router() -> aide::axum::ApiRouter<crate::state::AppState> {
    aide::axum::ApiRouter::new().api_route("/devices", aide::axum::routing::get(list))
}
