use super::messages::decode_audit;
use crate::error::AppError;
use crate::modules::entry_logs::{EntryLogService, RecordEntry};
use anyhow::{Context, bail};
use rumqttc::{AsyncClient, Event, EventLoop, Outgoing, Packet, QoS};
use std::{future::Future, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};

const AUDIT_QUEUE_CAPACITY: usize = 100;

/// Supervises polling and persistence. Shutdown closes intake, then drains accepted audits.
/// The owner must bound shutdown and cancel this future if the deadline expires.
pub async fn run(
    client: AsyncClient,
    event_loop: EventLoop,
    service: EntryLogService,
    mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let (sender, receiver) = mpsc::channel(AUDIT_QUEUE_CAPACITY);
    let (intake_stop, intake_stopped) = watch::channel(false);
    let mut tasks = JoinSet::new();
    let disconnect_client = client.clone();
    tasks.spawn(async move {
        poll(client, event_loop, sender, intake_stopped)
            .await
            .context("MQTT polling stopped")
    });
    tasks.spawn(async move {
        consume(receiver, |entry| {
            let service = service.clone();
            async move {
                match service.record(&entry).await {
                    Ok(()) => {}
                    Err(error @ AppError::DuplicateEntryLog) => tracing::warn!(
                        %error,
                        device = %entry.net_id,
                        event_date = %entry.event_date,
                        "Duplicate audit ignored"
                    ),
                    Err(error) => tracing::error!(?error, "Failed to record audit"),
                }
            }
        })
        .await;
        Ok(())
    });
    tokio::select! {
        result = tasks.join_next() => {
            result.context("MQTT tasks missing")??.context("MQTT task failed")?;
            bail!("MQTT task exited unexpectedly");
        }
        _ = shutdown.wait_for(|stop| *stop) => {}
    }
    intake_stop.send_replace(true);
    // Polling stays alive while the consumer drains; only the consumer exits normally.
    tasks
        .join_next()
        .await
        .context("MQTT tasks missing")??
        .context("MQTT shutdown failed")?;
    // Queue DISCONNECT behind commands accepted during HTTP draining, and keep
    // polling until it has been written. The owner enforces the shutdown deadline.
    disconnect_client.disconnect().await?;
    tasks
        .join_next()
        .await
        .context("MQTT polling task missing")??
        .context("MQTT disconnect failed")?;
    Ok(())
}

async fn poll(
    client: AsyncClient,
    mut event_loop: EventLoop,
    sender: mpsc::Sender<RecordEntry>,
    mut stop: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let mut sender = Some(sender);
    loop {
        tokio::select! {
            biased;
            _ = async { let _ = stop.wait_for(|stop| *stop).await; }, if sender.is_some() => { sender.take(); }
            event = event_loop.poll() => match event {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    // Never await a send to the request queue from its own polling task.
                    client.try_subscribe("doorsys/audit/+", QoS::AtLeastOnce).context("Could not queue MQTT subscription")?;
                }
                Ok(Event::Incoming(Packet::Publish(message))) => {
                    if let Some(queue) = &sender {
                        match decode_audit(&message.topic, &message.payload) {
                            Ok(entry) => {
                                // Bounded backpressure: no unbounded per-message tasks or silent drops.
                                tokio::select! {
                                    result = queue.send(entry) => { result.context("Audit consumer stopped")?; }
                                    _ = async { let _ = stop.wait_for(|stop| *stop).await; } => { sender.take(); }
                                }
                            }
                            Err(error) => tracing::warn!(?error, topic = %message.topic, "Rejected audit"),
                        }
                    }
                }
                Ok(Event::Outgoing(Outgoing::Disconnect)) => return Ok(()),
                Ok(_) => {},
                Err(error) => {
                    tracing::warn!(?error, "MQTT connection failed; retrying in five seconds");
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                        _ = async { let _ = stop.wait_for(|stop| *stop).await; }, if sender.is_some() => { sender.take(); }
                    }
                }
            }
        }
    }
}

async fn consume<F, Fut>(mut receiver: mpsc::Receiver<RecordEntry>, mut record: F)
where
    F: FnMut(RecordEntry) -> Fut,
    Fut: Future<Output = ()>,
{
    while let Some(entry) = receiver.recv().await {
        record(entry).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[tokio::test]
    async fn bounded_queue_applies_backpressure_and_drains_on_close() {
        let (sender, receiver) = mpsc::channel(AUDIT_QUEUE_CAPACITY);
        let entry = RecordEntry {
            code: 1,
            code_type: "pin".into(),
            net_id: "door".into(),
            success: true,
            event_date: chrono::Utc::now(),
        };
        for _ in 0..AUDIT_QUEUE_CAPACITY {
            sender.try_send(entry.clone()).unwrap();
        }
        assert!(matches!(
            sender.try_send(entry),
            Err(mpsc::error::TrySendError::Full(_))
        ));
        drop(sender);
        let recorded = Arc::new(Mutex::new(0));
        consume(receiver, |_| {
            let recorded = recorded.clone();
            async move {
                *recorded.lock().unwrap() += 1;
            }
        })
        .await;
        assert_eq!(*recorded.lock().unwrap(), AUDIT_QUEUE_CAPACITY);
    }
}
