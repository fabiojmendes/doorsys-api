use super::HttpResult;
use crate::domain::staff::{NewStaff, Staff, StaffRepository, StaffService};
use doorsys_protocol::UserAction;
use poem::{
    handler,
    web::{Data, Json, Path},
};
use rand::Rng;
use rumqttc::{AsyncClient, QoS};

fn generate_pin() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(100000..=999999)
}

#[handler]
pub async fn create(
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

#[handler]
pub async fn get(
    Data(staff_repo): Data<&StaffRepository>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Staff>> {
    let staff = staff_repo.fetch_one(id).await?;
    Ok(Json(staff))
}

#[handler]
pub async fn list(
    Data(staff_repo): Data<&StaffRepository>,
    Path(customer_id): Path<i64>,
) -> HttpResult<Json<Vec<Staff>>> {
    let staff_list = staff_repo.fetch_all(customer_id).await?;
    Ok(Json(staff_list))
}

#[handler]
pub async fn update(
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

#[handler]
pub async fn update_pin(
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

#[handler]
pub async fn update_status(
    Data(staff_service): Data<&StaffService>,
    Path(id): Path<i64>,
    Json(active): Json<bool>,
) -> HttpResult<Json<Staff>> {
    let staff = staff_service.update_status(id, active).await?;
    Ok(Json(staff))
}

#[handler]
pub async fn delete(
    Data(staff_service): Data<&StaffService>,
    Path(id): Path<i64>,
) -> HttpResult<Json<Staff>> {
    let staff = staff_service.delete(id).await?;
    Ok(Json(staff))
}

#[handler]
pub async fn bulk_load_codes(
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

