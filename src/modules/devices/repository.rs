use super::models::*;

pub(super) async fn fetch_all(
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Vec<Device>, sqlx::Error> {
    sqlx::query_as!(Device, r#"select * from device order by name"#)
        .fetch_all(executor)
        .await
}
