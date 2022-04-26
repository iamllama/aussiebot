use serde_derive::{Deserialize, Serialize};
use serde_json::Value::{self, Array};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Platform {
    Broadcast,
    Youtube,
    Discord,
    Twitch,
    Web,
}

impl Default for Platform {
    fn default() -> Self {
        Self::Broadcast
    }
}

impl Platform {
    fn from_u64(n: u64) -> Option<Self> {
        match n {
            0 => Some(Platform::Broadcast),
            1 => Some(Platform::Youtube),
            2 => Some(Platform::Discord),
            3 => Some(Platform::Twitch),
            4 => Some(Platform::Web),
            _ => None,
        }
    }

    pub fn from_str(s: impl Into<String>) -> Option<Self> {
        match s.into().to_lowercase().as_ref() {
            "broadcast" => Some(Platform::Broadcast),
            "y" | "yt" | "youtube" => Some(Platform::Youtube),
            "d" | "disc" | "discord" => Some(Platform::Discord),
            "t" | "tw" | "twitch" => Some(Platform::Twitch),
            "web" => Some(Platform::Web),
            _ => None,
        }
    }
}

#[derive(Debug, PartialOrd, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Permissions {
    None = 0,
    Member = 1,
    Admin = 2,
    Owner = 3,
}

impl From<u64> for Permissions {
    fn from(p: u64) -> Self {
        match p {
            0 => Permissions::None,
            1 => Permissions::Member,
            2 => Permissions::Admin,
            3 => Permissions::Owner,
            _ => Permissions::None,
        }
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::None
    }
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub id: String,
    pub platform: Platform,
    pub perms: Permissions,
}

#[derive(Debug, Default)]
pub struct Chat {
    pub src: User,
    pub msg: String,
    pub donation: Option<String>,
}

#[derive(Debug)]
pub enum Stream {
    Started(String), //livestream id
    Stopped(String),
}

impl Stream {
    fn from(n: u64, url: String) -> Option<Self> {
        match n {
            0 => Some(Stream::Started(url)),
            1 => Some(Stream::Stopped(url)),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Message {
    /// Chat instance started
    Started {
        channel: String,
        platform: Platform,
    },
    /// Chat instance stopped
    Stopped {
        channel: String,
        platform: Platform,
    },
    /// Interaction (chat/command) by a user on some platform
    Chat(Chat),
    // Stream notificaiton
    Stream(Stream), //ServerConfig,
    // reply to a PingRequest
    PingResponse(User, String),
}

impl Message {
    pub fn parse(input: impl AsRef<str>) -> Option<Message> {
        if let Array(mut v) = serde_json::from_str(input.as_ref()).ok()? {
            // [channel, platform, msg_type, ...]
            if v.len() < 3 {
                return None;
            }

            let channel = match v[0].take() {
                Value::String(chan) => chan,
                _ => return None,
            };

            let platform = match v[1].take() {
                Value::Number(n) => n.as_u64()?,
                _ => return None,
            };
            let platform = Platform::from_u64(platform)?;

            let msg_type = match v[2].take() {
                Value::Number(n) => n.as_u64()?,
                _ => return None,
            };

            match msg_type {
                0 => Some(Message::Started { channel, platform }),
                1 => Some(Message::Stopped { channel, platform }),
                2 => {
                    // [channel, platform, CHAT, user_name, user_id, user_perms, msg]
                    assert!(v.len() >= 7);
                    let (name, id, perms, msg) = Self::parse_helper(&mut v)?;
                    // only donations can have empty messages
                    if msg.is_empty() {
                        return None;
                    }
                    Some(Message::Chat(Chat {
                        src: User {
                            name,
                            id,
                            platform,
                            perms,
                        },
                        msg,
                        donation: None,
                    }))
                }
                3 => {
                    // [channel, platform, CHAT, user_name, user_id, user_perms, msg, amount]
                    assert!(v.len() == 8);
                    let (name, id, perms, msg) = Self::parse_helper(&mut v)?;
                    let donation = match v[7].take() {
                        Value::String(amount) => Some(amount),
                        _ => None,
                    };
                    Some(Message::Chat(Chat {
                        src: User {
                            name,
                            id,
                            platform,
                            perms,
                        },
                        msg,
                        donation,
                    }))
                }
                4 => {
                    // [channel, platform, STREAM, notify_type, stream_url]
                    assert!(v.len() >= 5);

                    let notify_type = match v[3].take() {
                        Value::Number(n) => n.as_u64()?,
                        _ => return None,
                    };

                    let stream_url = if v.len() >= 5 {
                        match v[4].take() {
                            Value::String(url) => Some(url),
                            _ => None,
                        }
                    } else {
                        None
                    }?;

                    let signal = Stream::from(notify_type, stream_url)?;
                    Some(Message::Stream(signal))
                }
                5 => {
                    // [channel, platform, PING_RESP, user_name, user_platform, msg]
                    assert!(v.len() >= 6);

                    // let user = v[3].take();
                    // let user = serde_json::from_value::<User>(user).ok()?;

                    let name = match v[3].take() {
                        Value::String(name) => name,
                        _ => return None,
                    };

                    let platform = match v[4].take() {
                        Value::String(platform) => platform,
                        _ => return None,
                    };
                    let platform = Platform::from_str(platform)?;

                    let msg = match v[5].take() {
                        Value::String(msg) => msg,
                        _ => return None,
                    };

                    let user = User {
                        name,
                        platform,
                        ..Default::default()
                    };

                    Some(Message::PingResponse(user, msg))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn parse_helper(v: &mut [Value]) -> Option<(String, String, Permissions, String)> {
        let perms = match v[5].take() {
            Value::Number(n) => n.as_u64()?,
            _ => return None,
        };
        let perms = perms.into();

        match (v[3].take(), v[4].take(), v[6].take()) {
            (Value::String(name), Value::String(id), Value::String(msg)) => {
                Some((name, id, perms, msg))
            }
            _ => None,
        }
    }
}
