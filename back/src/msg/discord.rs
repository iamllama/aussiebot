use serde_derive::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct Role {
    pub user_id: Arc<String>,
    pub role_id: Arc<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<Arc<String>>,
    pub reason: Option<Arc<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DiscordAction {
    AddRole(Role),
    RemoveRole(Role),
    StreamerId(Arc<String>),
}

struct DiscordConfig {
    enabled: bool,
    owner_id: String,
    streamer_id: String,
    detect_stream: bool,
    bot_cmds_channel: String,
    stream_announce_channel: String,
}
