mod domain;
mod http;
mod logging;
mod mqtt;

use domain::entry_log::EntryLogRepository;
use rumqttc::{AsyncClient, MqttOptions};
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init();
    tracing::info!("Starting Server");
    tracing::info!(
        "Doorsys api version {}, built for {} by {}.",
        built_info::PKG_VERSION,
        built_info::TARGET,
        built_info::RUSTC_VERSION
    );
    if let (Some(version), Some(hash), Some(dirty)) = (
        built_info::GIT_VERSION,
        built_info::GIT_COMMIT_HASH_SHORT,
        built_info::GIT_DIRTY,
    ) {
        tracing::info!("Git version: {version} ({hash})");
        if dirty {
            tracing::warn!("Repo was dirty");
        }
    }

    let pool = PgPoolOptions::new()
        .min_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;

    tracing::info!("Connected to database, executing migrations");
    sqlx::migrate!().run(&pool).await?;

    let mqtt_url = env::var("MQTT_URL")?;
    let mqtt_opts = MqttOptions::parse_url(mqtt_url)?;
    let entry_repo = EntryLogRepository { pool: pool.clone() };
    let (mqtt_client, event_loop) = AsyncClient::new(mqtt_opts, 10);
    mqtt::handle_messages(entry_repo, mqtt_client.clone(), event_loop).await;

    http::serve(pool, mqtt_client).await
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
