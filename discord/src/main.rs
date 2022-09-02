mod discord;
mod msg;

use crate::discord::Handler;
use back::msg::{Location, Response};
use back::{init_redis, pubsub};
use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;
use parking_lot::{Mutex, RwLock};
use serenity::model::gateway::GatewayIntents;
use serenity::prelude::*;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
//use tracing_subscriber::fmt::format::FmtSpan;
use tokio::main;
use tracing_subscriber::filter::{LevelFilter, Targets};

pub type RedisPool = Pool<RedisConnectionManager>;

#[main]
async fn main() {
    dotenv::dotenv().unwrap();

    let filter = Targets::new()
        .with_target("discord", LevelFilter::DEBUG)
        .with_target("back", LevelFilter::DEBUG)
        .with_target("serenity", LevelFilter::WARN)
        .with_target("h2", LevelFilter::WARN);

    let file_appender = tracing_appender::rolling::never(
        dotenv::var("LOG_DIR").expect("Log dir in env"),
        "disc.log",
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(non_blocking),
        )
        .init();

    let was_streaming = std::env::var("STARTED").is_ok();
    //println!("was_streaming: {}", was_streaming);
    tracing::info!(was_streaming = was_streaming);

    // Configure the client with your Discord bot token in the environment.
    let token = dotenv::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // validate_token is currently broken for new tokens
    //utils::validate_token(&token).expect("Expected a valid discord token");

    // Intents are a bitflag, bitwise operations can be used to dictate which intents to use
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS // for updating the cache after changing roles
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_PRESENCES;

    let cmd_cache = Arc::new(RwLock::new(None));

    // plumbing
    let (pub_in_tx, pub_in_rx) = mpsc::channel::<pubsub::Msg>(32);
    //let (discord_out_tx, discord_out_rx) = mpsc::channel::<discord::DiscordEvent>(32);

    // start msg loop
    let (msg_in_tx, msg_in_rx) = mpsc::channel::<(Location, String)>(32);
    let (msg_out_tx, msg_out_rx) = mpsc::channel::<(Location, Response)>(32);

    // init handler state
    let handler = Handler {
        msg_out_tx: msg_out_tx.clone(),
        cancel_chan: Arc::new(Mutex::new(None)), //watch::channel(()),
        was_streaming: Arc::new(AtomicBool::new(false)),
        stream_url: Arc::new(Mutex::new(Arc::new("".into()))),
        stream_announced: Arc::new(AtomicBool::new(false)),
        mee6_last_url: Arc::new(Mutex::new(Arc::new("".into()))),
        cmd_cache: cmd_cache.clone(),
        streamer_id: Arc::new(RwLock::new(*discord::OWNER_ID)),
    };

    // Build our client.
    let mut client = Client::builder(token, intents)
        .event_handler(handler.clone())
        .await
        .expect("Error creating client");

    let cache = client.cache_and_http.clone();

    let msg = msg::Server {
        pub_in_tx,
        msg_out_tx: msg_out_tx.clone(),
        handler,
        cache,
        cmd_cache,
    };

    msg.start(msg_in_rx, msg_out_rx);

    // start pubsub
    start_pubsub(msg_in_tx, pub_in_rx).await;

    //let _ = tokio::join!(client.start(), hmsg);
    client.start().await.unwrap();
}

async fn start_pubsub(
    msg_in_tx: mpsc::Sender<(Location, String)>,
    pub_in_rx: mpsc::Receiver<pubsub::Msg>,
) {
    // init redis pool
    let pool = init_redis().await.unwrap();

    // start pubsub
    pubsub::Server::new(
        pool,
        msg_in_tx,
        pub_in_rx,
        &*back::UPSTREAM_CHAN,
        &*back::DOWNSTREAM_CHAN,
    )
    .start();
}
