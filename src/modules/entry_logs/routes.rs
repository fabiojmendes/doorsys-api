use crate::http::HttpResult;
use crate::modules::entry_logs::{EntryLogDisplay, EntryLogService};
use axum::{
    Json,
    extract::{Query, State},
};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    device_id: Option<i64>,
    customer_id: Option<i64>,
}

pub async fn list(
    State(service): State<EntryLogService>,
    filter: Query<Filter>,
) -> HttpResult<Json<Vec<EntryLogDisplay>>> {
    let date_range = filter.start_date..filter.end_date;
    tracing::debug!("Getting entry_logs for {:?}", filter);
    let entry_list = service
        .list(date_range, filter.device_id, filter.customer_id)
        .await?;
    Ok(Json(entry_list))
}

pub(crate) fn router() -> aide::axum::ApiRouter<crate::state::AppState> {
    aide::axum::ApiRouter::new().api_route("/entry_logs", aide::axum::routing::get(list))
}
