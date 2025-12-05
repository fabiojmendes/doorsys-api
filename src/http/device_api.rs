use super::HttpResult;
use crate::domain::device::{Device, DeviceRepository};
use poem_openapi::{payload::Json, OpenApi};

pub struct DeviceApi {
    pub device_repo: DeviceRepository,
}

#[OpenApi]
impl DeviceApi {
    #[oai(path = "/devices", method = "get")]
    pub async fn list(&self) -> HttpResult<Json<Vec<Device>>> {
        let device_list = self.device_repo.fetch_all().await?;
        Ok(Json(device_list))
    }
}

