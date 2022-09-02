use crate::{
    error::{self, ChanSendError, Error},
    RedisPool,
};
use bb8_redis::redis::{self, AsyncCommands, RedisError};
use std::{fmt::Debug, sync::Arc};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub(crate) enum Cache {
    /// key, delta, expiry
    Increment(Arc<String>, usize, usize),
    Delete(Arc<String>),
    Get(Arc<String>),
    GetDel(Arc<String>),
    /// key, value, expiry, exclusive
    Set(Arc<String>, Arc<String>, usize, bool),
    SetGet(Arc<String>, Arc<String>, usize),
    //HashLen(&'static str),
    HashSet(Arc<String>, Arc<String>, String, bool),
    //HashGet(Arc<String>, String),
    HashGetAll(Arc<String>),
    //HashRand(&'static str, u64),
    Zadd(Arc<String>, Arc<String>, Arc<String>),
    /// key, min, max
    Zremrangebyscore(Arc<String>, Arc<String>, Arc<String>),
    /// key, start, stop
    Zrange(Arc<String>, isize, isize),
    /// key, start, stop
    Zrangewithscores(Arc<String>, isize, isize),
    Zpopmax(Arc<String>, isize),
}

type Resp = error::Result<RespType>;
type TaskChanPair = (Cache, oneshot::Sender<Resp>);

impl Cache {
    #[tracing::instrument(level = "debug", skip(handle), ret)]
    pub(crate) async fn exec(self, handle: &Handle) -> error::Result<RespType> {
        handle.task(self).await
    }
}

//#[derive(Debug)]
pub(crate) enum RespType {
    Bool(bool),
    U64(u64),
    String(String),
    VecString(Vec<String>),
    VecStringScore(Vec<(String, isize)>),
    VecStringString(Vec<(String, String)>),
}

// hide potentially massive inner value from tracing
impl Debug for RespType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(arg0) => f.debug_tuple("Bool").field(arg0).finish(),
            Self::U64(arg0) => f.debug_tuple("U64").field(arg0).finish(),
            Self::String(arg0) => f.debug_tuple("String").field(arg0).finish(),
            Self::VecString(arg0) => f.debug_tuple("VecString").field(&arg0.len()).finish(),
            Self::VecStringScore(arg0) => {
                f.debug_tuple("VecStringScore").field(&arg0.len()).finish()
            }
            Self::VecStringString(arg0) => {
                f.debug_tuple("VecStringString").field(&arg0.len()).finish()
            }
        }
    }
}

struct Actor {
    rx: mpsc::Receiver<TaskChanPair>,
    pool: RedisPool,
}

/// Handles store access
/// currently backed by redis
impl Actor {
    fn new(rx: mpsc::Receiver<TaskChanPair>, pool: RedisPool) -> Self {
        Self { rx, pool }
    }

    async fn handle_task(pool: RedisPool, (task, tx): TaskChanPair) -> error::Result<()> {
        let resp = Self::_handle_task(pool, task).await.map_err(Error::Redis);
        tx.send(resp).map_err(|e| {
            ChanSendError {
                msg: format!("{:?}", e),
            }
            .into()
        })
    }

