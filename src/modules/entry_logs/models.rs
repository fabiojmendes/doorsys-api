use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryLog {
    pub id: i64,
    pub staff_id: Option<i64>,
    pub code: i32,
    pub code_type: String,
    pub device_id: Option<i64>,
    pub success: bool,
    pub event_date: DateTime<Utc>,
    pub created: DateTime<Utc>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntryLogDisplay {
    pub id: i64,
    pub staff_id: Option<i64>,
    pub staff_name: Option<String>,
    pub staff_deleted: Option<DateTime<Utc>>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub device_id: Option<i64>,
    pub device_name: Option<String>,
    pub code: i32,
    pub code_type: String,
    pub success: bool,
    pub event_date: DateTime<Utc>,
}

/// Transport-independent audit input.
#[derive(Debug, Clone)]
pub struct RecordEntry {
    pub code: i32,
    pub code_type: String,
    pub net_id: String,
    pub success: bool,
    pub event_date: DateTime<Utc>,
}
