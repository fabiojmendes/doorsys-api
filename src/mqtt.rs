use std::time::Duration;

use doorsys_protocol::Audit;
use rumqttc::{AsyncClient, Event, EventLoop, Packet, Publish, QoS};
use tokio::{task, time};

use crate::domain::entry_log::EntryLogRepository;

/// Spwan async task to handle incomming mqtt messages
pub async fn handle_messages(
    entry_repo: EntryLogRepository,
    client: AsyncClient,
    mut event_loop: EventLoop,
) {
    task::spawn(async move {
        loop {
            match event_loop.poll().await {
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    tracing::info!(
                        "topic: {}, qos: {:?}, size: {:?}",
                        p.topic,
                        p.qos,
                        p.payload.len()
                    );

                    // Only one type of message is handled right now so
                    // no point in match on the topic

                    handle_audit_message(&entry_repo, p).await;
                }
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    tracing::info!("Connected to mqtt broker and subscribing to topics");
                    if let Err(e) = client.subscribe("doorsys/audit/+", QoS::AtLeastOnce).await {
                        tracing::error!("Error subscribing to topic {}", e);
                    }
                }
                Err(rumqttc::ConnectionError::Io(e)) => {
                    tracing::error!("Connection error {:?}", e);
                    time::sleep(Duration::from_secs(5)).await;
                }
                Err(rumqttc::ConnectionError::ConnectionRefused(e)) => {
                    tracing::error!("Connection refused {:?}", e);
                    time::sleep(Duration::from_secs(5)).await;
                }
                notification => {
                    tracing::trace!("Notification = {:?}", &notification);
                }
            }
        }
    });
}

async fn handle_audit_message(entry_repo: &EntryLogRepository, msg: Publish) {
    match postcard::from_bytes::<Audit>(&msg.payload) {
        Ok(audit) => {
            let net_id = msg.topic.split('/').nth(2);
            tracing::info!(
                "Audit({}) [{:?}]: {:?}",
                msg.payload.len(),
                net_id.unwrap_or(""),
                audit
            );
            match entry_repo
                .create_with_code(
                    audit.code,
                    &audit.code_type.to_string(),
                    net_id,
                    audit.success,
                    &audit.timestamp.into(),
                )
                .await
            {
                Ok(log) => {
                    tracing::info!("Log created {:?}", log);
                }
                Err(sqlx::Error::Database(e)) => {
                    if let Some(c) = e.constraint() {
                        tracing::warn!("Duplicated entry log, skpping... {}", c);
                    } else {
                        tracing::error!("Database error creating entry log {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Error creating entry log {}", e);
                }
            }
        }
        Err(e) => {
            tracing::error!("Error decoding message: {}", e);
        }
    }
}
