#[macro_use]
extern crate serde_derive;

fn main() {
    dotenv::dotenv().expect("Could not load .env file");
    std::env::set_var("RUST_BACKTRACE", "1");
    let discord_token = std::env::var("DISCORD_TOKEN").expect("Could not get DISCORD_TOKEN");
    let config_file = std::env::var("CONFIG_FILE").expect("Could not get CONFIG_FILE");

    let config: config::Config =
        serde_json::from_reader(std::fs::File::open(config_file).unwrap()).unwrap();
    println!("config: {:?}", config);
    let config = std::sync::Arc::new(std::sync::Mutex::new(config));

    crate::irc::spawn(config.clone());
    crate::discord::start_and_wait(&discord_token, config);
}

pub mod irc {
    use futures::sync::mpsc::{channel, Sender};
    use irc::client::prelude::*;
    type AppConfig = std::sync::Arc<std::sync::Mutex<crate::config::Config>>;

    pub fn spawn(config: AppConfig) {
        std::thread::spawn(move || {
            tokio::run(futures::future::lazy(move || {
                let (sender, receiver) = channel(100);
                for server in &config.lock().unwrap().servers {
                    spawn_irc_server(sender.clone(), server);
                }

                receiver.for_each(move |msg| {
                    let config: &mut crate::config::Config = &mut config.lock().unwrap();
                    match msg {
                        MessageReceived::PrivMsg {
                            host,
                            from,
                            channel,
                            message,
                        } => {
                            handle_incoming_privmsg(config, host, from, channel, message);
                        }
                        MessageReceived::Log { host, message } => {
                            handle_server_log(config, host, message);
                        }
                    }
                    Ok(())
                })
            }));
            println!("Irc spawn ended");
        });
    }

    fn handle_server_log(config: &mut crate::config::Config, host: String, message: String) {
        if let Some(server) = config.servers.iter_mut().find(|s| s.host == host) {
            crate::discord::send_to_channel(server.log_channel_id, String::from("SYSTEM"), message);
        } else {
            crate::discord::log_warning(
                &config,
                format!("Could not find server by host {:?}", host),
            );
        }
    }

    fn handle_incoming_privmsg(
        config: &mut crate::config::Config,
        host: String,
        from: String,
        channel: String,
        message: String,
    ) {
        if let Some(server) = config.servers.iter_mut().find(|s| s.host == host) {
            if let Some(channel) = server.channels.iter().find(|c| c.name == channel) {
                crate::discord::send_to_channel(channel.discord_channel_id, from, message);
            } else {
                let id = crate::discord::create_channel(config.guild_id, server, &channel);
                server.channels.push(crate::config::Channel {
                    name: channel,
                    discord_channel_id: id,
                });
                config.save();
                crate::discord::send_to_channel(id, from, message);
            }
        } else {
            crate::discord::log_warning(
                &config,
                format!("Could not find server by host {:?}", host),
            );
        }
    }

    #[derive(Debug)]
    pub enum MessageReceived {
        PrivMsg {
            host: String,
            from: String,
            channel: String,
            message: String,
        },
        Log {
            host: String,
            message: String,
        },
    }

    fn spawn_irc_server(mut sender: Sender<MessageReceived>, server: &crate::config::Server) {
        let host = server.host.clone();
        let nick = server.nick.clone();
        let client = IrcClient::from_config(Config {
            nickname: Some(server.nick.clone()),
            nick_password: Some(server.password.clone()),
            server: Some(host.clone()),
            port: Some(server.port),
            use_ssl: Some(server.use_ssl),
            channels: Some(server.channels.iter().map(|c| c.name.clone()).collect()),
            ..Default::default()
        })
        .unwrap();
        client.identify().unwrap();
        tokio::spawn(
            client
                .stream()
                .for_each(move |irc_msg| {
                    let host = host.clone();
                    let nick = nick.clone();
                    if let Command::PRIVMSG(channel, message) = irc_msg.command {
                        let from = irc_msg.prefix.unwrap();
                        let from: String = from.chars().take_while(|c| *c != '!').collect();
                        if channel == nick {
                            sender
                                .try_send(MessageReceived::PrivMsg {
                                    host,
                                    channel: from.to_owned(),
                                    from,
                                    message,
                                })
                                .unwrap();
                        } else {
                            sender
                                .try_send(MessageReceived::PrivMsg {
                                    host,
                                    from,
                                    channel,
                                    message,
                                })
                                .unwrap();
                        }
                    } else {
                        sender.try_send(MessageReceived::Log {
                            host,
                            message: irc_msg.to_string(),
                        });
                    }
                    Ok(())
                })
                .map_err(|e| {
                    eprintln!("IRC error: {:?}", e);
                }),
        );
    }
}

pub mod discord {
    use serenity::client::{Client, EventHandler};
    use serenity::model::channel::ChannelType;
    use serenity::model::id::{ChannelId, GuildId};
    type Config = std::sync::Arc<std::sync::Mutex<crate::config::Config>>;

    pub fn log_warning(config: &crate::config::Config, w: String) {
        let channel_id = config.special_channels.log;
        ChannelId(channel_id)
            .say(w)
            .unwrap_or_else(|e| panic!("Could not log message to channel {}: {:?}", channel_id, e));
    }

    pub fn send_to_channel(id: u64, from: String, message: String) {
        ChannelId(id)
            .say(format!("<{}> {}", from, message))
            .unwrap_or_else(|e| panic!("Could not send message to channel {}: {:?}", id, e));
    }

    pub fn create_channel(guild_id: u64, host: &crate::config::Server, channel: &str) -> u64 {
        let channel = GuildId(guild_id)
            .create_channel(
                channel,
                ChannelType::Text,
                Some(ChannelId(host.discord_channel_id)),
            )
            .expect("Could not create channel");
        channel.id.0
    }

    #[derive(Default)]
    struct Handler {}

    impl EventHandler for Handler {}

    pub fn start_and_wait(discord_token: &str, config: Config) {
        let mut client =
            Client::new(&discord_token, Handler::default()).expect("Could not connect to discord");

        if let Err(e) = client.start() {
            println!("Discord error: {:?}", e);
        }
    }
}

pub mod config {
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Config {
        pub servers: Vec<Server>,
        pub special_channels: SpecialChannels,
        pub guild_id: u64,
    }

    impl Config {
        pub fn save(&self) {
            let config_file = std::env::var("CONFIG_FILE").expect("Could not get CONFIG_FILE");
            println!("Saving to {:?}", config_file);
            let mut file = std::fs::File::create(config_file).expect("Could not open config");
            serde_json::to_writer_pretty(&mut file, self).expect("Could not save config");
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct SpecialChannels {
        pub log: u64,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Server {
        pub host: String,
        pub port: u16,
        pub use_ssl: bool,
        pub nick: String,
        pub password: String,
        pub discord_channel_id: u64,
        pub log_channel_id: u64,

        pub channels: Vec<Channel>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Channel {
        pub name: String,
        pub discord_channel_id: u64,
    }
}
