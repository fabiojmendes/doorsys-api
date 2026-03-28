use super::HttpResult;
use crate::domain::customer::{Customer, CustomerRepository, CustomerService, NewCustomer};
use poem_openapi::{
    param::{Path, Query},
    payload::Json,
    OpenApi,
};

pub struct CustomerApi {
    pub customer_repo: CustomerRepository,
    pub customer_service: CustomerService,
}

#[OpenApi]
impl CustomerApi {
    #[oai(path = "/customers", method = "post")]
    pub async fn create(
        &self,
        Json(customer_form): Json<NewCustomer>,
    ) -> HttpResult<Json<Customer>> {
        let customer = self.customer_repo.create(&customer_form).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers/:id", method = "put")]
    pub async fn update(
        &self,
        Path(id): Path<i64>,
        Json(new_customer): Json<NewCustomer>,
    ) -> HttpResult<Json<Customer>> {
        let customer = self.customer_repo.update(id, &new_customer).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers/:id/status", method = "put")]
    pub async fn update_status(
        &self,
        Path(id): Path<i64>,
        Json(active): Json<bool>,
    ) -> HttpResult<Json<Customer>> {
        let customer = self.customer_service.update_status(id, active).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers/:id", method = "get")]
    pub async fn get(&self, Path(id): Path<i64>) -> HttpResult<Json<Customer>> {
        let customer = self.customer_repo.fetch_one(id).await?;
        Ok(Json(customer))
    }

    #[oai(path = "/customers", method = "get")]
    pub async fn list(&self, active: Query<Option<bool>>) -> HttpResult<Json<Vec<Customer>>> {
        let customers = self.customer_repo.fetch_all(active.0).await?;
        Ok(Json(customers))
    }
}
