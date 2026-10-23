use doorsys_protocol::UserAction;
use std::{future::Future, pin::Pin};

pub type PublishFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

/// Success means the command was accepted by the transport, not applied by a device.
pub trait CredentialPublisher: Send + Sync {
    fn publish(&self, command: UserAction) -> PublishFuture<'_>;
}
