use super::HttpResult;
use crate::domain::device::{Device, DeviceRepository};
use poem::web::Data;
use poem_openapi::{payload::Json, OpenApi};

pub struct DeviceApi;

#[OpenApi]
impl DeviceApi {
    #[oai(path = "/devices", method = "get")]
    pub async fn list(&self, Data(device_repo): Data<&DeviceRepository>) -> HttpResult<Json<Vec<Device>>> {
        let device_list = device_repo.fetch_all().await?;
        Ok(Json(device_list))
    }
}