use back::{
    auth, cache,
    cmds::{self, ConfigFile},
    db, init_db, init_redis, lock, msg, pubsub, ws,
};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::main;
use tokio::sync::mpsc;
use tracing::Level;
use tracing_subscriber::{fmt::format::FmtSpan, FmtSubscriber};

#[main]
async fn main() {
    dotenv::dotenv().unwrap();

    let file_appender = tracing_appender::rolling::never(
        dotenv::var("LOG_DIR").expect("Log dir in env"),
        "back.log",
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // completes the builder.
        .with_max_level(Level::DEBUG)
        .with_span_events(/*FmtSpan::NEW |*/ FmtSpan::CLOSE)
        .with_writer(non_blocking)
        .with_line_number(true)
        //.with_thread_names(true)
        //.with_target(false)
        //.with_ansi(false)
        //.with_timer(time::LocalTime::rfc_3339()) // time must be built with the unsound_local_offset cfg flag for local timestamps
        .finish();

    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let (db_pool, redis_pool, cmds, filters, timers, users) = tokio::join!(
        init_db(),
        init_redis(),
        cmds::load(ConfigFile::Commands),
        cmds::load(ConfigFile::Filters),
        cmds::load(ConfigFile::Timers),
        auth::load()
    );

    let redis_pool = redis_pool.unwrap();
    let db = db::Handle::new(db_pool.unwrap());

    let cmds = cmds.unwrap();
    let filters = filters.unwrap();
    let timers = timers.unwrap();

    let commands = Arc::new(RwLock::new(Arc::new(cmds)));
    let filters = Arc::new(RwLock::new(Arc::new(filters)));
    let timers = Arc::new(RwLock::new(Arc::new(timers)));

    let lock = lock::Handle::new(redis_pool.clone());
    let cache = cache::Handle::new(redis_pool.clone());

    tracing::info!("commands: {:?}", commands);
    tracing::info!("filters: {:?}", filters);
    tracing::info!("timers: {:?}", timers);

    // plumbing
    // sub/ws -> msg task
    let (msg_in_tx, msg_in_rx) = mpsc::channel::<(msg::Location, String)>(32);
    // msg task -> pub
    let (pub_in_tx, pub_in_rx) = mpsc::channel::<pubsub::Msg>(32);
    // msg task -> ws
    let (ws_in_tx, ws_in_rx) = mpsc::channel::<ws::Msg>(32);
    // start msg loop
    let (msg_out_tx, msg_out_rx) = mpsc::channel::<(msg::Location, msg::Response)>(32);

    let users = users.unwrap();
    tracing::info!("users: {:?}", users);

    let auth = auth::Handle::new(cache.clone(), msg_out_tx.clone(), users);

    let msg = msg::Server {
        pub_in_tx,
        ws_in_tx,
        msg_out_tx,
        commands,
        filters,
        timers,
        db: db.clone(),
        cache: cache.clone(),
        lock: lock.clone(),
        cancel_tasks: RwLock::new(None).into(),
    };
    let hmsg = msg.start(msg_in_rx, msg_out_rx);

    // start redis
    pubsub::Server::new(
        redis_pool.clone(),
        msg_in_tx.clone(),
        pub_in_rx,
        &*back::DOWNSTREAM_CHAN, // as &'static str,
        &*back::UPSTREAM_CHAN,   //as &'static str,
    )
    .start();

    // start ws
    ws::Server::new(msg_in_tx.clone(), ws_in_rx, auth)
        .start()
        .await;

    let _ = tokio::join!(hmsg);
}
