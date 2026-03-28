use crate::error::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use doorsys_protocol::UserAction;
use poem_openapi::Object;
use rand::RngExt;
use rumqttc::{AsyncClient, QoS};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

fn generate_pin() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(100000..=999999)
}

#[derive(Debug, Serialize, Object)]
#[serde(rename_all = "camelCase")]
#[oai(rename_all = "camelCase")]
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

#[derive(Debug, Deserialize, Object)]
#[serde(rename_all = "camelCase")]
#[oai(rename_all = "camelCase")]
pub struct NewStaff {
    pub customer_id: i64,
    pub name: String,
    pub phone: String,
    pub fob: Option<i32>,
}

#[derive(Clone)]
pub struct StaffRepository {
    pub pool: PgPool,
}

impl StaffRepository {
    pub async fn create(&self, new_staff: &NewStaff, pin: i32) -> DomainResult<Staff> {
        sqlx::query_as!(
            Staff,
            r#"insert into staff (customer_id, name, phone, pin, fob) values ($1, $2, $3, $4, $5) returning *"#,
            new_staff.customer_id,
            new_staff.name,
            new_staff.phone,
            pin,
            new_staff.fob,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DomainError::from)
    }

    pub async fn update(&self, id: i64, update_staff: &NewStaff) -> DomainResult<Staff> {
        sqlx::query_as!(
            Staff,
            r#"update staff set name = $1, phone = $2, fob = $3 where id = $4 returning *"#,
            update_staff.name,
            update_staff.phone,
            update_staff.fob,
            id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                DomainError::NotFound(format!("Staff not found with id: {}", id))
            }
            _ => DomainError::from(e),
        })
    }

    pub async fn update_pin(&self, id: i64, new_pin: i32) -> DomainResult<Staff> {
        sqlx::query_as!(
            Staff,
            r#"update staff set pin = $1 where id = $2 returning *"#,
            new_pin,
            id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                DomainError::NotFound(format!("Staff not found with id: {}", id))
            }
            _ => DomainError::from(e),
        })
    }

    pub async fn update_status(&self, id: i64, active: bool) -> DomainResult<Staff> {
        sqlx::query_as!(
            Staff,
            r#"update staff set active = $1 where id = $2 returning *"#,
            active,
            id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                DomainError::NotFound(format!("Staff not found with id: {}", id))
            }
            _ => DomainError::from(e),
        })
    }

    pub async fn delete(&self, id: i64) -> DomainResult<Staff> {
        sqlx::query_as!(
            Staff,
            r#"update staff set active = false, deleted = now() where id = $1 returning *"#,
            id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                DomainError::NotFound(format!("Staff not found with id: {}", id))
            }
            _ => DomainError::from(e),
        })
    }

    pub async fn bulk_update_status_with_conn(
        &self,
        executor: impl sqlx::PgExecutor<'_>,
        customer_id: i64,
        active: bool,
    ) -> DomainResult<Vec<Staff>> {
        sqlx::query_as!(
            Staff,
            r#"update staff set active = $1 where customer_id = $2 and deleted is null returning *"#,
            active,
            customer_id,
        )
        .fetch_all(executor)
        .await
        .map_err(DomainError::from)
    }

    pub async fn fetch_all(&self, customer_id: i64) -> DomainResult<Vec<Staff>> {
        sqlx::query_as!(
            Staff,
            r#"select * from staff where customer_id = $1 and deleted is null order by name"#,
            customer_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DomainError::from)
    }

    pub async fn fetch_one(&self, id: i64) -> DomainResult<Staff> {
        sqlx::query_as!(Staff, r#"select * from staff where id = $1"#, id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    DomainError::NotFound(format!("Staff not found with id: {}", id))
                }
                _ => DomainError::from(e),
            })
    }

    pub async fn fetch_all_codes(&self) -> DomainResult<Vec<Option<i32>>> {
        sqlx::query_scalar!(
            r#"
            with all_codes(code, active) as (
                select pin, active from staff 
                union 
                select fob, active from staff
            ) select code from all_codes where code is not null and active is true order by code
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DomainError::from)
    }
}

#[derive(Clone)]
pub struct StaffService {
    pub staff_repo: StaffRepository,
    pub mqtt_client: AsyncClient,
}

impl StaffService {
    pub async fn create(&self, new_staff: &NewStaff) -> DomainResult<Staff> {
        let pin = generate_pin();
        let staff = self.staff_repo.create(new_staff, pin).await?;

        let user_add = UserAction::Add(pin);
        let payload = postcard::to_allocvec(&user_add)?;
        self.mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;

        if let Some(fob) = staff.fob {
            let user_add = UserAction::Add(fob);
            let payload = postcard::to_allocvec(&user_add)?;
            self.mqtt_client
                .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
                .await?;
        }
        Ok(staff)
    }

    pub async fn update(&self, id: i64, update_staff: &NewStaff) -> DomainResult<Staff> {
        let old_staff = self.staff_repo.fetch_one(id).await?;
        let staff = self.staff_repo.update(id, update_staff).await?;

        if let Some(action) = match (old_staff.fob, staff.fob) {
            (Some(old), Some(new)) if old != new => Some(UserAction::Replace { old, new }),
            (None, Some(fob)) => Some(UserAction::Add(fob)),
            (Some(fob), None) => Some(UserAction::Del(fob)),
            _ => None,
        } {
            let payload = postcard::to_allocvec(&action)?;
            self.mqtt_client
                .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
                .await?;
        }
        Ok(staff)
    }

    pub async fn update_pin(&self, id: i64) -> DomainResult<Staff> {
        let old_staff = self.staff_repo.fetch_one(id).await?;
        let old_pin = old_staff.pin;
        let new_pin = generate_pin();
        let staff = self.staff_repo.update_pin(id, new_pin).await?;

        let replace_pin = UserAction::Replace {
            old: old_pin,
            new: new_pin,
        };
        let payload = postcard::to_allocvec(&replace_pin)?;
        self.mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(staff)
    }

    pub async fn update_status(&self, id: i64, active: bool) -> DomainResult<Staff> {
        let staff = self.staff_repo.update_status(id, active).await?;
        self.send_mqtt_message(&staff).await?;
        Ok(staff)
    }

    pub async fn send_mqtt_message(&self, staff: &Staff) -> DomainResult<()> {
        let pin_action = match staff.active {
            true => UserAction::Add(staff.pin),
            false => UserAction::Del(staff.pin),
        };

        let payload = postcard::to_allocvec(&pin_action)?;
        self.mqtt_client
            .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
            .await?;

        if let Some(fob) = staff.fob {
            let fob_action = match staff.active {
                true => UserAction::Add(fob),
                false => UserAction::Del(fob),
            };
            let payload = postcard::to_allocvec(&fob_action)?;
            self.mqtt_client
                .publish("doorsys/user", QoS::AtLeastOnce, false, payload)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn delete(&self, id: i64) -> DomainResult<Staff> {
        let staff = self.staff_repo.delete(id).await?;
        self.send_mqtt_message(&staff).await?;
        Ok(staff)
    }
}
