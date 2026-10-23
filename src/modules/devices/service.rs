use super::{Device, repository};
use crate::error::Result;
use sqlx::PgPool;

#[derive(Clone)]
pub struct DeviceService {
    pool: PgPool,
}
impl DeviceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn list(&self) -> Result<Vec<Device>> {
        Ok(repository::fetch_all(&self.pool).await?)
    }
}
