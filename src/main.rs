use parking_lot::Mutex;
use std::sync::Arc;

pub mod config;
pub mod discord;

fn main() {
    let config = Arc::new(Mutex::new(config::Config::load()));
    tokio::run(futures::future::lazy(move || {
        discord::run(config.clone());
        Ok(())
    }));
}
