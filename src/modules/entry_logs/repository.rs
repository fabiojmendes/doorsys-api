use super::models::*;
use chrono::{DateTime, Utc};
use std::ops::Range;

pub(super) async fn create_with_code(
    executor: impl sqlx::PgExecutor<'_>,
    code: i32,
    code_type: &str,
    net_id: Option<&str>,
    success: bool,
    event_date: &DateTime<Utc>,
) -> Result<EntryLog, sqlx::Error> {
    sqlx::query_as!(
        EntryLog,
        r#"
            with temp(code, net_id) as (values($1::int, $3::varchar))
            insert into entry_log (staff_id, code, code_type, device_id, success, event_date) 
                select s.id, t.code, $2, d.id, $4, $5
                from temp t
                left join staff s on s.pin = t.code or s.fob = t.code
                left join device d on d.net_id = t.net_id
            returning *
            "#,
        code,
        code_type,
        net_id,
        success,
        event_date
    )
    .fetch_one(executor)
    .await
}

pub(super) async fn fetch_all(
    executor: impl sqlx::PgExecutor<'_>,
    date_range: Range<DateTime<Utc>>,
    device_id: Option<i64>,
    customer_id: Option<i64>,
) -> Result<Vec<EntryLogDisplay>, sqlx::Error> {
    sqlx::query_as!(
        EntryLogDisplay,
        r#"
            select 
                e.id, 
                s.id as "staff_id?", 
                s.name as "staff_name?", 
                s.deleted as "staff_deleted?",
                c.id as "customer_id?",
                c.name as "customer_name?",
                d.id as "device_id?",
                d.name as "device_name?",
                e.code,
                e.code_type,
                e.success,
                e.event_date
            from entry_log e
            left join staff s on s.id = e.staff_id
            left join customer c on s.customer_id = c.id
            left join device d on d.id = e.device_id
            where e.event_date between $1 and $2
            and (d.id = $3 or $3 is null)
            and (c.id = $4 or $4 is null)
            order by e.event_date desc
            "#,
        date_range.start,
        date_range.end,
        device_id,
        customer_id,
    )
    .fetch_all(executor)
    .await
}
