use super::{EntryLogDisplay, RecordEntry, repository};
use crate::error::{AppError, Result, unique_constraint};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::ops::Range;

#[derive(Clone)]
pub struct EntryLogService {
    pool: PgPool,
}
impl EntryLogService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn list(
        &self,
        range: Range<DateTime<Utc>>,
        device_id: Option<i64>,
        customer_id: Option<i64>,
    ) -> Result<Vec<EntryLogDisplay>> {
        if range.start > range.end {
            return Err(AppError::InvalidInput(
                "startDate must not be after endDate",
            ));
        }
        Ok(repository::fetch_all(&self.pool, range, device_id, customer_id).await?)
    }
    /// Returns `AppError::DuplicateEntryLog` when this event was already recorded.
    pub async fn record(&self, input: &RecordEntry) -> Result<()> {
        match repository::create_with_code(
            &self.pool,
            input.code,
            &input.code_type,
            Some(&input.net_id),
            input.success,
            &input.event_date,
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(error) if unique_constraint(&error, "event_code_date") => {
                Err(AppError::DuplicateEntryLog)
            }
            Err(error) => Err(error.into()),
        }
    }
}
