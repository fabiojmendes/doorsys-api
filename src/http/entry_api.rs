use super::HttpResult;
use crate::domain::entry_log::{EntryLogDisplay, EntryLogRepository};
use poem::web::Data;
use poem_openapi::{
    param::Query,
    payload::Json,
    OpenApi,
};
use chrono::{DateTime, Utc};

pub struct EntryLogApi;

#[OpenApi]
impl EntryLogApi {
    #[oai(path = "/entry_logs", method = "get")]
    pub async fn list(
        &self,
        Data(entry_log_repo): Data<&EntryLogRepository>,
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
        let entry_list = entry_log_repo
            .fetch_all(date_range, device_id.0, customer_id.0)
            .await?;
        Ok(Json(entry_list))
    }
}