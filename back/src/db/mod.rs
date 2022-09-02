pub(crate) mod give;
pub(crate) mod hours;
pub(crate) mod link;
pub(crate) mod modaction;

use self::{give::GiveOp, hours::HoursOp, link::LinkOp, modaction::ModActionDump};
use crate::{
    cmds::ModAction,
    error::{self, ChanSendError},
    msg::Platform,
    DbPool,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

#[allow(dead_code)]
#[derive(Debug)]

pub(crate) enum Db {
    Upsert(Platform, Arc<String>, Arc<String>, i32),
    GetPoints(Platform, Arc<String>),
    SetPoints(Platform, Arc<String>, i32),
    Give(GiveOp),
    ModAction(Platform, Arc<String>, ModAction, Arc<String>),
    Link(LinkOp),
    Hours(HoursOp),
    DumpModActions,
}

impl Db {
    #[tracing::instrument(level = "debug", skip(handle), ret)]
    pub(crate) async fn exec(self, handle: &Handle) -> error::Result<Resp> {
        handle.task(self).await
    }
}

pub enum Resp {
    Ok,
    GetPoints([(Platform, Option<i32>); 3]),
    Give(i32),
    Hours(i32),
    ModActionDump(ModActionDump),
}

// hide potentially massive inner value from tracing
impl std::fmt::Debug for Resp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "Ok"),
            Self::GetPoints(arg0) => f.debug_tuple("GetPoints").field(arg0).finish(),
            Self::Give(arg0) => f.debug_tuple("Give").field(arg0).finish(),
            Self::Hours(arg0) => f.debug_tuple("Hours").field(arg0).finish(),
            Self::ModActionDump(arg0) => {
                let mut _f = f.debug_tuple("ModActionDump");
                for (plat, rows) in arg0 {
                    _f.field(&(plat, rows.len()));
                }
                _f.finish()
            }
        }
    }
}

type TaskChanPair = (Db, oneshot::Sender<error::Result<Resp>>);

struct Actor {
    rx: mpsc::Receiver<TaskChanPair>,
    db: DbPool,
}

/// Database operations
/// Currently backed by psql
impl Actor {
    // fn new(db: DbPool, rx: mpsc::Receiver<TaskChanPair>) -> Self {
    //     Self { rx, db }
    // }

    async fn handle_task(db: DbPool, (task, tx): TaskChanPair) {
        let resp = Self::_handle_task(db, task).await;
        let res: error::Result<()> = tx.send(resp).map_err(|e| {
            ChanSendError {
                msg: format!("{:?}", e),
            }
            .into()
        });
        if let Err(e) = res {
            tracing::error!("{}", e);
        }
    }

    async fn _handle_task(db: DbPool, task: Db) -> error::Result<Resp> {
        match task {
            Db::GetPoints(platform, id) => {
                let sql = match platform {
                    Platform::YOUTUBE => include_str!("sql/select/points_youtube.sql"),
                    Platform::DISCORD => include_str!("sql/select/points_discord.sql"),
                    Platform::TWITCH => include_str!("sql/select/points_twitch.sql"),
                    _ => unreachable!(),
                };
                let client = db.get().await?;
                let row = client.query_one(sql, &[&id.as_str()]).await?;
                let youtube_points = row.try_get::<_, i32>(3).ok();
                let discord_points = row.try_get::<_, i32>(4).ok();
                let twitch_points = row.try_get::<_, i32>(5).ok();

                tracing::info!(
                    "Db::GetPoints({:?}, {}) => [{:?}, {:?}, {:?}]",
                    platform,
                    id,
                    youtube_points,
                    discord_points,
                    twitch_points
                );

                Ok(Resp::GetPoints([
                    (Platform::YOUTUBE, youtube_points),
                    (Platform::DISCORD, discord_points),
                    (Platform::TWITCH, twitch_points),
                ]))
            }
            Db::SetPoints(platform, name, points) => {
                let sql = match platform {
                    Platform::YOUTUBE => include_str!("sql/update/set_points_youtube.sql"),
                    Platform::DISCORD => include_str!("sql/update/set_points_discord.sql"),
                    Platform::TWITCH => include_str!("sql/update/set_points_twitch.sql"),
                    _ => unreachable!(),
                };
                let client = db.get().await?;
                let _ = client.query_one(sql, &[&name.as_str(), &points]).await;

                tracing::info!(to = points, "set points");
                Ok(Resp::Ok)
            }
            Db::Upsert(platform, id, name, points) => {
                let sql = match platform {
                    Platform::YOUTUBE => include_str!("sql/upsert/youtube_id.sql"),
                    Platform::DISCORD => include_str!("sql/upsert/discord_id.sql"),
                    Platform::TWITCH => include_str!("sql/upsert/twitch_id.sql"),
                    _ => unreachable!(),
                };
                let client = db.get().await?;
                let _ = client
                    .query_one(sql, &[&id.as_str(), &name.as_str(), &points])
                    .await?;

                tracing::info!(by = points, "incremented points");
                Ok(Resp::Ok)
            }
            Db::Give(args) => give::op(db, args).await.map(Resp::Give),
            Db::ModAction(platform, id, action, reason) => {
                let sql = match platform {
                    Platform::YOUTUBE => include_str!("sql/insert/modaction_youtube.sql"),
                    Platform::DISCORD => include_str!("sql/insert/modaction_discord.sql"),
                    Platform::TWITCH => include_str!("sql/insert/modaction_twitch.sql"),
                    _ => unreachable!(),
                };
                let client = db.get().await?;
                let _ = client
                    .query_one(
                        sql,
                        &[
                            &id.as_str(),
                            /*&(action as i32)*/ &(action.to_string()),
                            &reason.as_str(),
                        ],
                    )
                    .await?;
                Ok(Resp::Ok)
            }
            Db::Link(args) => link::op(db, args).await.map(|_| Resp::Ok),
            Db::Hours(args) => hours::op(db, args).await.map(Resp::Hours),
            Db::DumpModActions => modaction::op(db).await.map(Resp::ModActionDump),
        }
    }

    async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            let db = self.db.clone();
            tokio::spawn(Self::handle_task(db, msg));
        }
    }
}

#[derive(Clone)]
pub struct Handle {
    tx: mpsc::Sender<TaskChanPair>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbHandle").finish()
    }
}

impl Handle {
    pub fn new(db: DbPool) -> Self {
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(Actor { db, rx }.run());

        Self { tx }
    }

    async fn task(&self, task: Db) -> error::Result<Resp> {
        let (tx, rx) = oneshot::channel::<error::Result<Resp>>();
        self.tx.send((task, tx)).await?;
        rx.await.expect("Actor task killed")
    }
}
