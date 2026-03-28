use crate::domain::staff::StaffService;
use crate::error::{DomainError, DomainResult};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Serialize, Object)]
#[serde(rename_all = "camelCase")]
#[oai(rename_all = "camelCase")]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub active: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Object)]
#[serde(rename_all = "camelCase")]
#[oai(rename_all = "camelCase")]
pub struct NewCustomer {
    pub name: String,
    pub email: String,
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct CustomerRepository {
    pub pool: PgPool,
}

impl CustomerRepository {
    pub async fn fetch_one(&self, id: i64) -> DomainResult<Customer> {
        sqlx::query_as!(Customer, r#"select * from customer where id = $1"#, id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    DomainError::NotFound(format!("Customer not found with id: {}", id))
                }
                _ => DomainError::from(e),
            })
    }

    pub async fn fetch_all(&self, active: Option<bool>) -> DomainResult<Vec<Customer>> {
        sqlx::query_as!(
            Customer,
            r#"select * from customer where (active = $1 or $1 is null) order by name"#,
            active
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DomainError::from)
    }

    pub async fn update(&self, id: i64, new_customer: &NewCustomer) -> DomainResult<Customer> {
        sqlx::query_as!(
            Customer,
            r#"update customer set name = $1, email = $2, notes = $3 where id = $4 returning *"#,
            new_customer.name,
            new_customer.email,
            new_customer.notes,
            id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                DomainError::NotFound(format!("Customer not found with id: {}", id))
            }
            _ => DomainError::from(e),
        })
    }

    pub async fn update_status(
        &self,
        executor: impl sqlx::PgExecutor<'_>,
        id: i64,
        active: bool,
    ) -> DomainResult<Customer> {
        sqlx::query_as!(
            Customer,
            r#"update customer set active = $1 where id = $2 returning *"#,
            active,
            id,
        )
        .fetch_one(executor)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                DomainError::NotFound(format!("Customer not found with id: {}", id))
            }
            _ => DomainError::from(e),
        })
    }

    pub async fn create(&self, new_customer: &NewCustomer) -> DomainResult<Customer> {
        sqlx::query_as!(
            Customer,
            r#"insert into customer (name, email, notes) values ($1, $2, $3) returning *"#,
            new_customer.name,
            new_customer.email,
            new_customer.notes,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DomainError::from)
    }
}

#[derive(Clone)]
pub struct CustomerService {
    pub customer_repo: CustomerRepository,
    pub staff_service: StaffService,
}

impl CustomerService {
    pub async fn update_status(&self, id: i64, active: bool) -> DomainResult<Customer> {
        // 1. Start Transaction
        let mut tx = self.customer_repo.pool.begin().await?;

        // 2. Perform DB operations with the transaction
        let customer = self
            .customer_repo
            .update_status(&mut *tx, id, active)
            .await?;
        let staff_list = self
            .staff_service
            .staff_repo
            .bulk_update_status(&mut *tx, id, active)
            .await?;

        // 3. Commit
        tx.commit().await?;

        // 4. Side Effects (only after success)
        for staff in staff_list {
            self.staff_service.send_mqtt_message(&staff).await?;
        }

        Ok(customer)
    }
}
