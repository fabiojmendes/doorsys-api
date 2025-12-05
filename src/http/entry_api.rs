use super::HttpResult;
use crate::domain::entry_log::{EntryLogDisplay, EntryLogRepository};
use chrono::{DateTime, Utc};
use poem_openapi::{param::Query, payload::Json, OpenApi};

pub struct EntryLogApi {
    pub entry_log_repo: EntryLogRepository,
}

#[OpenApi]
impl EntryLogApi {
    #[oai(path = "/entry_logs", method = "get")]
    pub async fn list(
        &self,
        #[oai(name = "startDate")] start_date: Query<DateTime<Utc>>,
        #[oai(name = "endDate")] end_date: Query<DateTime<Utc>>,
        #[oai(name = "deviceId")] device_id: Query<Option<i64>>,
        #[oai(name = "customerId")] customer_id: Query<Option<i64>>,
    ) -> HttpResult<Json<Vec<EntryLogDisplay>>> {
        let date_range = start_date.0..end_date.0;
        tracing::debug!(
            "Getting entry_logs for start_date: {:?}, end_date: {:?}, device_id: {:?}, customer_id: {:?}",
            start_date.0,
            end_date.0,
            device_id.0,
            customer_id.0
        );
        let entry_list = self
            .entry_log_repo
            .fetch_all(date_range, device_id.0, customer_id.0)
            .await?;
        Ok(Json(entry_list))
    }
}

