use super::HttpResult;
use crate::domain::staff::{NewStaff, Staff, StaffRepository, StaffService};
use doorsys_protocol::UserAction;
use poem::web::Data;
use poem_openapi::{param::Path, payload::Json, OpenApi};
use rand::Rng;
use rumqttc::{AsyncClient, QoS};

fn generate_pin() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(100000..=999999)
}

pub struct StaffApi;

#[OpenApi]
impl StaffApi {
    #[oai(path = "/staff", method = "post")]
    pub async fn create(
        &self,
        Data(staff_repo): Data<&StaffRepository>,
        Data(mqtt_client): Data<&AsyncClient>,
        Json(new_staff): Json<NewStaff>,
    ) -> HttpResult<Json<Staff>> {
        let pin = generate_pin();
        let staff = staff_repo.create(&new_staff, pin).await?;

        let user_add = UserAction::Add(pin);
        let payload = postcard::to_allocvec(&user_add)?;
        mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;

        if let Some(fob) = staff.fob {
            let user_add = UserAction::Add(fob);
            let payload = postcard::to_allocvec(&user_add)?;
            mqtt_client
                .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
                .await?;
        }
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id", method = "get")]
    pub async fn get(
        &self,
        Data(staff_repo): Data<&StaffRepository>,
        Path(id): Path<i64>,
    ) -> HttpResult<Json<Staff>> {
        let staff = staff_repo.fetch_one(id).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/customers/:customer_id/staff", method = "get")]
    pub async fn list(
        &self,
        Data(staff_repo): Data<&StaffRepository>,
        Path(customer_id): Path<i64>,
    ) -> HttpResult<Json<Vec<Staff>>> {
        let staff_list = staff_repo.fetch_all(customer_id).await?;
        Ok(Json(staff_list))
    }

    #[oai(path = "/staff/:id", method = "put")]
    pub async fn update(
        &self,
        Data(staff_repo): Data<&StaffRepository>,
        Data(mqtt_client): Data<&AsyncClient>,
        Path(id): Path<i64>,
        Json(update_staff): Json<NewStaff>,
    ) -> HttpResult<Json<Staff>> {
        let old_staff = staff_repo.fetch_one(id).await?;
        let staff = staff_repo.update(id, &update_staff).await?;

        if let Some(action) = match (old_staff.fob, staff.fob) {
            (Some(old), Some(new)) if old != new => Some(UserAction::Replace { old, new }),
            (None, Some(fob)) => Some(UserAction::Add(fob)),
            (Some(fob), None) => Some(UserAction::Del(fob)),
            _ => None,
        } {
            let payload = postcard::to_allocvec(&action)?;
            mqtt_client
                .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
                .await?;
        }
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id/pin", method = "post")]
    pub async fn update_pin(
        &self,
        Data(staff_repo): Data<&StaffRepository>,
        Data(mqtt_client): Data<&AsyncClient>,
        Path(id): Path<i64>,
    ) -> HttpResult<Json<Staff>> {
        let old_staff = staff_repo.fetch_one(id).await?;
        let old_pin = old_staff.pin;
        let new_pin = generate_pin();
        let staff = staff_repo.update_pin(id, new_pin).await?;

        let replace_pin = UserAction::Replace {
            old: old_pin,
            new: new_pin,
        };
        let payload = postcard::to_allocvec(&replace_pin)?;
        mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id/status", method = "put")]
    pub async fn update_status(
        &self,
        Data(staff_service): Data<&StaffService>,
        Path(id): Path<i64>,
        Json(active): Json<bool>,
    ) -> HttpResult<Json<Staff>> {
        let staff = staff_service.update_status(id, active).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/staff/:id", method = "delete")]
    pub async fn delete(
        &self,
        Data(staff_service): Data<&StaffService>,
        Path(id): Path<i64>,
    ) -> HttpResult<Json<Staff>> {
        let staff = staff_service.delete(id).await?;
        Ok(Json(staff))
    }

    #[oai(path = "/admin/bulk", method = "post")]
    pub async fn bulk_load_codes(
        &self,
        Data(staff_repo): Data<&StaffRepository>,
        Data(mqtt_client): Data<&AsyncClient>,
    ) -> HttpResult<()> {
        let codes = staff_repo.fetch_all_codes().await?;
        tracing::info!("Executing bulk load of {} codes", codes.len());
        let bulk_action = UserAction::Bulk(codes.into_iter().flatten().collect());
        let payload = postcard::to_allocvec(&bulk_action)?;
        mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }
}
