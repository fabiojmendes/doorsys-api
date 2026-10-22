use super::models::*;

pub(super) async fn create(
    executor: impl sqlx::PgExecutor<'_>,
    new_staff: &NewStaff,
    pin: i32,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(
            Staff,
            r#"insert into staff (customer_id, name, phone, pin, fob) values ($1, $2, $3, $4, $5) returning *"#,
            new_staff.customer_id,
            new_staff.name,
            new_staff.phone,
            pin,
            new_staff.fob,
        )
        .fetch_one(executor)
        .await
}

pub(super) async fn update(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
    update_staff: &NewStaff,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(
        Staff,
        r#"update staff set name = $1, phone = $2, fob = $3 where id = $4 returning *"#,
        update_staff.name,
        update_staff.phone,
        update_staff.fob,
        id,
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn update_pin(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
    new_pin: i32,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(
        Staff,
        r#"update staff set pin = $1 where id = $2 returning *"#,
        new_pin,
        id,
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn update_status(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
    active: bool,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(
        Staff,
        r#"update staff set active = $1 where id = $2 returning *"#,
        active,
        id,
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn delete(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(
        Staff,
        r#"update staff set active = false, deleted = now() where id = $1 returning *"#,
        id,
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn bulk_update_status(
    executor: impl sqlx::PgExecutor<'_>,
    customer_id: i64,
    active: bool,
) -> Result<Vec<Staff>, sqlx::Error> {
    sqlx::query_as!(
        Staff,
        r#"update staff set active = $1 where customer_id = $2 and deleted is null returning *"#,
        active,
        customer_id,
    )
    .fetch_all(executor)
    .await
}

pub(super) async fn fetch_all(
    executor: impl sqlx::PgExecutor<'_>,
    customer_id: i64,
) -> Result<Vec<Staff>, sqlx::Error> {
    sqlx::query_as!(
        Staff,
        r#"select * from staff where customer_id = $1 and deleted is null order by name"#,
        customer_id
    )
    .fetch_all(executor)
    .await
}

pub(super) async fn fetch_one(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(Staff, r#"select * from staff where id = $1"#, id)
        .fetch_one(executor)
        .await
}

pub(super) async fn fetch_all_codes(
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Vec<Option<i32>>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
            with all_codes(code, active) as (
                select pin, active from staff where deleted is null 
                union 
                select fob, active from staff where deleted is null
            ) select code from all_codes where code is not null and active is true order by code
            "#,
    )
    .fetch_all(executor)
    .await
}

pub(super) async fn lock(
    executor: impl sqlx::PgExecutor<'_>,
    id: i64,
) -> Result<Staff, sqlx::Error> {
    sqlx::query_as!(Staff, "select * from staff where id = $1 for update", id)
        .fetch_one(executor)
        .await
}
