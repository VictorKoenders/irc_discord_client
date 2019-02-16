use serde_derive::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub discord_bot_token: String,
    pub config_channel: u64,
    pub mapping: Vec<Mapping>,
}

impl Config {
    pub fn load() -> Config {
        serde_json::from_reader(
            std::fs::File::open(CONFIG_FILE_NAME).expect("Could not open config file"),
        )
        .expect("Could not read config file")
    }

    pub fn save(&self) {
        let mut file = std::fs::File::create(CONFIG_FILE_NAME).expect("Could not open config file");
        serde_json::to_writer_pretty(&mut file, &self).expect("Could not write config file");
    }
}

#[derive(Serialize, Deserialize)]
pub struct Mapping {
    pub irc_config: IrcConfig,
    pub discord_config: DiscordConfig,
    pub channel_map: Vec<ChannelMap>,
}

#[derive(Serialize, Deserialize)]
pub struct IrcConfig {
    pub host: String,
    pub port: u16,
    pub use_ssl: bool,
    pub nick: String,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordConfig {
    pub config_channel: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ChannelMap {
    pub irc_channel: String,
    pub discord_channel: u64,
}
