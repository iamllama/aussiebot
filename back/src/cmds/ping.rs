use super::{util, Arg, ArgKind, ArgValue, CmdDesc, Context, Invokable, RunRes};
use crate::{
    error,
    msg::{
        self, ArgMap, ArgMapError, Chat, Invocation, Location, Payload, Permissions, Platform,
        Response, User,
    },
};
use back_derive::command;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;

#[derive(Debug)]
struct Args {
    msg: Option<String>,
}

#[command(locks(rate))]
/// Ping someone on another platform
pub struct Ping {
    /// Command prefix
    #[cmd(def("!ping"), constr(non_empty))]
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
    /// Target platform (choose one)
    #[cmd(defl("Platform::DISCORD"))]
    pingee_platform: Platform,
    /// Target ID (Discord id etc.)
    pingee_id: String,
    /// Target name (Youtube name etc.)
    pingee_name: String,
}

static PING_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)(?:\s(.{1,200}))?").unwrap());

impl Ping {
    fn parse_arguments(&self, chat: &Chat) -> Option<(bool, Args)> {
        let captures = PING_REGEX.captures(&chat.msg)?;

        // check command prefix
        let autocorrect = util::check_autocorrect(
            &self.prefix,
            &captures[1],
            self.autocorrect,
            &self.levenshtein,
        )?;

        let msg = captures.get(2).map(|m| m.as_str().to_owned());

        Some((autocorrect, Args { msg }))
    }

    fn can_run(&self, ctx: &Context<'_>) -> Option<()> {
        if !self.enabled {
            return None;
        }

        match self.pingee_platform {
            Platform::YOUTUBE if !self.pingee_name.is_empty() => {}
            Platform::TWITCH if !self.pingee_name.is_empty() => {}
            Platform::DISCORD if !self.pingee_id.is_empty() => {}
            _ => return None,
        };

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

        let (autocorrect, args) = match self.parse_arguments(chat) {
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
            stringify!(Ping),
            &self.name,
            &*PING_LOCK_RATE,
        )
        .await
        {
            Ok(false) => {}
            Ok(true) => return Ok(RunRes::Ratelimited { global: true }),
            Err(e) => return Err(e),
        }

        self.run(ctx, args).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        ctx: &Context<'_>,
        invocation: &Invocation,
    ) -> Option<RunRes> {
        self.can_run(ctx)?;

        super::check_invoke_prefix(&self.prefix, &invocation.cmd)?;

        let args = Args::try_from(&invocation.args).ok()?;

        match util::ratelimit_global(
            ctx,
            self.ratelimit,
            self.ratelimit_user,
            stringify!(Ping),
            &self.name,
            &*PING_LOCK_RATE,
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

        match self.run(ctx, args).await {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all, name = "Ping")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str(), args = ?args);

        Response {
            platform: self.pingee_platform,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Ping(msg::Ping {
                pinger: Some((ctx.platform, ctx.user.clone())),
                pingee: Arc::new(User {
                    id: Arc::new(self.pingee_id.to_owned()),
                    name: Arc::new(self.pingee_name.to_owned()),
                    perms: Permissions::NONE,
                }),
                msg: args.msg.map(Arc::new),
                meta: ctx.meta.clone(),
            }),
        }
        .send(Location::Broadcast, ctx.resp)
        .await;

        Ok(RunRes::Ok)
    }
}

impl CmdDesc for Ping {
    #[inline]
    fn platform(&self) -> Platform {
        self.platforms
    }

    #[inline]
    fn description(&self, platform: Platform) -> Option<String> {
        if self.pingee_name.is_empty() {
            return None;
        }
        match platform {
            Platform::DISCORD if self.pingee_platform == Platform::DISCORD => {
                Some(format!("Ping {}", self.pingee_name))
            }
            _ if self.pingee_platform == platform => Some(format!("Ping {}", self.pingee_name)),
            _ => Some(format!(
                "Ping {} on {}",
                self.pingee_name, self.pingee_platform
            )),
        }
    }
}

impl Invokable for Ping {
    //fn args<'a>() -> &'a [Arg] {
    fn args(&self, _platform: Platform) -> Vec<Arg> {
        vec![Arg {
            name: "message".into(),
            desc: "Message to send (if any)".into(),
            kind: ArgKind::String,
            optional: true,
        }]
    }

    fn hidden(&self, _platform: Platform) -> bool {
        true
    }
}

impl TryFrom<&ArgMap> for Args {
    type Error = ArgMapError;

    fn try_from(value: &ArgMap) -> Result<Self, Self::Error> {
        let msg = match value.get("message") {
            Some(ArgValue::String(msg)) => Some(msg.to_owned()),
            _ => None,
        };

        Ok(Args { msg })
    }
}
