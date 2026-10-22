use anyhow::Context;
use std::{env, net::SocketAddr};

pub struct Config {
    pub database_url: String,
    pub mqtt_url: String,
    pub http_addr: SocketAddr,
}
impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            mqtt_url: env::var("MQTT_URL").context("MQTT_URL is required")?,
            http_addr: match env::var("HTTP_ADDR") {
                Ok(value) => value
                    .parse()
                    .context("HTTP_ADDR must be an IP address and port")?,
                Err(env::VarError::NotPresent) => "0.0.0.0:3000".parse()?,
                Err(error) => return Err(error.into()),
            },
        })
    }
}
