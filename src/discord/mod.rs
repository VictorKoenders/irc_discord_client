use crate::config::Config;
use parking_lot::Mutex;
use serenity::prelude::{Client, Context, EventHandler};
use std::sync::Arc;

pub enum Message {}

pub struct Handler {
    // irc_handler_sender: Arc<Sender<Message>>,
}

impl EventHandler for Handler {
    fn message(&self, _ctx: Context, new_message: serenity::model::channel::Message) {
        dbg!(new_message);
    }
}

pub fn run(config: Arc<Mutex<Config>>) {
    let config = config.lock();
    let mut client = Client::new(&config.discord_bot_token, Handler {})
        .expect("Could not create discord client");

    client.start();
}
