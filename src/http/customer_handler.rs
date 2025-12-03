use super::HttpResult;
use crate::domain::{
    customer::{Customer, CustomerRepository, NewCustomer},
    staff::StaffService,
};
use poem::{
    handler,
    web::{Data, Json, Path, Query},
};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    active: Option<bool>,
}

#[handler]
pub async fn create(
    Data(customer_repo): Data<&CustomerRepository>,
    Json(customer_form): Json<NewCustomer>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.create(&customer_form).await?;
    Ok(Json(customer))
}

#[handler]
pub async fn update(
    Data(customer_repo): Data<&CustomerRepository>,
    Path(id): Path<i64>,
    Json(new_customer): Json<NewCustomer>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.update(id, &new_customer).await?;
    Ok(Json(customer))
}

#[handler]
pub async fn update_status(
    Data(customer_repo): Data<&CustomerRepository>,
    Data(staff_service): Data<&StaffService>,
    Path(id): Path<i64>,
    Json(active): Json<bool>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.update_status(id, active).await?;
    staff_service.bulk_update_status(id, active).await?;
    Ok(Json(customer))
}

#[handler]
pub async fn get(
    Data(customer_repo): Data<&CustomerRepository>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Customer>> {
    let customer = customer_repo.fetch_one(id).await?;
    Ok(Json(customer))
}

#[handler]
pub async fn list(
    Data(customer_repo): Data<&CustomerRepository>,
    Query(filter): Query<Filter>,
) -> HttpResult<Json<Vec<Customer>>> {
    let customers = customer_repo.fetch_all(filter.active).await?;
    Ok(Json(customers))
}