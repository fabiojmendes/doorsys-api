use super::{CredentialPublisher, NewStaff, Staff, repository};
use crate::error::{AppError, Result, unique_constraint};
use doorsys_protocol::UserAction;
use rand::RngExt;
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Clone)]
pub struct StaffService {
    pool: PgPool,
    publisher: Arc<dyn CredentialPublisher>,
    // Keep commit/publication order consistent within this process, including bulk sync.
    mutations: Arc<Mutex<()>>,
}

impl StaffService {
    pub fn new(pool: PgPool, publisher: Arc<dyn CredentialPublisher>) -> Self {
        Self {
            pool,
            publisher,
            mutations: Arc::new(Mutex::new(())),
        }
    }

    pub async fn get(&self, id: i64) -> Result<Staff> {
        Ok(repository::fetch_one(&self.pool, id).await?)
    }
    pub async fn list(&self, customer_id: i64) -> Result<Vec<Staff>> {
        Ok(repository::fetch_all(&self.pool, customer_id).await?)
    }

    pub async fn create(&self, input: &NewStaff) -> Result<Staff> {
        self.create_with_pin(input, generate_pin).await
    }

    async fn create_with_pin(
        &self,
        input: &NewStaff,
        mut pin: impl FnMut() -> i32 + Send,
    ) -> Result<Staff> {
        let _guard = self.lock_mutations().await;
        for _ in 0..5 {
            let mut tx = self.pool.begin().await?;
            match repository::create(&mut *tx, input, pin()).await {
                Ok(staff) => {
                    tx.commit().await?;
                    self.publish_committed(status_commands(&staff)).await?;
                    return Ok(staff);
                }
                Err(error) if unique_constraint(&error, "staff_pin_key") => {
                    tx.rollback().await?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(AppError::Conflict(
            "Could not allocate a unique PIN; retry the request",
        ))
    }

    pub async fn update(&self, id: i64, input: &NewStaff) -> Result<Staff> {
        let _guard = self.lock_mutations().await;
        let mut tx = self.pool.begin().await?;
        let old = repository::lock(&mut *tx, id).await?;
        ensure_mutable(&old)?;
        let staff = repository::update(&mut *tx, id, input).await?;
        let commands = if staff.active {
            credential_change(old.fob, staff.fob).into_iter().collect()
        } else {
            Vec::new()
        };
        tx.commit().await?;
        self.publish_committed(commands).await?;
        Ok(staff)
    }

    pub async fn update_pin(&self, id: i64) -> Result<Staff> {
        self.update_pin_with(id, generate_pin).await
    }

    async fn update_pin_with(&self, id: i64, mut pin: impl FnMut() -> i32 + Send) -> Result<Staff> {
        let _guard = self.lock_mutations().await;
        for _ in 0..5 {
            let mut tx = self.pool.begin().await?;
            let old = repository::lock(&mut *tx, id).await?;
            ensure_mutable(&old)?;
            let new_pin = pin();
            if new_pin == old.pin {
                tx.rollback().await?;
                continue;
            }
            match repository::update_pin(&mut *tx, id, new_pin).await {
                Ok(staff) => {
                    tx.commit().await?;
                    if staff.active {
                        self.publish_committed(vec![UserAction::Replace {
                            old: old.pin,
                            new: staff.pin,
                        }])
                        .await?;
                    }
                    return Ok(staff);
                }
                Err(error) if unique_constraint(&error, "staff_pin_key") => {
                    tx.rollback().await?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(AppError::Conflict(
            "Could not allocate a unique PIN; retry the request",
        ))
    }

    pub async fn update_status(&self, id: i64, active: bool) -> Result<Staff> {
        self.set_status(id, Some(active)).await
    }
    pub async fn delete(&self, id: i64) -> Result<Staff> {
        self.set_status(id, None).await
    }

    async fn set_status(&self, id: i64, active: Option<bool>) -> Result<Staff> {
        let _guard = self.lock_mutations().await;
        let mut tx = self.pool.begin().await?;
        ensure_mutable(&repository::lock(&mut *tx, id).await?)?;
        let staff = match active {
            Some(active) => repository::update_status(&mut *tx, id, active).await?,
            None => repository::delete(&mut *tx, id).await?,
        };
        tx.commit().await?;
        self.publish_committed(status_commands(&staff)).await?;
        Ok(staff)
    }

    pub async fn bulk_load_codes(&self) -> Result<()> {
        let _guard = self.lock_mutations().await;
        let codes = repository::fetch_all_codes(&self.pool)
            .await?
            .into_iter()
            .flatten()
            .collect();
        self.publisher
            .publish(UserAction::Bulk(codes))
            .await
            .map_err(AppError::PublishFailed)
    }

    pub(crate) async fn lock_mutations(&self) -> MutexGuard<'_, ()> {
        self.mutations.lock().await
    }

    pub(crate) async fn set_customer_status_in(
        &self,
        connection: &mut PgConnection,
        customer_id: i64,
        active: bool,
    ) -> Result<Vec<UserAction>> {
        let staff = repository::bulk_update_status(connection, customer_id, active).await?;
        Ok(staff.iter().flat_map(status_commands).collect())
    }

    pub(crate) async fn publish_committed(&self, commands: Vec<UserAction>) -> Result<()> {
        for command in commands {
            self.publisher
                .publish(command)
                .await
                .map_err(AppError::SyncFailed)?;
        }
        Ok(())
    }
}

fn generate_pin() -> i32 {
    rand::rng().random_range(100000..=999999)
}
fn ensure_mutable(staff: &Staff) -> Result<()> {
    if staff.deleted.is_some() {
        Err(AppError::Conflict("Deleted staff cannot be changed"))
    } else {
        Ok(())
    }
}
fn status_commands(staff: &Staff) -> Vec<UserAction> {
    std::iter::once(staff.pin)
        .chain(staff.fob)
        .map(|code| {
            if staff.active && staff.deleted.is_none() {
                UserAction::Add(code)
            } else {
                UserAction::Del(code)
            }
        })
        .collect()
}
fn credential_change(old: Option<i32>, new: Option<i32>) -> Option<UserAction> {
    match (old, new) {
        (Some(old), Some(new)) if old != new => Some(UserAction::Replace { old, new }),
        (None, Some(code)) => Some(UserAction::Add(code)),
        (Some(code), None) => Some(UserAction::Del(code)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credential_transitions() {
        assert_eq!(credential_change(None, Some(2)), Some(UserAction::Add(2)));
        assert_eq!(credential_change(Some(1), None), Some(UserAction::Del(1)));
        assert_eq!(
            credential_change(Some(1), Some(2)),
            Some(UserAction::Replace { old: 1, new: 2 })
        );
        assert_eq!(credential_change(Some(1), Some(1)), None);
        assert_eq!(credential_change(None, None), None);
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod database_tests;
