mod cmds;
mod msg;
mod resp;
mod util;

use crate::{
    cmds::CommandList,
    resp::{Response, UPSTREAM_CHAN},
};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use bb8_redis::RedisConnectionManager;
use clap::Parser;
use futures_util::StreamExt;
use msg::{Message, Permissions, Platform};
use parking_lot::RwLock;
use redis::RedisError;
use std::sync::Arc;
use tokio_postgres::NoTls;
pub type RedisPool = Pool<RedisConnectionManager>;
pub type DbPool = Pool<PostgresConnectionManager<NoTls>>;

/// Aussiebot backend
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Start as a secondary
    #[clap(short, long)]
    secondary: bool,
}

/// TODO: loadbalancer that hashes platform ids into buckets and passes them to a server instance. !set commands are broadcasted and all server instances are assumed to be down till they notify config changed (and maybe dump it for verification)
#[tokio::main]
async fn main() {
    dotenv::dotenv().unwrap();

    // TODO: raft algorithm to decide leader
    let args = Args::parse();
    // at the moment this is only used for Timer tasks
    let is_main_instance: bool = !args.secondary;

    // init redis and db pools
    let redis_client =
        redis::Client::open(dotenv::var("REDIS_URL").expect("REDIS_URL env var")).unwrap();
    let redis_pool = init_redis().await.unwrap();
    let db_pool = init_db().await.unwrap();

    // try to load config from disk, otherwise init defaults
    let commands = if let Some(cmdlist) =
        util::load_config(redis_pool.clone(), db_pool.clone(), is_main_instance)
    {
        Arc::new(cmds::CommandListInner {
            filters: RwLock::new(cmdlist.filters),
            commands: RwLock::new(cmdlist.commands),
            timers: RwLock::new(cmdlist.timers),
            is_main_instance: cmdlist.is_main_instance,
            timer_handles: RwLock::new(cmdlist.timer_handles),
        })
    } else {
        println!("\x1b[91mFailed to load config, initialising with default values\x1b[0m");
        let commands = cmds::init(redis_pool.clone(), db_pool.clone());
        // add filters
        add_aussie_filters(commands.clone(), redis_pool.clone(), db_pool.clone());
        commands
    };

    // init timers
    {
        let mut timer_handles = commands.timer_handles.write(); //.unwrap();
        let timers = commands.timers.read();
        //DerefMut is pog
        *timer_handles = cmds::Timer::start(&timers, is_main_instance).await.unwrap();
    }

    // init signals
    init_signals(commands.clone(), redis_pool.clone(), db_pool.clone()).await;

    // start main loop
    tokio::spawn(async move {
        // TODO: add backoff
        println!("\x1b[92m--------------Starting redis loop--------------\x1b[0m");
        loop {
            start_redis_loop(redis_pool.clone(), &redis_client, commands.clone()).await;
            // restart loop if process_chat errors out (broken conn etc.)
            println!("\x1b[91m-------------Restarting redis loop-------------\x1b[0m");
        }
    })
    .await
    .unwrap();
}

