use crate::modules::staff::{CredentialPublisher, PublishFuture};
use doorsys_protocol::UserAction;
use rumqttc::{AsyncClient, QoS};

#[derive(Clone)]
pub struct MqttPublisher {
    client: AsyncClient,
}
impl MqttPublisher {
    pub fn new(client: AsyncClient) -> Self {
        Self { client }
    }
}
impl CredentialPublisher for MqttPublisher {
    fn publish(&self, command: UserAction) -> PublishFuture<'_> {
        Box::pin(async move {
            self.client
                .publish(
                    "doorsys/user",
                    QoS::AtLeastOnce,
                    false,
                    postcard::to_allocvec(&command)?,
                )
                .await?;
            Ok(())
        })
    }
}
