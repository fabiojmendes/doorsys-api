use super::HttpResult;
use crate::domain::device::{Device, DeviceRepository};
use poem::{handler, web::{Data, Json}};

#[handler]
pub async fn list(Data(device_repo): Data<&DeviceRepository>) -> HttpResult<Json<Vec<Device>>> {
    let device_list = device_repo.fetch_all().await?;
    Ok(Json(device_list))
}