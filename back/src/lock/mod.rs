use crate::{
    error::{self, ChanSendError, Error},
    RedisPool,
};
use bb8_redis::redis;
use tokio::sync::{mpsc, oneshot};

#[allow(dead_code)]
#[derive(Debug)]
enum Lock {
    Lock(String, u64),
    Unlock(String),
}

type Resp = error::Result<bool>;
type TaskChanPair = (Lock, oneshot::Sender<Resp>);

struct Actor {
    rx: mpsc::Receiver<TaskChanPair>,
    pool: RedisPool,
}

/// Handles locking
/// currently backed by redis
impl Actor {
    fn new(rx: mpsc::Receiver<TaskChanPair>, pool: RedisPool) -> Self {
        Self { rx, pool }
    }

    async fn handle_task(pool: RedisPool, (task, tx): TaskChanPair) -> error::Result<()> {
        let mut conn = pool.get().await.unwrap();
        match task {
            Lock::Lock(key, time) => {
                // try to acquire lock
                let locked = redis::cmd("SET")
                    .arg(&[&key, "1", "NX", "EX", &time.to_string()])
                    .query_async::<redis::aio::Connection, bool>(&mut conn)
                    .await
                    .map_err(Error::Redis);
                // send result
                tx.send(locked).map_err(|e| {
                    ChanSendError {
                        msg: format!("{:?}", e),
                    }
                    .into()
                })
                //println!("acquired lock: {:?} ({})", locked, &key);
            }
            Lock::Unlock(key) => {
                // try to release lock
                let unlocked = redis::cmd("DEL")
                    .arg(&key)
                    .query_async::<redis::aio::Connection, bool>(&mut conn)
                    .await
                    .map_err(Error::Redis);
                // send result
                tx.send(unlocked).map_err(|e| {
                    ChanSendError {
                        msg: format!("{:?}", e),
                    }
                    .into()
                })
                //println!("released lock: {:?} ({})", unlocked, &key);
            }
        }
    }

    async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            let pool = self.pool.clone();
            tokio::spawn(Self::handle_task(pool, msg));
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tx: mpsc::Sender<TaskChanPair>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockHandle").finish()
    }
}

impl Handle {
    pub fn new(pool: RedisPool) -> Self {
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(Actor::new(rx, pool).run());
        Self { tx }
    }

    //#[tracing::instrument(skip(self, key), fields(key), ret)]
    pub async fn lock(&self, key: impl Into<String>, time: u64) -> error::Result<bool> {
        let key = key.into();
        tracing::Span::current().record("key", &key.as_str());
        let (resp_tx, resp_rx) = oneshot::channel::<Resp>();
        self.tx.send((Lock::Lock(key, time), resp_tx)).await?;
        // TODO: implement a timeout here
        resp_rx.await?
    }

    //#[tracing::instrument(skip_all, fields(key), ret)]
    pub async fn unlock(&self, key: impl Into<String>) -> error::Result<bool> {
        let key = key.into();
        tracing::Span::current().record("key", &key.as_str());
        let (resp_tx, resp_rx) = oneshot::channel::<Resp>();
        self.tx.send((Lock::Unlock(key), resp_tx)).await?;
        // TODO: implement a timeout here
        resp_rx.await?
    }
}
