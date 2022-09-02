use crate::{
    error::{self, Error},
    msg::Location,
    RedisPool,
};
use bb8_redis::redis::AsyncCommands;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type Msg = Arc<String>;

// TODO: generalise (Location, String)
pub struct Server {
    msg_in_tx: mpsc::Sender<(Location, String)>, // <- subbo
    msg_out_rx: mpsc::Receiver<Msg>,             // -> pubbo
    pool: RedisPool,
    pub_chan: &'static str,
    sub_chan: &'static str,
}

#[derive(Debug)]
pub struct EOF;

impl std::fmt::Display for EOF {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EOF")
    }
}

impl Server {
    pub fn new(
        pool: RedisPool,
        msg_in_tx: mpsc::Sender<(Location, String)>,
        msg_out_rx: mpsc::Receiver<Msg>,
        pub_chan: &'static str,
        sub_chan: &'static str,
    ) -> Self {
        Self {
            pool,
            msg_in_tx,
            msg_out_rx,
            pub_chan,
            sub_chan,
        }
    }

    async fn sub_task(
        pool: RedisPool,
        msg_in_tx: mpsc::Sender<(Location, String)>,
        sub_chan: &str,
    ) -> error::Result<()> {
        let client = pool.dedicated_connection().await?;
        let mut sub = client.into_pubsub();
        sub.subscribe(sub_chan).await?;
        let mut sub = sub.into_on_message();
        loop {
            // get pubsub message
            let msg = sub.next().await.ok_or(EOF)?.get_payload::<String>()?;
            // wrap with location
            let msg = (Location::Pubsub, msg);
            // forward to msg task
            msg_in_tx.send(msg).await?;
        }
    }

    async fn pub_task(
        pool: RedisPool,
        mut msg_out_rx: mpsc::Receiver<Msg>,
        pub_chan: &'static str,
    ) {
        while let Some(msg) = msg_out_rx.recv().await {
            let redis = pool.clone();
            // spawn a task to publish
            tokio::spawn(async move {
                redis
                    .get()
                    .await
                    .unwrap()
                    .publish::<&str, &str, bool>(pub_chan, &msg)
                    .await
                    .unwrap()
            });
        }
    }

    //const MAX_RETRIES: usize = 10;

    /// Start the server, consuming it
    #[tracing::instrument(skip(self))]
    pub fn start(self) {
        tracing::info!("\x1b[92m-------------Starting pub-sub loop-------------\x1b[0m");

        let Self {
            msg_in_tx,
            msg_out_rx,
            pool,
            pub_chan,
            sub_chan,
        } = self;

        // Spawn sub task in a loop (conn closes during inactivity)
        let _pool = pool.clone();
        tokio::spawn(async move {
            //for _ in 0.. {
            loop {
                match Self::sub_task(_pool.clone(), msg_in_tx.clone(), sub_chan).await {
                    Err(Error::PubSubEOF(e)) => {
                        tracing::trace!("{}", e);
                    }
                    Err(e) => {
                        tracing::error!("{}", e);
                    }
                    Ok(_) => {}
                }
            }
        });

        // Spawn pub task
        tokio::spawn(Self::pub_task(pool, msg_out_rx, pub_chan));

        tracing::info!(chan = sub_chan, "listening");
    }
}
