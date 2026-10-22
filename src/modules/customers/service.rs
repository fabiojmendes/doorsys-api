use super::{Customer, NewCustomer, repository};
use crate::error::Result;
use sqlx::{PgConnection, PgPool};

#[derive(Clone)]
pub struct CustomerService {
    pool: PgPool,
}

impl CustomerService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn get(&self, id: i64) -> Result<Customer> {
        Ok(repository::fetch_one(&self.pool, id).await?)
    }
    pub async fn list(&self, active: Option<bool>) -> Result<Vec<Customer>> {
        Ok(repository::fetch_all(&self.pool, active).await?)
    }
    pub async fn create(&self, input: &NewCustomer) -> Result<Customer> {
        Ok(repository::create(&self.pool, input).await?)
    }
    pub async fn update(&self, id: i64, input: &NewCustomer) -> Result<Customer> {
        Ok(repository::update(&self.pool, id, input).await?)
    }
    // Only the application coordinator can participate in a cross-domain transaction.
    pub(crate) async fn set_status_in(
        &self,
        connection: &mut PgConnection,
        id: i64,
        active: bool,
    ) -> Result<Customer> {
        Ok(repository::update_status(connection, id, active).await?)
    }
}
