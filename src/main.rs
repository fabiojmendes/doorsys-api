use anyhow::Context;
use doorsys_api::{built_info, config::Config, db, http, logging, mqtt, state::AppState};
use rumqttc::{AsyncClient, MqttOptions};
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::watch};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init();
    tracing::info!(
        version = built_info::GIT_VERSION.unwrap_or(built_info::PKG_VERSION),
        target = built_info::TARGET,
        "Starting Doorsys API"
    );
    let config = Config::load()?;
    let pool = db::connect(&config.database_url).await?;
    let listener = TcpListener::bind(config.http_addr).await?;
    let (client, event_loop) = AsyncClient::new(MqttOptions::parse_url(&config.mqtt_url)?, 10);
    let state = AppState::new(
        pool.clone(),
        Arc::new(mqtt::MqttPublisher::new(client.clone())),
    );
    let (http_stop, mut http_shutdown) = watch::channel(false);
    let (mqtt_stop, mqtt_shutdown) = watch::channel(false);
    let mut mqtt_task = tokio::spawn(mqtt::run(
        client,
        event_loop,
        state.entry_logs.clone(),
        mqtt_shutdown,
    ));
    let mut http_task = tokio::spawn(async move {
        axum::serve(listener, http::router(state))
            .with_graceful_shutdown(async move {
                let _ = http_shutdown.wait_for(|stop| *stop).await;
            })
            .await
    });
    let mut http_done = false;
    let mut mqtt_done = false;
    let outcome = tokio::select! {
        signal = shutdown_signal() => signal,
        result = &mut mqtt_task => {
            mqtt_done = true;
            match result {
                Ok(Err(error)) => Err(error),
                Err(error) => Err(error.into()),
                Ok(Ok(())) => Err(anyhow::anyhow!("MQTT runtime exited unexpectedly")),
            }
        }
        result = &mut http_task => {
            http_done = true;
            result.context("HTTP task failed").and_then(|result| result.context("HTTP server failed"))
        }
    };
    http_stop.send_replace(true);
    let drained = tokio::time::timeout(Duration::from_secs(30), async {
        if !http_done {
            (&mut http_task).await.context("HTTP task failed")??;
        }
        mqtt_stop.send_replace(true);
        if !mqtt_done {
            (&mut mqtt_task).await.context("MQTT task failed")??;
        }
        pool.close().await;
        anyhow::Ok(())
    })
    .await;
    // Dropping MQTT's JoinSet on cancellation also aborts its child workers.
    http_task.abort();
    mqtt_task.abort();
    match drained {
        Ok(result) => result?,
        Err(_) => {
            tracing::error!(
                "Shutdown deadline exceeded; unfinished requests or audit work were cancelled"
            );
            anyhow::bail!("Shutdown exceeded 30 seconds");
        }
    }
    outcome
}

async fn shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(unix)]
    let terminate = terminate.recv();
    #[cfg(not(unix))]
    let terminate = std::future::pending::<Option<()>>();
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = terminate => {},
    }
    Ok(())
}
