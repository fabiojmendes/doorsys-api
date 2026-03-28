use super::HttpResult;
use crate::domain::staff::{NewStaff, Staff, StaffRepository, StaffService};
use doorsys_protocol::UserAction;
use poem_openapi::{param::Path, payload::Json, OpenApi};
use rumqttc::{AsyncClient, QoS};

pub struct StaffApi {
    pub staff_repo: StaffRepository,
    pub staff_service: StaffService,
    pub mqtt_client: AsyncClient,
}

#[OpenApi]
impl StaffApi {
    #[oai(path = "/staff", method = "post")]
    pub async fn create(&self, Json(new_staff): Json<NewStaff>) -> HttpResult<Json<Staff>> {
        let staff = self.staff_service.create(&new_staff).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id", method = "get")]
    pub async fn get(&self, Path(id): Path<i64>) -> HttpResult<Json<Staff>> {
        let staff = self.staff_repo.fetch_one(id).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/customers/:customer_id/staff", method = "get")]
    pub async fn list(&self, Path(customer_id): Path<i64>) -> HttpResult<Json<Vec<Staff>>> {
        let staff_list = self.staff_repo.fetch_all(customer_id).await?;
        Ok(Json(staff_list))
    }

    #[oai(path = "/staff/:id", method = "put")]
    pub async fn update(
        &self,
        Path(id): Path<i64>,
        Json(update_staff): Json<NewStaff>,
    ) -> HttpResult<Json<Staff>> {
        let staff = self.staff_service.update(id, &update_staff).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id/pin", method = "post")]
    pub async fn update_pin(&self, Path(id): Path<i64>) -> HttpResult<Json<Staff>> {
        let staff = self.staff_service.update_pin(id).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id/status", method = "put")]
    pub async fn update_status(
        &self,
        Path(id): Path<i64>,
        Json(active): Json<bool>,
    ) -> HttpResult<Json<Staff>> {
        let staff = self.staff_service.update_status(id, active).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id", method = "delete")]
    pub async fn delete(&self, Path(id): Path<i64>) -> HttpResult<Json<Staff>> {
        let staff = self.staff_service.delete(id).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/admin/bulk", method = "post")]
    pub async fn bulk_load_codes(&self) -> HttpResult<()> {
        let codes = self.staff_repo.fetch_all_codes().await?;
        tracing::info!("Executing bulk load of {} codes", codes.len());
        let bulk_action = UserAction::Bulk(codes.into_iter().flatten().collect());
        let payload = postcard::to_allocvec(&bulk_action)?;
        self.mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }
}
