use crate::{
    application::CustomerStatusService,
    db::HealthService,
    modules::{
        customers::CustomerService,
        devices::DeviceService,
        entry_logs::EntryLogService,
        staff::{CredentialPublisher, StaffService},
    },
};
use axum::extract::FromRef;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub customers: CustomerService,
    pub staff: StaffService,
    pub devices: DeviceService,
    pub entry_logs: EntryLogService,
    pub customer_status: CustomerStatusService,
    pub health: HealthService,
}
impl AppState {
    pub fn new(pool: PgPool, publisher: Arc<dyn CredentialPublisher>) -> Self {
        let customers = CustomerService::new(pool.clone());
        let staff = StaffService::new(pool.clone(), publisher);
        Self {
            customer_status: CustomerStatusService::new(
                pool.clone(),
                customers.clone(),
                staff.clone(),
            ),
            customers,
            staff,
            devices: DeviceService::new(pool.clone()),
            entry_logs: EntryLogService::new(pool.clone()),
            health: HealthService::new(pool),
        }
    }
}
