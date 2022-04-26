use serde_derive::{Deserialize, Serialize};

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

#[derive(Debug, PartialOrd, PartialEq, Clone, Copy, Deserialize)]
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

#[derive(Debug, Default, Clone, Deserialize)]
pub struct User {
    pub name: String,
    pub id: String,
    pub platform: Platform,
    pub perms: Permissions,
}