/// Process chat messages
async fn start_redis_loop(
    redis_pool: RedisPool,
    client: &redis::Client,
    cmdlist: CommandList,
) -> Option<()> {
    let mut sub = client.get_tokio_connection().await.ok()?.into_pubsub();
    sub.subscribe(&*UPSTREAM_CHAN).await.unwrap();
    let mut sub = sub.into_on_message();

    loop {
        // get pubsub message
        let msg = sub.next().await?.get_payload::<String>().ok()?;
        // try to parse as Message
        let msg = if let Some(msg) = Message::parse(msg) {
            msg
        } else {
            continue;
        };
        match msg {
            Message::Chat(chat) => {
                println!("-----------------\x1b[93mChat received\x1b[0m-----------------");
                println!("{:?}", chat);
                let cmdlist = cmdlist.clone();
                let redis_pool = redis_pool.clone();

                tokio::spawn(async move {
                    // create a new context for running filters and commands
                    let ctx = cmds::Context::new(cmdlist, &chat);
                    // run all filters
                    if let Some((action, filter_name)) = ctx.filter().await {
                        // send filter action
                        Response::new(
                            (chat.src.platform, &chat.src.name, &chat.src.id),
                            resp::Payload::ModAction(action, &filter_name),
                        )
                        .send(redis_pool)
                        .await
                        .unwrap();
                    } else {
                        // run cmds to completion
                        ctx.run().await;
                    }
                    // time taken to process chat
                    println!("\x1b[38;5;8m{}s\x1b[0m", ctx.time.elapsed().as_secs_f32());
                });
            }
            Message::Started { .. } => {}
            Message::Stopped { .. } => {}
            Message::Stream(notify_type) => {
                println!("Stream: {:?}", notify_type);
                match notify_type {
                    msg::Stream::Started(url) => {
                        // parse stream url
                        let (platform, captures) =
                            if let Some(captures) = cmds::YOUTUBE_REGEX.captures(&url) {
                                (Platform::Youtube, captures)
                            } else {
                                continue;
                            };
                        let url = &captures[1];
                        // send start signal
                        let signal = resp::StreamSignal::Start(url);
                        cmds::Stream::signal_stream(redis_pool.clone(), platform, signal).await?;
                    }
                    msg::Stream::Stopped(url) => {
                        // parse stream url
                        let platform = if cmds::YOUTUBE_REGEX.is_match(&url) {
                            Platform::Youtube
                        } else {
                            continue;
                        };
                        // send stop signal
                        let signal = resp::StreamSignal::Stop;
                        cmds::Stream::signal_stream(redis_pool.clone(), platform, signal).await?;
                    }
                }
            }
            Message::PingResponse(dest, msg) => {
                println!("PingResponse: {:?} {:?}", dest, msg);
                resp::Response::new(
                    (dest.platform, &dest.name, &dest.id),
                    resp::Payload::Message(&msg),
                )
                .send(redis_pool.clone())
                .await?;
            }
        }
    }
}

async fn init_redis() -> Result<RedisPool, RedisError> {
    let manager = bb8_redis::RedisConnectionManager::new(
        dotenv::var("REDIS_URL").expect("REDIS_URL env var"),
    )?;
    Pool::builder().max_size(10).build(manager).await
}

async fn init_db() -> Option<DbPool> {
    let manager = bb8_postgres::PostgresConnectionManager::new_from_stringlike(
        dotenv::var("DATABASE_CONFIG").expect("DATABASE_CONFIG env var"),
        tokio_postgres::NoTls,
    )
    .ok()?;
    Pool::builder().max_size(10).build(manager).await.ok()
}

