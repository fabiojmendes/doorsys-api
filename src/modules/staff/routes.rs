use super::{NewStaff, Staff, StaffService};
use crate::{http::HttpResult, state::AppState};
use aide::axum::{
    ApiRouter,
    routing::{get, post, put},
};
use axum::{
    Json,
    extract::{Path, State},
};

pub(crate) fn router() -> ApiRouter<AppState> {
    ApiRouter::new()
        .api_route("/customers/{id}/staff", get(list))
        .api_route("/staff", post(create))
        .api_route("/staff/{id}", get(get_staff).put(update).delete(delete))
        .api_route("/staff/{id}/pin", post(update_pin))
        .api_route("/staff/{id}/status", put(update_status))
        .api_route("/admin/bulk", post(bulk_load_codes))
}
async fn create(
    State(service): State<StaffService>,
    Json(input): Json<NewStaff>,
) -> HttpResult<Json<Staff>> {
    Ok(Json(service.create(&input).await?))
}
async fn get_staff(
    State(service): State<StaffService>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Staff>> {
    Ok(Json(service.get(id).await?))
}
async fn list(
    State(service): State<StaffService>,
    Path(customer_id): Path<i64>,
) -> HttpResult<Json<Vec<Staff>>> {
    Ok(Json(service.list(customer_id).await?))
}
async fn update(
    State(service): State<StaffService>,
    Path(id): Path<i64>,
    Json(input): Json<NewStaff>,
) -> HttpResult<Json<Staff>> {
    Ok(Json(service.update(id, &input).await?))
}
async fn update_pin(
    State(service): State<StaffService>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Staff>> {
    Ok(Json(service.update_pin(id).await?))
}
async fn update_status(
    State(service): State<StaffService>,
    Path(id): Path<i64>,
    Json(active): Json<bool>,
) -> HttpResult<Json<Staff>> {
    Ok(Json(service.update_status(id, active).await?))
}
async fn delete(
    State(service): State<StaffService>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Staff>> {
    Ok(Json(service.delete(id).await?))
}
async fn bulk_load_codes(State(service): State<StaffService>) -> HttpResult<()> {
    Ok(service.bulk_load_codes().await?)
}
