mod domain;
mod http;
mod logging;
mod mqtt;

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
    let mqtt_client = mqtt::start(pool.clone(), &mqtt_url).await?;

    http::serve(pool, mqtt_client).await
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
