use super::HttpResult;
use crate::domain::{
    customer::{Customer, CustomerRepository, NewCustomer},
    staff::StaffService,
};
use poem::web::Data;
use poem_openapi::{
    param::{Path, Query},
    payload::Json,
    OpenApi,
};

pub struct CustomerApi;

#[OpenApi]
impl CustomerApi {
    #[oai(path = "/customers", method = "post")]
    pub async fn create(
        &self,
        Data(customer_repo): Data<&CustomerRepository>,
        Json(customer_form): Json<NewCustomer>,
    ) -> HttpResult<Json<Customer>> {
        let customer = customer_repo.create(&customer_form).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers/:id", method = "put")]
    pub async fn update(
        &self,
        Data(customer_repo): Data<&CustomerRepository>,
        Path(id): Path<i64>,
        Json(new_customer): Json<NewCustomer>,
    ) -> HttpResult<Json<Customer>> {
        let customer = customer_repo.update(id, &new_customer).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers/:id/status", method = "put")]
    pub async fn update_status(
        &self,
        Data(customer_repo): Data<&CustomerRepository>,
        Data(staff_service): Data<&StaffService>,
        Path(id): Path<i64>,
        Json(active): Json<bool>,
    ) -> HttpResult<Json<Customer>> {
        let customer = customer_repo.update_status(id, active).await?;
        staff_service.bulk_update_status(id, active).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers/:id", method = "get")]
    pub async fn get(
        &self,
        Data(customer_repo): Data<&CustomerRepository>,
        Path(id): Path<i64>,
    ) -> HttpResult<Json<Customer>> {
        let customer = customer_repo.fetch_one(id).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers", method = "get")]
    pub async fn list(
        &self,
        Data(customer_repo): Data<&CustomerRepository>,
        active: Query<Option<bool>>,
    ) -> HttpResult<Json<Vec<Customer>>> {
        let customers = customer_repo.fetch_all(active.0).await?;
        Ok(Json(customers))
    }
}
