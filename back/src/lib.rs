use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use bb8_redis::RedisConnectionManager;
use error::Error;
use once_cell::sync::Lazy;
use tokio_postgres::NoTls;

pub mod auth;
pub mod cache;
pub mod cmds;
pub mod db;
pub mod error;
pub mod lock;
pub mod msg;
pub mod pubsub;
pub mod ws;

pub type RedisPool = Pool<RedisConnectionManager>;
pub type DbPool = Pool<PostgresConnectionManager<NoTls>>;

pub fn assert_sync<T: ?Sized + Sync>() {}
pub fn assert_send<T: ?Sized + Send>() {}
pub fn assert_send_val<T: ?Sized + Send>(_t: &T) {}
pub fn assert_send_sync_val<T: ?Sized + Sync + Send>(_t: &T) {}

pub static CHANNEL_NAME: Lazy<String> =
    Lazy::new(|| dotenv::var("CHANNEL_NAME").unwrap().to_lowercase());
pub static UPSTREAM_CHAN: Lazy<String> =
    Lazy::new(|| dotenv::var("UPSTREAM_CHAN").unwrap().to_lowercase());
pub static DOWNSTREAM_CHAN: Lazy<String> =
    Lazy::new(|| dotenv::var("DOWNSTREAM_CHAN").unwrap().to_lowercase());
pub static WS_BIND: Lazy<String> = Lazy::new(|| dotenv::var("WS_BIND").unwrap());
pub static CONFIG_DIR: Lazy<String> = Lazy::new(|| dotenv::var("CONFIG_DIR").unwrap());

#[tracing::instrument]
pub async fn init_db() -> error::Result<DbPool> {
    let manager = bb8_postgres::PostgresConnectionManager::new_from_stringlike(
        dotenv::var("DATABASE_CONFIG").expect("DATABASE_CONFIG env var"),
        tokio_postgres::NoTls,
    )?;
    Pool::builder()
        .max_size(10)
        .build(manager)
        .await
        .map_err(Error::Postgres)
}

#[tracing::instrument]
pub async fn init_redis() -> error::Result<RedisPool> {
    let manager = bb8_redis::RedisConnectionManager::new(
        dotenv::var("REDIS_URL").expect("REDIS_URL env var"),
    )?;
    Pool::builder()
        .max_size(10)
        .build(manager)
        .await
        .map_err(Error::Redis)
}
