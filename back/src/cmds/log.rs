use super::{Context, ModAction, RunRes};
use crate::{
    cache::{self, Cache, RespType},
    db::{self, modaction::ModActionDump, Db, Resp},
    error,
    msg::{Chat, Invocation, Platform, CHAT_PLATFORMS},
};
use back_derive::command;
use once_cell::sync::Lazy;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;
use tracing::{debug_span, Instrument};

static YT_KEY: Lazy<String> = Lazy::new(|| format!("{}_{:?}", &*LOG_LOCK_LIST, Platform::YOUTUBE));
static DISCORD_KEY: Lazy<String> =
    Lazy::new(|| format!("{}_{:?}", &*LOG_LOCK_LIST, Platform::DISCORD));
static TWITCH_KEY: Lazy<String> =
    Lazy::new(|| format!("{}_{:?}", &*LOG_LOCK_LIST, Platform::TWITCH));
static _AUSSIEBOT_KEY: Lazy<String> = Lazy::new(|| format!("{}_ab", &*LOG_LOCK_LIST));

#[command(locks(list))]
/// Log recent messages for inspection
pub struct Log {
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Duration to keep a message for (in seconds)
    #[cmd(def(10u64), constr(range = "10..=3600"))]
    keep_for: u64,
}

impl Log {
    fn can_run(&self, ctx: &Context<'_>) -> Option<()> {
        if !self.enabled {
            return None;
        }

        // check if platform is applicable
        if !self.platforms.contains(ctx.platform) {
            return None;
        }

        Some(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn chat(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        if self.can_run(ctx).is_none() {
            return Ok(RunRes::Disabled);
        }
        self.run(ctx, chat).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        _ctx: &Context<'_>,
        _invocation: &Invocation,
    ) -> Option<RunRes> {
        None
    }

    /// Current timestamp with ms resolution, minus `minus`
    fn timestamp(minus: u64) -> error::Result<String> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;
        Ok(timestamp
            .as_secs()
            .wrapping_sub(minus)
            .wrapping_mul(1000) // overflow is ok, since overlap is practically impossible
            .wrapping_add(timestamp.subsec_millis() as u64) // extra resolution
            .to_string())
    }

    /// Implicit log fn that stores msgs in chats for a specified duration
    #[tracing::instrument(level = "trace", skip_all, name = "Log")]
    async fn run(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        if !self.enabled {
            return Ok(RunRes::Disabled);
        }

        // TODO: memoize this
        let list_key = Self::get_keys(&ctx.platform);
        if list_key.len() != 1 {
            return Ok(RunRes::InvalidArgs); //TODO: should be an assert
        }
        let list_key = list_key[0].1;

        let timestamp = Arc::new(Self::timestamp(0)?);
        // include timestamp in value to prevent deduping when inserting into the set
        let item = (timestamp.clone(), chat.clone());
        let msg = tokio::task::spawn_blocking(move || serde_json::to_string(&item)).await??;

        Cache::Zadd(list_key.to_owned().into(), timestamp, msg.into())
            .exec(ctx.cache)
            .await?;

        tracing::info!(platform = %ctx.platform, "logged");

        Ok(RunRes::Noop)
    }

    fn get_keys(platform: &Platform) -> Vec<(Platform, &'static str)> {
        let mut keys = vec![];
        if platform.contains(Platform::YOUTUBE) {
            keys.push((Platform::YOUTUBE, &*YT_KEY as &'static str));
        }
        if platform.contains(Platform::DISCORD) {
            keys.push((Platform::DISCORD, &*DISCORD_KEY));
        }
        if platform.contains(Platform::TWITCH) {
            keys.push((Platform::TWITCH, &*TWITCH_KEY));
        }
        keys
    }

    // TODO: doesn't need to be kept running, run on every nth chat msg or smth
    /// Remove messages older than keep_for
    async fn cleanup(
        platforms: &Platform,
        keep_for: u64,
        cache: &cache::Handle,
    ) -> error::Result<()> {
        // ZREMRANGEBYSCORE aussiebot_aussiegg_log_list_YOUTUBE -inf (currrent timestamp - keep_for)

        let list_keys = Self::get_keys(platforms);

        if list_keys.is_empty() {
            return Ok(());
        }

        let timestamp = Arc::new(Self::timestamp(keep_for)?);

        let futures = list_keys.iter().map(|key| {
            Cache::Zremrangebyscore(
                key.1.to_owned().into(),
                "-inf".to_owned().into(),
                timestamp.clone(),
            )
            .exec(cache)
        });

        futures_util::future::join_all(futures).await;

        // for list_key in list_keys {
        //     cache
        //         .task(Cache::Zremrangebyscore(list_key, "-inf", timestamp.clone()))
        //         .await?;
        // }

        Ok(())
    }

    /// Get all currently stored messages for a specific platform
    pub(crate) async fn list(
        cache: &cache::Handle,
        platform: &Platform,
    ) -> Option<Vec<(Platform, Vec<String>)>> {
        // ZRANGE aussiebot_aussiegg_log_list_YOUTUBE 0 -1 WITHSCORES
        let list_keys = Self::get_keys(platform);

        if list_keys.is_empty() {
            return None;
        }

        let futures = list_keys
            .iter()
            .map(|key| Cache::Zrange(key.1.to_owned().into(), 0, -1).exec(cache));

        let res = futures_util::future::join_all(futures).await;

        let platform_logs = Vec::from_iter(res.into_iter().enumerate().filter_map(
            |(i, opt_resp)| match opt_resp {
                Ok(RespType::VecString(list)) => Some((list_keys[i].0, list)),
                Ok(_) => unreachable!(),
                Err(e) => {
                    tracing::error!("{}", e);
                    None
                }
            },
        ));
        //.collect();

        Some(platform_logs)
    }

    pub(crate) fn init(
        &self,
        cancel_chan: watch::Receiver<()>,
        cache: &cache::Handle,
        //resp: &mpsc::Sender<(Location, Response)>,
    ) -> Option<()> {
        let keep_for = self.keep_for as u64;
        let platforms = self.platforms;

        let platform_list: Vec<Platform> = CHAT_PLATFORMS
            .into_iter()
            .filter(|p| platforms.contains(*p))
            .collect();

        //let cancel_chan1 = cancel_chan.clone();
        let cache = cache.clone();

        tracing::info!(
            "\x1b[93mSpawning Log cleanup task with interval: {}s\x1b[0m",
            keep_for
        );

        // spawn task to clear messages older than keep_of (task interval keepof?)
        tokio::task::spawn(
            async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(keep_for)).await;
                    match cancel_chan.has_changed() {
                        Ok(false) => {}
                        _ => {
                            // value changed or channel closed
                            tracing::info!("\x1b[93maborting\x1b[0m");
                            return;
                        }
                    }

                    futures_util::future::join_all(
                        platform_list
                            .iter()
                            .map(|platform| Self::cleanup(platform, keep_for, &cache)),
                    )
                    .await;

                    tracing::info!(
                        platforms = %platforms,
                        keep_for = %keep_for,
                        "ran",
                    );
                }
            }
            .instrument(debug_span!("Log cleanup task")),
        );

        Some(())
    }

    /// Log mod actions
    pub(crate) fn mod_action(
        db: db::Handle,
        platform: Platform,
        id: Arc<String>,
        action: ModAction,
        reason: Arc<String>,
    ) {
        tracing::info!(action = %action, reason = %reason,"\x1b[33mlogging\x1b[0m");
        tokio::spawn(async move { Db::ModAction(platform, id, action, reason).exec(&db).await });
    }

    /// Get all currently stored messages for a specific platform
    pub(crate) async fn list_mod_actions(db: &db::Handle) -> error::Result<ModActionDump> {
        let res = Db::DumpModActions.exec(db).await?;
        match res {
            Resp::ModActionDump(dump) => Ok(dump),
            _ => unreachable!(),
        }
    }
}
