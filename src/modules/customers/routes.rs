use crate::http::HttpResult;
use crate::modules::customers::{Customer, CustomerService, NewCustomer};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    active: Option<bool>,
}

pub async fn create(
    State(service): State<CustomerService>,
    Json(customer_form): Json<NewCustomer>,
) -> HttpResult<Json<Customer>> {
    let customer = service.create(&customer_form).await?;
    Ok(Json(customer))
}

pub async fn update(
    State(service): State<CustomerService>,
    Path(id): Path<i64>,
    Json(new_customer): Json<NewCustomer>,
) -> HttpResult<Json<Customer>> {
    let customer = service.update(id, &new_customer).await?;
    Ok(Json(customer))
}

pub async fn update_status(
    State(service): State<crate::application::CustomerStatusService>,
    Path(id): Path<i64>,
    Json(active): Json<bool>,
) -> HttpResult<Json<Customer>> {
    let customer = service.update(id, active).await?;
    Ok(Json(customer))
}

pub async fn get_customer(
    State(service): State<CustomerService>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Customer>> {
    let customer = service.get(id).await?;
    Ok(Json(customer))
}

pub async fn list(
    State(service): State<CustomerService>,
    Query(filter): Query<Filter>,
) -> HttpResult<Json<Vec<Customer>>> {
    let customers = service.list(filter.active).await?;
    Ok(Json(customers))
}

pub(crate) fn router() -> aide::axum::ApiRouter<crate::state::AppState> {
    use aide::axum::{
        ApiRouter,
        routing::{get, put},
    };
    ApiRouter::new()
        .api_route("/customers", get(list).post(create))
        .api_route("/customers/{id}", get(get_customer).put(update))
        .api_route("/customers/{id}/status", put(update_status))
}
