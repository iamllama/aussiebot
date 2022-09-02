use super::{Context, FilterCache, ModAction, RunRes};
use crate::{
    cache::{Cache, RespType},
    error,
    msg::{Chat, Invocation, Permissions, Platform},
};
use back_derive::command;
use std::sync::Arc;

/*
edit distance compares 2 strings

config defines min edit distance and min consecutive trip count

we only need to store:
  last msg
  no. of consecutive times within threshold
*/

#[command(filter, locks(lock, prev_msg, count))]
/// Filter consecutive similar chat messages from the same user
pub struct Levenshtein {
    /// Apply to anyone below permission level
    #[cmd(defl("Permissions::NONE"))]
    apply_to: Permissions,
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Mod action
    #[cmd(defl("ModAction::None"), constr(range = "1..=86400"))]
    action: ModAction,
    /// Minimum allowable message similarity (0 means identical)
    #[cmd(constr(pos))]
    min_dist: u64,
    /// Mininum number of consecutive trips
    #[cmd(constr(pos))]
    min_times: u64,
    /// Burst rate (in seconds)
    #[cmd(constr(pos))]
    burst_rate: u64,
}

impl Levenshtein {
    fn can_run(&self, ctx: &Context<'_>) -> Option<()> {
        if !self.enabled {
            return None;
        }

        // check if platform is applicable
        if !self.platforms.contains(ctx.platform) {
            return None;
        }

        // check perms
        if ctx.user.perms > self.apply_to {
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

    #[tracing::instrument(level = "trace", skip_all, name = "Levenshtein")]
    async fn run(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        // fill filter cache if empty
        super::Filter::fill_cache(ctx, chat);

        let filter_cache = match ctx.filter_cache.read().clone() {
            Some(c) => c,
            None => return Ok(RunRes::Noop),
        };
        let lock_name = format!("{}_{}_{}", &*LEVENSHTEIN_LOCK_LOCK, self.name, ctx.user.id);

        ctx.lock.lock(&lock_name, 5).await?;
        let action = self.inner(ctx, chat, filter_cache).await;
        ctx.lock.unlock(&lock_name).await?;

        Ok(action.map_or(RunRes::Ok, RunRes::Filtered))
    }

    async fn inner(
        &self,
        ctx: &Context<'_>,
        chat: &Chat,
        filter_cache: FilterCache,
    ) -> Option<ModAction> {
        let burst_rate = self.burst_rate as usize;

        // fetch-swap the prev msg with the current one
        let msg_key = format!(
            "{}_{}_{}",
            &*LEVENSHTEIN_LOCK_PREV_MSG, self.name, chat.user.id
        );
        let prev_msg =
            match Cache::SetGet(msg_key.into(), Arc::clone(&filter_cache.msg), burst_rate)
                .exec(ctx.cache)
                .await
            {
                Ok(RespType::String(prev_msg)) => prev_msg,
                _ => return None,
            };
        tracing::debug!("prev: {}, curr: {}", &prev_msg, filter_cache.msg);

        // compute edit distance between prev_msg and chat.msg
        let edit_dist =
            Self::edit_distance(prev_msg, &*filter_cache.msg).min(i64::MAX as usize) as u64;
        tracing::debug!("edit dist: {}", edit_dist);

        let count_key = format!(
            "{}_{}_{}",
            &*LEVENSHTEIN_LOCK_COUNT, self.name, chat.user.id
        );
        let count_key = Arc::new(count_key);

        // check if edit distance is under threshold
        if edit_dist < self.min_dist {
            // streak started or sustained, increment trip count
            let trip_count = match Cache::Increment(count_key.clone(), 1, burst_rate)
                .exec(ctx.cache)
                .await
            {
                Ok(RespType::U64(count)) => count,
                _ => return None,
            };

            tracing::debug!(
                "\x1b[91m{}'s edit distance is {} (<{}), trip count: {} (<{})\x1b[0m",
                chat.user.name,
                edit_dist,
                self.min_dist,
                trip_count,
                self.min_times
            );

            // check trip count
            if trip_count > self.min_times {
                tracing::info!(
                    "\x1b[91m{}'s edit distance <{} for >{} times\x1b[0m",
                    chat.user.name,
                    self.min_dist,
                    self.min_times
                );
                // reset count
                if let Err(e) = Cache::Delete(count_key).exec(ctx.cache).await {
                    tracing::error!("{}", e);
                }
                return Some(self.action);
            }
        } else {
            // streak broken, reset count
            if let Err(e) = Cache::Delete(count_key).exec(ctx.cache).await {
                tracing::error!("{}", e);
            }
        }
        None
    }

    fn edit_distance(a: impl AsRef<str>, b: impl AsRef<str>) -> usize {
        levenshtein::levenshtein(a.as_ref(), b.as_ref())
    }
}
