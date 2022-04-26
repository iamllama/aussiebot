use once_cell::sync::Lazy;
use redis::AsyncCommands;
use serde_derive::Serialize;

use crate::{cmds::ModAction, msg::Platform, RedisPool};

pub static CHANNEL_NAME: Lazy<String> =
    Lazy::new(|| dotenv::var("CHANNEL_NAME").unwrap().to_lowercase());
pub static UPSTREAM_CHAN: Lazy<String> =
    Lazy::new(|| dotenv::var("UPSTREAM_CHAN").unwrap().to_lowercase());
pub static DOWNSTREAM_CHAN: Lazy<String> =
    Lazy::new(|| dotenv::var("DOWNSTREAM_CHAN").unwrap().to_lowercase());

#[derive(Serialize)]
pub struct Response<'a> {
    channel: &'a str,
    dest: (Platform, &'a str, &'a str), // (platform, name, id)
    payload: Payload<'a>,
}

#[derive(Serialize)]
pub enum Payload<'a> {
    #[serde(rename(serialize = "message"))]
    Message(&'a str),
    #[serde(rename(serialize = "modaction"))]
    ModAction(ModAction, &'a str), //(action, filter name aka reason)
    #[serde(rename(serialize = "notify"))]
    Notify(Platform, NotifyType),
    #[serde(rename(serialize = "stream"))]
    Stream(StreamSignal<'a>),
    #[serde(rename(serialize = "ping"))]
    PingRequest(&'a crate::msg::User, &'a str, Option<&'a str>), // src user, platform_id, msg
}

#[derive(Serialize)]
pub enum NotifyType {
    #[serde(rename(serialize = "commands"))]
    Commands,
    #[serde(rename(serialize = "filters"))]
    Filters,
    #[serde(rename(serialize = "timers"))]
    Timers,
    #[serde(rename(serialize = "config"))]
    Config,
}

#[derive(Debug, Serialize)]
pub enum StreamSignal<'a> {
    #[serde(rename(serialize = "start"))]
    Start(&'a str), // livestream id
    #[serde(rename(serialize = "stop"))]
    Stop,
}

impl<'a> Response<'a> {
    pub fn new(dest: (Platform, &'a str, &'a str), payload: Payload<'a>) -> Self {
        Self {
            channel: &*CHANNEL_NAME,
            dest,
            payload,
        }
    }

    // TODO: should be an async trait of RedisPool
    pub async fn send(&self, redis_pool: RedisPool) -> Option<()> {
        let json = serde_json::to_string(self).ok()?;
        //println!("JSON: {}", json);
        redis_pool
            .get()
            .await
            .ok()?
            .publish::<&str, String, _>(&*DOWNSTREAM_CHAN, json)
            .await
            .ok()?
    }
}
