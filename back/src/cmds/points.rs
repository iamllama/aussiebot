use super::{util, Context, RunRes};
use crate::{
    db::{self, Db},
    error,
    msg::{Chat, ChatMeta, Invocation, Location, Payload, Permissions, Platform, Response},
};
use back_derive::command;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt::Write as _;

static CHAT_DONO_AMT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{amount\}").unwrap());

struct Args {
    user_asked: bool,
}

#[command(locks(rate, update_rate))]
/// Accumulate and check points
pub struct Points {
    /// Command prefix
    #[cmd(def("!points"), constr(non_empty))]
    prefix: String,
    /// Autocorrect prefix
    autocorrect: bool,
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Permissions
    #[cmd(defl("Permissions::NONE"))]
    perms: Permissions,
    /// Points awarded per chat message
    #[cmd(def(5_u64), constr(pos))]
    points: u64,
    /// Message to send in response to donations
    dono_msg: String,
    /// Cooldown per user (in seconds)
    #[cmd(constr(pos))]
    ratelimit_user: u64,
    /// Cooldown for adding points
    #[cmd(constr(pos))]
    ratelimit_update: u64,
}

impl Points {
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

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn chat(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        let autocorrect = self.parse_arguments(chat);

        let args = Args {
            user_asked: match autocorrect {
                None => false,
                Some(false) => true,
                Some(true) => false,
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

    /// Send donation reply
    async fn handle_dono(&self, ctx: &Context<'_>, amount: &str) -> error::Result<()> {
        // replace amount and name vars
        // escape chars on amount and name to avoid regex operators
        // escape_debug doesn't work, it escapes whitespace too, but not $
        let rep = CHAT_DONO_AMT_REGEX.replace_all(self.dono_msg.as_ref(), amount);

        // send reply
        Response {
            platform: ctx.platform,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Message {
                user: Some((ctx.platform, ctx.user.clone())),
                msg: rep.into_owned().into(),
                meta: ctx.meta.clone(),
            },
        }
        .send(Location::Broadcast, ctx.resp)
        .await;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip_all, name = "Points")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        if !self.enabled || !self.platforms.contains(ctx.platform) {
            return Ok(RunRes::Disabled);
        }

        let user_asked = args.user_asked;
        let user = ctx.user;
        let platform = ctx.platform;

        // Different ratelimits for hours updating and Points as a cmd
        if user_asked {
            // check perms
            if ctx.user.perms < self.perms {
                return Ok(RunRes::InsufficientPerms);
            }
            // if Points was invoked as a command then do the usual checks

            if util::ratelimit_user(
                ctx,
                self.ratelimit_user,
                stringify!(Points),
                &self.name,
                &*POINTS_LOCK_RATE,
            )
            .await?
            {
                return Ok(RunRes::Ratelimited { global: false });
            }
        } else if self.ratelimit_update > 0 {
            // do custom ratelimiting for points tracking
            let cooldown = self.ratelimit_update as u64;
            let user_ratelimit_key = format!("{}_{}", &*POINTS_LOCK_UPDATE_RATE, &user.id);

            if !ctx.lock.lock(user_ratelimit_key, cooldown).await? {
                tracing::info!("\x1b[33mPoints update rate-limited locally\x1b[0m");
                return Ok(RunRes::Ratelimited { global: false });
            }
        }

        // increment points if applicable
        if self.points > 0 {
            let resp = Db::Upsert(
                ctx.platform,
                user.id.clone(),
                user.name.clone(),
                self.points as i32,
            )
            .exec(ctx.db)
            .await?;
            assert!(matches!(resp, db::Resp::Ok));
        }

        if user_asked {
            let resp = Db::GetPoints(ctx.platform, user.id.clone())
                .exec(ctx.db)
                .await?;

            let points_list = match resp {
                db::Resp::GetPoints(l) => l,
                _ => unreachable!(),
            };

            let mut msg = String::new();

            for (platform, points) in &points_list {
                if let Some(points) = points {
                    write!(msg, "{} ({}), ", points, platform).unwrap();
                }
            }

            if !points_list.is_empty() {
                msg.truncate(msg.chars().count() - 2);
            }

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
        } else {
            // TOOD: move this somewhere else
            // send dono message if applicable
            if let Some(ChatMeta::Youtube(amount)) = &ctx.meta {
                if !self.dono_msg.is_empty() {
                    self.handle_dono(ctx, amount).await?;
                }
            }
        }

        Ok(RunRes::Noop)
    }
}
