use super::models::*;

pub(super) async fn fetch_one(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(Customer, r#"select * from customer where id = $1"#, id,)
        .fetch_one(executor)
        .await
}

pub(super) async fn fetch_all(
    executor: impl sqlx::PgExecutor<'_>,
    active: Option<bool>,
) -> Result<Vec<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"select * from customer where (active = $1 or $1 is null) order by name"#,
        active
    )
    .fetch_all(executor)
    .await
}

pub(super) async fn update(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
    new_customer: &NewCustomer,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"update customer set name = $1, email = $2, notes = $3 where id = $4 returning *"#,
        new_customer.name,
        new_customer.email,
        new_customer.notes,
        id,
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn update_status(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
    active: bool,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"update customer set active = $1 where id = $2 returning *"#,
        active,
        id,
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn create(
    executor: impl sqlx::PgExecutor<'_>,
    new_customer: &NewCustomer,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"insert into customer (name, email, notes) values ($1, $2, $3) returning *"#,
        new_customer.name,
        new_customer.email,
        new_customer.notes,
    )
    .fetch_one(executor)
    .await
}
