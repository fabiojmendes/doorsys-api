use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Staff {
    pub id: i64,
    pub customer_id: i64,
    pub name: String,
    pub phone: String,
    pub pin: i32,
    pub fob: Option<i32>,
    pub active: bool,
    pub created: DateTime<Utc>,
    pub deleted: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewStaff {
    pub customer_id: i64,
    pub name: String,
    pub phone: String,
    pub fob: Option<i32>,
}