fn add_aussie_filters(commands: CommandList, redis_pool: RedisPool, db_pool: DbPool) -> Option<()> {
    let mut cam_filter = cmds::Command::Filter(cmds::Filter::new(
        "name contains 'bestcam'",
        redis_pool.clone(),
        db_pool.clone(),
    ));
    cam_filter
        .set(
            "user_contains",
            cmds::Value::String(Arc::new("bestcam".into())),
        )?
        .set("timeout", cmds::Value::Bool(true))?;

    let mut masochist_timeout_filter = cmds::Command::Filter(cmds::Filter::new(
        "asked for a timeout",
        redis_pool.clone(),
        db_pool.clone(),
    ));
    masochist_timeout_filter
        .set(
            "msg_contains",
            cmds::Value::String(Arc::new("i want a timeout".into())),
        )?
        .set("timeout", cmds::Value::Bool(true))?;

    let mut masochist_ban_filter = cmds::Command::Filter(cmds::Filter::new(
        "asked for a ban",
        redis_pool.clone(),
        db_pool.clone(),
    ));
    masochist_ban_filter
        .set(
            "msg_contains",
            cmds::Value::String(Arc::new("i want a ban".into())),
        )?
        .set("ban", cmds::Value::Bool(true))?;

    let mut tech_filter = cmds::Command::Filter(cmds::Filter::new(
        ".tech",
        redis_pool.clone(),
        db_pool.clone(),
    ));
    tech_filter
        .set(
            "msg_contains",
            cmds::Value::String(Arc::new(".tech".into())),
        )?
        .set("ban", cmds::Value::Bool(true))?;

    let mut streamlabs_filter = cmds::Command::Filter(cmds::Filter::new(
        "streamlabs",
        redis_pool.clone(),
        db_pool.clone(),
    ));
    streamlabs_filter
        .set(
            "id_contains",
            cmds::Value::String(Arc::new("UCNL8jaJ9hId96P13QmQXNtA".into())),
        )?
        .set("apply_to", cmds::Value::Permissions(Permissions::Admin))?;

    let mut filters = commands.filters.write();
    filters.clear();
    filters.extend([
        cam_filter,
        masochist_timeout_filter,
        streamlabs_filter
        //masochist_ban_filter,
        //tech_filter,
    ]);

    let mut commands = commands.commands.write();
    // disable all cmds except chat and french
    for cmd in &mut *commands {
        match cmd {
            cmds::Command::Chat(_)
            | cmds::Command::French(_)
            | cmds::Command::Gamble(_)
            | cmds::Command::Give(_)
            | cmds::Command::Ban(_)
            | cmds::Command::Heist(_)
            | cmds::Command::Help(_)
            | cmds::Command::Text(_)
            | cmds::Command::ToggleFilters(_)
            | cmds::Command::Dump(_)
            | cmds::Command::Set(_)
            | cmds::Command::Points(_)
            | cmds::Command::Transfer(_) => {
                cmd.set("enabled", cmds::Value::Bool(true));
            }
            _ => {
                cmd.set("enabled", cmds::Value::Bool(false));
            }
        }
    }

    // rename commands to avoid conflicts with streamlabs
    for cmd in &mut *commands {
        match cmd {
            cmds::Command::Gamble(_) | cmds::Command::Give(_) | cmds::Command::Heist(_) => {
                if let Some(cmds::Value::String(name)) = cmd.get("name") {
                    let mut name: String = String::to_owned(&name);
                    name.push('_');
                    cmd.set("name", cmds::Value::String(Arc::new(name)))?;
                }
            }
            _ => {}
        }
    }

    // cmdlist.commands.clear();
    // cmdlist
    //     .commands
    //     .push(cmds::Command::Set(cmds::Set::new("", redis_pool, db_pool)));

    let mut chairgg = cmds::Command::Text(cmds::Text::new("chairgg", redis_pool, db_pool));
    chairgg
        .set("name", cmds::Value::String(Arc::new("!chairgg".into())))?
        .set(
            "msg",
            cmds::Value::String(Arc::new(
                "ratJAM ratJAM this is now a certified chairGG™ stream ratJAM ratJAM".into(),
            )),
        )?
        .set("perms", cmds::Value::Permissions(Permissions::Member))?;

    commands.push(chairgg);

    //println!("Commands: {:#?}", cmdlist);

    Some(())
}

use tokio::signal::unix::{signal, SignalKind};

// Setup SIGHUP handler to reload config
async fn init_signals(cmdlist: CommandList, redis: RedisPool, db: DbPool) -> Option<()> {
    let mut stream = signal(SignalKind::hangup()).ok()?;
    println!("Signal handler registered, waiting");
    tokio::spawn(async move {
        loop {
            stream.recv().await;
            println!("\x1b[95mgot signal HUP, reloading config\x1b[0m");

            let old_timer_handles = cmdlist.timer_handles.read().clone();

            // Set new config
            cmds::Set::set_config(
                cmdlist.is_main_instance,
                &old_timer_handles,
                cmdlist.clone(),
                redis.clone(),
                db.clone(),
            )
            .await
            .unwrap();
        }
    });

    Some(())
}
