use crate::msg::Platform;
use once_cell::sync::Lazy;
use serde::Deserialize;

pub static CHANNEL_NAME: Lazy<String> =
    Lazy::new(|| dotenv::var("CHANNEL_NAME").unwrap().to_lowercase());
pub static UPSTREAM_CHAN: Lazy<String> =
    Lazy::new(|| dotenv::var("UPSTREAM_CHAN").unwrap().to_lowercase());
pub static DOWNSTREAM_CHAN: Lazy<String> =
    Lazy::new(|| dotenv::var("DOWNSTREAM_CHAN").unwrap().to_lowercase());

#[derive(Deserialize)]
pub struct Response<'a> {
    pub channel: &'a str,
    pub dest: (Platform, &'a str, &'a str), // (platform, name, id)
    pub payload: Payload<'a>,
}

#[derive(Deserialize)]
pub enum Payload<'a> {
    #[serde(rename(deserialize = "ping"))]
    PingRequest(crate::msg::User, &'a str, Option<&'a str>), // src user, platform_id, msg
}
