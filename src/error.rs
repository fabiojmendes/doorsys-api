use std::{error::Error, fmt};

/// Application failures. HTTP status codes and response bodies belong to the HTTP adapter.
#[derive(Debug)]
pub enum AppError {
    NotFound,
    DuplicateEntryLog,
    Conflict(&'static str),
    InvalidInput(&'static str),
    Internal(anyhow::Error),
    /// The transaction committed, but one or more device commands could not be queued.
    SyncFailed(anyhow::Error),
    PublishFailed(anyhow::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("Record not found"),
            Self::DuplicateEntryLog => f.write_str("Entry log has already been recorded"),
            Self::Conflict(message) | Self::InvalidInput(message) => f.write_str(message),
            Self::Internal(_) => f.write_str("Internal server error"),
            Self::SyncFailed(_) => f.write_str(
                "Changes were saved, but device synchronization failed; reconcile with /admin/bulk",
            ),
            Self::PublishFailed(_) => {
                f.write_str("Device synchronization failed; retry /admin/bulk")
            }
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Internal(error) | Self::SyncFailed(error) | Self::PublishFailed(error) => {
                Some(error.as_ref())
            }
            _ => None,
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        match &error {
            sqlx::Error::RowNotFound => Self::NotFound,
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                tracing::warn!(error = ?error, "Database uniqueness constraint rejected an operation");
                Self::Conflict(match db.constraint() {
                    Some("staff_fob_key") => "Fob is already in use by another staff member",
                    Some("staff_pin_key") => "PIN is already in use by another staff member",
                    Some("unique_staff_name_customer") => {
                        "A staff member with this name already exists for this customer"
                    }
                    Some("unique_customer_name") => "A customer with this name already exists",
                    // Renaming mac_addr to net_id did not rename the database constraint.
                    Some("device_mac_addr_key") => "A device with this network ID already exists",
                    _ => "Record conflicts with an existing record",
                })
            }
            sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                tracing::warn!(error = ?error, "Database reference constraint rejected an operation");
                Self::InvalidInput(match db.constraint() {
                    Some("staff_customer_id_fkey") => "Customer does not exist",
                    Some("entry_log_staff_id_fkey") => "Staff member does not exist",
                    Some("entry_log_device_id_fkey") => "Device does not exist",
                    _ => "Referenced record does not exist",
                })
            }
            sqlx::Error::Database(db) if db.is_check_violation() => {
                tracing::warn!(error = ?error, "Database validation constraint rejected an operation");
                Self::InvalidInput("Invalid record values")
            }
            _ => Self::Internal(error.into()),
        }
    }
}

pub(crate) fn unique_constraint(error: &sqlx::Error, name: &str) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.is_unique_violation() && db.constraint() == Some(name))
}
