pub mod application;
pub mod config;
pub mod db;
pub mod error;
pub mod http;
pub mod logging;
pub mod modules;
pub mod mqtt;
pub mod state;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
