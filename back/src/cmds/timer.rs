use super::{Context, RunRes};
use crate::{
    cache::{self, Cache, RespType},
    error,
    msg::{Chat, Invocation, Location, Payload, Platform, Response},
};
use back_derive::command;
use rand::{distributions::Uniform, prelude::*};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, watch};
use tracing::{info_span, Instrument};

#[command(timer, locks(count))]
/// Send a message at preset intervals
pub struct Timer {
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Repetition interval (in seconds)
    #[cmd(constr(pos))]
    interval: u64,
    /// Max random delay (in seconds)
    #[cmd(constr(pos))]
    jitter: u64,
    /// Message to send
    msg: String,
    /// Min. number of chat messages required (Setting this to 0 will cause messages to be sent regardless of whether anyone's talking in chat, which may not be what you want)
    #[cmd(def(1_u64), constr(pos))]
    msg_count: u64,
}

impl Timer {
    /// Implicit chat fn to increment timer msg count
    #[tracing::instrument(level = "trace", skip_all, name = "Timer")]
    pub(super) async fn chat(&self, ctx: &Context<'_>, _chat: &Chat) -> error::Result<RunRes> {
        if !self.enabled || self.msg_count == 0 {
            // don't count messages for Timers with no msg_count trigger set
            return Ok(RunRes::Disabled);
        }

        // increment associated timer's count
        let count_key = format!("{}_{}", &*TIMER_LOCK_COUNT, self.name);

        Cache::Increment(count_key.into(), 1, 0)
            .exec(ctx.cache)
            .await?;

        Ok(RunRes::Noop)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        _ctx: &Context<'_>,
        _invocation: &Invocation,
    ) -> Option<RunRes> {
        None
    }

    pub(crate) fn init(
        &self,
        cancel_chan: watch::Receiver<()>,
        cache: &cache::Handle,
        resp: &mpsc::Sender<(Location, Response)>,
    ) -> Option<()> {
        if !self.enabled || self.platforms.is_empty() || self.interval == 0 || self.msg.is_empty() {
            return None;
        }

        tracing::info!(
            "\x1b[93mSpawning Timer {:?} with interval: {}s, max jitter: {}s\x1b[0m",
            self.name,
            self.interval,
            self.jitter
        );

        let cache = cache.clone();
        let resp = resp.clone();

        let timer_name = self.name.clone();
        let interval = self.interval as u64;
        let jitter = self.jitter as u64;
        let trigger_count = self.msg_count as u64;
        let platform = self.platforms;
        let msg = Arc::new(self.msg.clone());

        let jitter_dist = Uniform::from(0..=jitter);
        let count_key = Arc::new(format!("{}_{}", &*TIMER_LOCK_COUNT, self.name));

        let zero: Arc<String> = Arc::new("0".into());

        tokio::spawn(
            async move {
                loop {
                    // sleep with random jitter
                    let jitter = jitter_dist.sample(&mut rand::thread_rng());
                    tokio::time::sleep(Duration::from_secs(interval.saturating_add(jitter))).await;

                    match cancel_chan.has_changed() {
                        Ok(false) => {}
                        _ => {
                            // value changed or channel closed
                            tracing::info!(timer_name = %timer_name, "\x1b[93maborting\x1b[0m");
                            return;
                        }
                    }

                    if trigger_count > 0 {
                        // get msg count from cache
                        let count = Cache::SetGet(count_key.clone(), zero.clone(), 0)
                            .exec(&cache)
                            .await;
                        let count: u64 = if let Ok(RespType::String(s)) = count {
                            s.parse().unwrap_or_default()
                        } else {
                            0
                        };
                        // check if enough msgs have been received
                        if count < trigger_count {
                            continue;
                        }

                        tracing::trace!(
                            "\x1b[93m{} msg count: {}, trigger count: {}\x1b[0m",
                            timer_name,
                            count,
                            trigger_count
                        );
                    }

                    // broadcast msg to any applicable chatbot
                    Response {
                        platform,
                        channel: &*crate::CHANNEL_NAME,
                        payload: Payload::Message {
                            user: None,
                            msg: msg.clone(),
                            meta: None,
                        },
                    }
                    .send(Location::Pubsub, &resp)
                    .await;
                }
            }
            .instrument(info_span!("Timer")),
        );

        Some(())
    }
}
