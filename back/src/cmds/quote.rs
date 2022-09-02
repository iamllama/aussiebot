use back_derive::command;

use super::{util, Context, RunRes};
use crate::{
    error,
    msg::{Chat, Invocation, Location, Payload, Permissions, Platform, Response},
};

#[command(locks(rate))]
/// Quote something
pub struct Quote {
    /// Command prefix
    #[cmd(def("!quote"), constr(non_empty))]
    prefix: String,
    /// Autocorrect prefix
    autocorrect: bool,
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Permissions
    #[cmd(defl("Permissions::NONE"))]
    perms: Permissions,
    /// Cooldown per user (in seconds)
    #[cmd(constr(pos))]
    ratelimit_user: u64,
    /// Cooldown per use (in seconds)
    #[cmd(constr(pos))]
    ratelimit: u64,
    /// Message
    #[cmd(def("<placeholder text - change me>"), constr(range = "1..=500"))]
    message: String,
    /// Broadcast to all chat platforms
    broadcast: bool,
    /// Mention caller
    #[cmd(def(true))]
    mention_caller: bool,
}

impl Quote {
    fn parse_arguments(&self, chat: &Chat) -> Option<bool> {
        let captures = util::PREFIX_REGEX.captures(&chat.msg)?;

        // check command prefix
        let autocorrect = util::check_autocorrect(
            &self.prefix,
            &captures[1],
            self.autocorrect,
            &self.levenshtein,
        )?;

        Some(autocorrect)
    }

    fn can_run(&self, ctx: &Context<'_>) -> Option<()> {
        if !self.enabled || self.message.is_empty() {
            return None;
        }

        // check if platform is applicable
        if !self.platforms.contains(ctx.platform) {
            return None;
        }

        // check perms
        if ctx.user.perms < self.perms {
            return None;
        }

        Some(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn chat(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        if self.can_run(ctx).is_none() {
            return Ok(RunRes::Disabled);
        }

        let autocorrect = match self.parse_arguments(chat) {
            Some(t) => t,
            None => return Ok(RunRes::Noop),
        };

        if autocorrect {
            return Ok(RunRes::Autocorrect(self.prefix.clone()));
        }

        match util::ratelimit_global(
            ctx,
            self.ratelimit,
            self.ratelimit_user,
            stringify!(Quote),
            &self.name,
            &*QUOTE_LOCK_RATE,
        )
        .await
        {
            Ok(false) => {}
            Ok(true) => return Ok(RunRes::Ratelimited { global: true }),
            Err(e) => return Err(e),
        }

        self.run(ctx).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        ctx: &Context<'_>,
        invocation: &Invocation,
    ) -> Option<RunRes> {
        self.can_run(ctx)?;

        super::check_invoke_prefix(&self.prefix, &invocation.cmd)?;

        match util::ratelimit_global(
            ctx,
            self.ratelimit,
            self.ratelimit_user,
            stringify!(Quote),
            &self.name,
            &*QUOTE_LOCK_RATE,
        )
        .await
        {
            Ok(false) => {}
            Ok(true) => return None,
            Err(e) => {
                tracing::error!("{}", e);
                return None;
            }
        }

        match self.run(ctx).await {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all, name = "Quote")]
    async fn run(&self, ctx: &Context<'_>) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str());

        let platform = if !self.broadcast {
            ctx.platform
        } else {
            Platform::CHAT
        };

        let user = if self.mention_caller {
            Some((ctx.platform, ctx.user.clone()))
        } else {
            None
        };

        Response {
            platform,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Message {
                user,
                msg: self.message.to_owned().into(),
                meta: ctx.meta.clone(),
            },
        }
        .send(Location::Broadcast, ctx.resp)
        .await;

        Ok(RunRes::Ok)
    }
}