    async fn _handle_task(pool: RedisPool, task: Cache) -> Result<RespType, RedisError> {
        let mut conn = pool.get().await.unwrap();
        match task {
            Cache::Increment(key, delta, expire) => {
                // atomically increment count
                let mut cmd = redis::pipe();
                cmd.incr(&*key, delta);
                if expire > 0 {
                    cmd.expire(&*key, expire).ignore();
                }
                cmd.query_async::<redis::aio::Connection, (u64,)>(&mut conn)
                    .await
                    .map(|(r,)| RespType::U64(r))
            }
            Cache::Delete(key) => redis::cmd("DEL")
                .arg(key.as_str())
                .query_async::<redis::aio::Connection, bool>(&mut conn)
                .await
                .map(RespType::Bool),
            Cache::Get(key) => redis::cmd("GET")
                .arg(&[&key.as_str()])
                .query_async::<redis::aio::Connection, String>(&mut conn)
                .await
                .map(RespType::String),
            Cache::GetDel(key) => redis::cmd("GETDEL")
                .arg(&[&key.as_str()])
                .query_async::<redis::aio::Connection, String>(&mut conn)
                .await
                .map(RespType::String),
            Cache::Set(key, value, ex, nx) => {
                let mut cmd = redis::cmd("SET");
                cmd.arg(&*key).arg(&*value);
                if ex > 0 {
                    cmd.arg("EX").arg(ex);
                }
                if nx {
                    cmd.arg("NX");
                }
                cmd.query_async::<redis::aio::Connection, bool>(&mut conn)
                    .await
                    .map(RespType::Bool)
            }
            Cache::SetGet(key, value, expire) => {
                let mut cmd = redis::pipe();
                cmd.cmd("SET").arg(&[&key, value.as_str(), "GET"]);
                if expire > 0 {
                    cmd.expire(&*key, expire).ignore();
                }
                cmd.query_async::<redis::aio::Connection, (String,)>(&mut conn)
                    .await
                    .map(|(r,)| RespType::String(r))
            }
            // Cache::HashLen(key) => {
            //     let resp = redis::cmd("HLEN")
            //         .arg(&key)
            //         .query_async::<redis::aio::Connection, u64>(&mut conn)
            //         .await
            //         .ok();
            //     let _ = tx.send(resp.map(RespType::U64));
            // }
            Cache::HashSet(key, field, value, exclusive) => {
                redis::cmd(if exclusive { "HSETNX" } else { "HSET" })
                    .arg(&[key.as_str(), field.as_str(), value.as_str()])
                    .query_async::<redis::aio::Connection, bool>(&mut conn)
                    .await
                    .map(RespType::Bool)
            }
            // Cache::HashGet(key, field) => {
            //     let resp = redis::cmd("HGET")
            //         .arg(&[key.as_str(), field.as_str()])
            //         .query_async::<redis::aio::Connection, String>(&mut conn)
            //         .await
            //         .ok();
            //     let _ = tx.send(resp.map(RespType::String));
            // }
            Cache::HashGetAll(key) => conn.hgetall(&*key).await.map(RespType::VecStringString),
            // Cache::HashRand(key, num) => {
            //     let resp = redis::cmd("HRANDFIELD")
            //         .arg(&[key, &num.to_string()])
            //         .query_async::<redis::aio::Connection, Vec<String>>(&mut conn)
            //         .await
            //         .ok();
            //     let _ = tx.send(resp.map(RespType::VecString));
            // }
            Cache::Zadd(key, score, value) => redis::cmd("ZADD")
                .arg(&[key.as_str(), score.as_str(), value.as_str()])
                .query_async::<redis::aio::Connection, bool>(&mut conn)
                .await
                .map(RespType::Bool),
            Cache::Zremrangebyscore(key, min, max) => redis::cmd("ZREMRANGEBYSCORE")
                .arg(&[key.as_str(), min.as_str(), max.as_str()])
                .query_async::<redis::aio::Connection, bool>(&mut conn)
                .await
                .map(RespType::Bool),
            Cache::Zrange(key, start, stop) => conn
                .zrange(&*key, start, stop)
                .await
                .map(RespType::VecString),
            Cache::Zrangewithscores(key, start, stop) => conn
                .zrange_withscores(&*key, start, stop)
                .await
                .map(RespType::VecStringScore),
            Cache::Zpopmax(key, count) => conn
                .zpopmax(&*key, count)
                .await
                .map(RespType::VecStringScore),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_task(pool, msg).await {
                    tracing::error!("{}", e);
                }
            });
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tx: mpsc::Sender<TaskChanPair>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheHandle").finish()
    }
}

impl Handle {
    pub fn new(pool: RedisPool) -> Self {
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(Actor::new(rx, pool).run());
        Self { tx }
    }

    async fn task(&self, task: Cache) -> error::Result<RespType> {
        let (resp_tx, resp_rx) = oneshot::channel::<Resp>();
        self.tx.send((task, resp_tx)).await?;
        // TODO: implement a timeout here
        resp_rx.await.expect("Cache task killed")
    }
}
