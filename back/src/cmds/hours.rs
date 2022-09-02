use super::{util, Context, RunRes};
use crate::{
    db::{hours::HoursOp, Db, Resp},
    error,
    msg::{Chat, Invocation, Location, Payload, Permissions, Platform, Response},
};
use back_derive::command;

struct Args {
    user_asked: bool,
}

#[command(locks(rate, update_rate))]
/// Accumulate and check watch time
pub struct Hours {
    /// Command prefix
    #[cmd(def("!hours"), constr(non_empty))]
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
    /// Cooldown for adding points
    #[cmd(constr(pos))]
    ratelimit_update: u64,
    /// Max. duration between messages (in seconds)
    #[cmd(defl("60*60*2"), constr(pos))]
    max_diff: i64,
}

impl Hours {
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
        if !self.enabled {
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

        let autocorrect = self.parse_arguments(chat);

        let args = Args {
            user_asked: match autocorrect {
                None => false,
                Some(false) => true,
                Some(true) => false, // TODO: autocorrect
            },
        };

        self.run(ctx, args).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        ctx: &Context<'_>,
        invocation: &Invocation,
    ) -> Option<RunRes> {
        super::check_invoke_prefix(&self.prefix, &invocation.cmd)?;

        let args = Args { user_asked: true };

        match self.run(ctx, args).await {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all, name = "Hours")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        if !self.enabled || !self.platforms.contains(ctx.platform) {
            return Ok(RunRes::Disabled);
        }

        let user_asked = args.user_asked;
        let user = ctx.user;
        let platform = ctx.platform;

        // Different ratelimits for hours updating and Hours as a cmd
        if user_asked {
            // check perms
            if ctx.user.perms < self.perms {
                return Ok(RunRes::InsufficientPerms);
            }
            // if Hours was invoked as a command then do the usual checks
            if util::ratelimit_user(
                ctx,
                self.ratelimit_user,
                stringify!(Hours),
                &self.name,
                &*HOURS_LOCK_RATE,
            )
            .await?
            {
                return Ok(RunRes::Ratelimited { global: false });
            }
        } else if self.ratelimit_update > 0 {
            // do custom ratelimiting for hours tracking
            let cooldown = self.ratelimit_update as u64;
            let user_ratelimit_key = format!("{}_{}", &*HOURS_LOCK_UPDATE_RATE, user.id);

            if !ctx.lock.lock(&user_ratelimit_key, cooldown).await? {
                tracing::info!("\x1b[33mHours update rate-limited locally\x1b[0m");
                return Ok(RunRes::Ratelimited { global: false });
            }
        }

        // update hours
        let resp = Db::Hours(HoursOp {
            platform,
            id: user.id.clone(),
            max_diff: self.max_diff,
        })
        .exec(ctx.db)
        .await?;

        let new_watchtime = match resp {
            Resp::Hours(watchtime) => watchtime,
            _ => unreachable!(),
        };

        tracing::info!(watch_time = new_watchtime);

        if user_asked {
            // send reply
            let new_watchtime = new_watchtime as u64;

            let hours = new_watchtime / 3600;
            let minutes = (new_watchtime - (hours * 3600)) / 60;

            let msg = format!(
                "{} hour{} {} minute{}",
                hours,
                if hours != 1 { "s" } else { "" },
                minutes,
                if minutes != 1 { "s" } else { "" },
            );
            tracing::debug!("{}", &msg);

            // send reply
            Response {
                platform,
                channel: &*crate::CHANNEL_NAME,
                payload: Payload::Message {
                    user: Some((platform, user.clone())),
                    msg: msg.into(),
                    meta: ctx.meta.clone(),
                },
            }
            .send(Location::Pubsub, ctx.resp)
            .await;

            return Ok(RunRes::Ok);
        }

        Ok(RunRes::Noop)
    }
}
