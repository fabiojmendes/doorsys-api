use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect(url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().min_connections(5).connect(url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

#[derive(Clone)]
pub struct HealthService {
    pool: PgPool,
}
impl HealthService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn check(&self) -> crate::error::Result<()> {
        sqlx::query("select 1").execute(&self.pool).await?;
        Ok(())
    }
}
