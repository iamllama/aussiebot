mod msg;
mod resp;

use crate::msg::Platform;
use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;
use chrono::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use redis::{AsyncCommands, RedisError};
use regex::Regex;
use resp::{Payload, Response};
use serde_json::json;
use serenity::futures::StreamExt;
use serenity::model::channel::Message;
use serenity::model::gateway::{ActivityType, GatewayIntents, Presence, Ready};
use serenity::model::id::UserId;
use serenity::prelude::*;
use serenity::{async_trait, CacheAndHttp};
use std::sync::Arc;
use tokio::task::JoinHandle;

pub type RedisPool = Pool<RedisConnectionManager>;

struct Handler {
    redis: RedisPool,
    was_streaming: Arc<Mutex<bool>>,
    prev_url: Arc<Mutex<String>>, // prev_stream_url
    cancel_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

static STREAMER_ID: Lazy<u64> =
    Lazy::new(|| dotenv::var("STREAMER_ID").unwrap().parse::<u64>().unwrap());
static AUSSIEBOT_ID: Lazy<u64> =
    Lazy::new(|| dotenv::var("AUSSIEBOT_ID").unwrap().parse::<u64>().unwrap());

// for debouncing spurious status changes
const STATUS_DEBOUNCE_SECS: u64 = 8;

static PING_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)\sfrom\s(\S+)\spinged\syou").unwrap());

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        // ignore self
        if msg.author.id == *AUSSIEBOT_ID {
            return;
        }

        let local: DateTime<Local> = Local::now();

        println!(
            "------------------------\x1b[93mmessage\x1b[0m------------------------\n{}",
            local
        );

        println!(
            "Received message from {} ({:?})",
            //msg.content_safe(_ctx.cache.clone()),
            msg.author.name,
            msg.author.id,
        );

        if let Some(referenced_msg) = msg.referenced_message {
            let orig_reply = referenced_msg.content_safe(_ctx.cache);
            // check if it was a pingrequest
            if let Some(captures) = PING_REGEX.captures(&orig_reply) {
                let name = captures[1].to_owned();
                let platform = Platform::from_str(&captures[2]);

                let msg = format!("{} replied: {}", msg.author.name, msg.content);

                println!("Sending {:?} to {:?} on {:?}", msg, name, platform);

                let resp = json!([
                    resp::CHANNEL_NAME.as_str(),
                    2_u8,
                    5_u8,
                    &name,
                    platform,
                    &msg
                ])
                .to_string();

                //println!("Sending JSON {}\n", resp);
                self.redis
                    .get()
                    .await
                    .unwrap()
                    .publish::<&str, String, ()>(&*resp::UPSTREAM_CHAN, resp)
                    .await
                    .unwrap();
            }
        }
    }

    async fn presence_update(&self, _ctx: Context, new_data: Presence) {
        // check if streamer
        if new_data.user.id != *STREAMER_ID {
            return;
        }

        let local: DateTime<Local> = Local::now();

        println!(
            "--------------------\x1b[93mpresence update\x1b[0m--------------------\n{}",
            local
        );

        // is there at least one streaming-related activity?
        let is_streaming = new_data
            .activities
            .iter()
            .find(|activity| (activity.kind == ActivityType::Streaming));

        // get the stream's url if any
        let (is_streaming, stream_url) = if let Some(act) = is_streaming {
            (true, act.url.as_ref())
        } else {
            (false, None)
        };

        // read previous stream state
        let was_streaming = *self.was_streaming.lock();

        println!(
            "was streaming: {}, is streaming: {}",
            was_streaming, is_streaming
        );
        println!("url: {:?}", stream_url);

        // was_streaming: current chatbot status
        // is_streaming: what discord says

        if !was_streaming && is_streaming {
            // not streaming -> streaming
            // abort cancel task if any
            if let Some(h) = self.cancel_task.lock().take() {
                h.abort();
                println!("Aborted cancel task");
            }
            // get new_url
            let new_url = stream_url.unwrap().to_string();
            let resp = json!([&*resp::CHANNEL_NAME, 2_u8, 4_u8, 0_u8, &new_url]).to_string();
            // update stream state
            {
                let mut was_streaming = self.was_streaming.lock();
                *was_streaming = is_streaming;
                let mut prev_url = self.prev_url.lock();
                *prev_url = new_url;
            }
            // send signal
            self.redis
                .get()
                .await
                .unwrap()
                .publish::<&str, String, ()>(&*resp::UPSTREAM_CHAN, resp)
                .await
                .unwrap();
        } else if was_streaming {
            // abort cancel task if any
            if let Some(h) = self.cancel_task.lock().take() {
                h.abort();
                println!("Aborted cancel task");
            }

            // stop here if no change
            if is_streaming {
                return;
            }

            // streaming -> not streaming
            let was_streaming = self.was_streaming.clone();
            let prev_url = self.prev_url.clone();
            let redis = self.redis.clone();
            let cancel_task = self.cancel_task.clone();

            println!("Spawning cancel task");
            let h = tokio::spawn(async move {
                // wait to debounce status changes
                tokio::time::sleep(std::time::Duration::from_secs(STATUS_DEBOUNCE_SECS)).await;
                println!("IN CANCEL TASK");
                // serialise resp
                let resp = {
                    let mut prev_url = prev_url.lock();
                    let resp = json!([&*resp::CHANNEL_NAME, 2_u8, 4_u8, 1_u8, prev_url.as_str()])
                        .to_string();
                    prev_url.clear();
                    resp
                };
                // update state
                {
                    let mut was_streaming = was_streaming.lock();
                    *was_streaming = false;
                    let mut cancel_task = cancel_task.lock();
                    *cancel_task = None;
                }
                // send signal
                redis
                    .get()
                    .await
                    .unwrap()
                    .publish::<&str, String, ()>(&*resp::UPSTREAM_CHAN, resp)
                    .await
                    .unwrap();
            });
            // store handle
            let mut cancel_task = self.cancel_task.lock();
            *cancel_task = Some(h);
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().unwrap();

    // init redis pool
    let redis_pool = init_redis().await.unwrap();
    let redis_client =
        redis::Client::open(dotenv::var("REDIS_URL").expect("REDIS_URL env var")).unwrap();

    // init handler state
    let handler = Handler {
        redis: redis_pool.clone(),
        was_streaming: Arc::new(Mutex::new(false)),
        prev_url: Arc::new(Mutex::new("".into())),
        cancel_task: Arc::new(Mutex::new(None)),
    };

    // Configure the client with your Discord bot token in the environment.
    let token = dotenv::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Intents are a bitflag, bitwise operations can be used to dictate which intents to use
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_PRESENCES;

    // Build our client.
    let mut client = Client::builder(token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    let cache = client.cache_and_http.clone();

    // start main loop
    tokio::spawn(async move {
        // TODO: add backoff
        println!("\x1b[92m------------------Starting redis loop------------------\x1b[0m");
        loop {
            start_redis_loop(&redis_client, cache.clone()).await;
            // restart loop if process_chat errors out (broken conn etc.)
            println!("\x1b[91m-----------------Restarting redis loop-----------------\x1b[0m");
        }
    });

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

async fn init_redis() -> Result<RedisPool, RedisError> {
    let manager = bb8_redis::RedisConnectionManager::new(
        dotenv::var("REDIS_URL").expect("REDIS_URL env var"),
    )?;
    Pool::builder().max_size(10).build(manager).await
}

/// Process chat messages
async fn start_redis_loop(client: &redis::Client, cache: Arc<CacheAndHttp>) -> Option<()> {
    let mut sub = client.get_tokio_connection().await.ok()?.into_pubsub();
    sub.subscribe(&*resp::DOWNSTREAM_CHAN).await.unwrap();
    let mut sub = sub.into_on_message();
    loop {
        let msg = sub.next().await?.get_payload::<String>().ok()?;
        // println!("redis recv: {}", msg);
        // marshal into Response
        let data = match serde_json::from_str::<Response>(&msg).ok() {
            Some(data) => data,
            _ => continue,
        };
        // check channel
        if data.channel != resp::CHANNEL_NAME.as_str() {
            continue;
        }
        #[allow(irrefutable_let_patterns)] // for future use
        if let Payload::PingRequest(user, pingee_id, msg) = data.payload {
            let msg = if let Some(msg) = msg {
                format!(
                    "{} from {:?} pinged you: {:?} (reply to resp)",
                    user.name, user.platform, msg
                )
            } else {
                format!(
                    "{} from {:?} pinged you! (reply to resp)",
                    user.name, user.platform
                )
            };

            let id = pingee_id.parse::<u64>().ok()?;
            let pingee = UserId(id).to_user(&cache).await.unwrap();
            pingee
                .direct_message(&cache, |m| m.content(msg))
                .await
                .unwrap();
        }
    }
}
