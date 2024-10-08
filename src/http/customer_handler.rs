use super::HttpResult;
use crate::domain::{
    customer::{Customer, CustomerRepository, NewCustomer},
    staff::StaffService,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    active: Option<bool>,
}

pub async fn create(
    State(customer_repo): State<CustomerRepository>,
    Json(customer_form): Json<NewCustomer>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.create(&customer_form).await?;
    Ok(Json(customer))
}

pub async fn update(
    State(customer_repo): State<CustomerRepository>,
    Path(id): Path<i64>,
    Json(new_customer): Json<NewCustomer>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.update(id, &new_customer).await?;
    Ok(Json(customer))
}

pub async fn update_status(
    State(customer_repo): State<CustomerRepository>,
    State(staff_service): State<StaffService>,
    Path(id): Path<i64>,
    Json(active): Json<bool>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.update_status(id, active).await?;
    staff_service.bulk_update_status(id, active).await?;
    Ok(Json(customer))
}

pub async fn get(
    State(customer_repo): State<CustomerRepository>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.fetch_one(id).await?;
    Ok(Json(customer))
}

pub async fn list(
    State(customer_repo): State<CustomerRepository>,
    Query(filter): Query<Filter>,
) -> HttpResult<Json<Vec<Customer>>> {
    let customers = customer_repo.fetch_all(filter.active).await?;
    Ok(Json(customers))
}
