use crate::{
    error::Result,
    modules::{
        customers::{Customer, CustomerService},
        staff::StaffService,
    },
};
use sqlx::PgPool;

/// Coordinates a cross-domain use case without exposing either repository.
#[derive(Clone)]
pub struct CustomerStatusService {
    pool: PgPool,
    customers: CustomerService,
    staff: StaffService,
}
impl CustomerStatusService {
    pub fn new(pool: PgPool, customers: CustomerService, staff: StaffService) -> Self {
        Self {
            pool,
            customers,
            staff,
        }
    }
    pub async fn update(&self, id: i64, active: bool) -> Result<Customer> {
        let _guard = self.staff.lock_mutations().await;
        let mut tx = self.pool.begin().await?;
        // UPDATE locks the customer first; the staff UPDATE then locks the affected rows.
        let customer = self.customers.set_status_in(&mut tx, id, active).await?;
        let commands = self
            .staff
            .set_customer_status_in(&mut tx, id, active)
            .await?;
        tx.commit().await?;
        self.staff.publish_committed(commands).await?;
        Ok(customer)
    }
}
