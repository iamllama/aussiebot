use super::{util, Arg, ArgKind, Context, Invokable, RunRes};
use crate::{
    cache::{Cache, RespType},
    db::{link::LinkOp, Db, Resp},
    error::{self, Error},
    msg::{
        ArgMap, Chat, Invocation, Location, Payload, Permissions, Ping, Platform, Response, User,
    },
};
use back_derive::command;
use bb8_redis::redis;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use std::sync::Arc;

static LINK_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)(?:\s([[:xdigit:]]{4}-[[:xdigit:]]{4}))?\s*").unwrap());

#[derive(Debug)]
struct Args {
    code: Option<String>,
}

#[derive(Debug)]
pub enum LinkError {
    InvalidCode,
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

#[command(locks(rate, otp))]
/// Link Youtube and Twitch to Discord
pub struct Link {
    /// Command prefix
    #[cmd(def("!link"), constr(non_empty))]
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
    /// Duration before code expires (in seconds)
    #[cmd(def(30_u64), constr(range = "10..=600"))]
    expiry: u64,
}

/// yt || twitch:
/// user: !link <DISCORD_ID>
/// discord:
/// bot: If you requested this, type !code <OTP> in yt || twitch
///
/// ----------------- or -----------------
///
/// yt || twitch:
/// user: !link
/// bot: DM Aussiebot on Discord with `!link`
/// discord DMS:
/// user: !link
/// aussiebot_otp_<OTP> = <DISCORD_ID>
/// bot: type !link <OTP> in yt || twitch to link
/// yt || twitch:
/// user: !link <OTP>:
/// (<DISCORD_ID>, <PLATFORM_ID>) = aussiebot_otp_<OTP>
/// req and keys' PLATFORM_IDs match => link
///
impl Link {
    fn parse_arguments(&self, chat: &Chat) -> Option<(bool, Args)> {
        let captures = LINK_REGEX.captures(&chat.msg)?;

        // check command prefix
        let autocorrect = util::check_autocorrect(
            &self.prefix,
            &captures[1],
            self.autocorrect,
            &self.levenshtein,
        )?;

        let code = captures.get(2).map(|m| m.as_str().to_owned());

        Some((autocorrect, Args { code }))
    }

    fn can_run(&self, ctx: &Context<'_>) -> Option<()> {
        if !self.enabled {
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

        let (autocorrect, args) = match self.parse_arguments(chat) {
            Some(t) => t,
            None => return Ok(RunRes::Noop),
        };

        if autocorrect {
            return Ok(RunRes::Autocorrect(self.prefix.clone()));
        }

        match util::ratelimit_user(
            ctx,
            self.ratelimit_user,
            stringify!(Link),
            &self.name,
            &*LINK_LOCK_RATE,
        )
        .await
        {
            Ok(false) => {}
            Ok(true) => return Ok(RunRes::Ratelimited { global: false }),
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

        match util::ratelimit_user(
            ctx,
            self.ratelimit_user,
            stringify!(Link),
            &self.name,
            &*LINK_LOCK_RATE,
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

        //TODO: inform on failure
    }

    #[tracing::instrument(level = "trace", skip_all, name = "Link")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str(), args = ?args);

        let from_discord = ctx.platform.contains(Platform::DISCORD);

        match (from_discord, args.code) {
            (false, None) => {
                /* yt: !link, tell user to dm !link on discord */
                let msg = "DM Aussiebot with or type \"!link\" in the discord server".to_owned();
                Response {
                    platform: ctx.platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Message {
                        user: Some((ctx.platform, ctx.user.clone())),
                        msg: msg.into(),
                        meta: ctx.meta.clone(),
                    },
                }
                .send(Location::Broadcast, ctx.resp)
                .await;
            }
            (true, None) => {
                // generate OTP
                let otp_code = self.handle_gen_otp(ctx).await?;
                // send reply with code
                let msg = format!("Type `!link {}` within {} sec(s) in the stream's live chat to link that account with your discord",otp_code, self.expiry);
                Response {
                    platform: ctx.platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(Ping {
                        pinger: None,
                        pingee: ctx.user.clone(),
                        msg: Some(msg.into()),
                        meta: ctx.meta.clone(),
                    }),
                }
                .send(Location::Broadcast, ctx.resp)
                .await;
            }
            (false, Some(code)) => {
                // check OTP, upsert link if valid
                let discord_id = self.handle_recv_otp(ctx, code).await?;
                // send success dm
                let msg = "Successfully linked!".to_string();
                Response {
                    platform: Platform::DISCORD,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(Ping {
                        pinger: Some((ctx.platform, ctx.user.clone())),
                        pingee: Arc::new(User {
                            id: discord_id,
                            name: "".to_owned().into(),
                            perms: Permissions::NONE,
                        }),
                        msg: Some(msg.into()),
                        meta: ctx.meta.clone(),
                    }),
                }
                .send(Location::Broadcast, ctx.resp)
                .await;
            }
            (true, _) => { /* discord: !link <code>, ignore for now */ }
        }

        Ok(RunRes::Ok)
    }

    async fn handle_gen_otp(&self, ctx: &Context<'_>) -> error::Result<String> {
        const MAX_RETRY: usize = 10;

        let expiry = self.expiry as usize;

        for _ in 0..MAX_RETRY {
            let otp_code1 = rand::thread_rng().gen::<u16>();
            let otp_code2 = rand::thread_rng().gen::<u16>();
            let otp_code = format!("{:04X}-{:04X}", otp_code1, otp_code2);

            let otp_key = Arc::new(format!("{}_{}", &*LINK_LOCK_OTP, otp_code));

            // try to exclusively set, retry on failure (key already exists)
            tracing::debug!("trying to set {} to {}", otp_key, ctx.user.id);

            let resp = Cache::Set(otp_key.clone(), ctx.user.id.clone(), expiry, true)
                .exec(ctx.cache)
                .await;

            match resp {
                Ok(RespType::Bool(true)) => {}
                Ok(RespType::Bool(false)) => {
                    tracing::error!("error setting {} to {}", otp_key, ctx.user.id);
                    continue;
                }
                Ok(_) => unreachable!(),
                Err(e) => {
                    tracing::error!("{}", e);
                    continue;
                }
            }

            tracing::debug!("set");

            return Ok(otp_code);
        }

        tracing::warn!("failed to set otp code after {} tries", MAX_RETRY);

        // failed to gen a unique otp
        Err("failed to generate unique otp".into())
    }

    async fn handle_recv_otp(
        &self,
        ctx: &Context<'_>,
        otp_code: String,
    ) -> error::Result<Arc<String>> {
        let otp_key = Arc::new(format!("{}_{}", &*LINK_LOCK_OTP, otp_code));

        // take code if it exists
        let resp = Cache::GetDel(otp_key.clone()).exec(ctx.cache).await;
        let discord_id = match resp {
            Ok(RespType::String(s)) if !s.is_empty() => s,
            Ok(_) => unreachable!(),
            Err(Error::Redis(e)) if e.kind() == redis::ErrorKind::TypeError => {
                return Err(LinkError::InvalidCode.into())
            }
            Err(e) => return Err(e),
        };
        let discord_id = Arc::new(discord_id);

        // upsert link
        let resp = Db::Link(LinkOp {
            platform: ctx.platform,
            discord_id: discord_id.clone(),
            platform_id: ctx.user.id.clone(),
        })
        .exec(ctx.db)
        .await?;

        assert!(matches!(resp, Resp::Ok));

        tracing::info!("successful");
        Ok(discord_id)
    }
}

impl Invokable for Link {
    //fn args<'a>() -> &'a [Arg] {
    fn args(&self, platform: Platform) -> Vec<Arg> {
        match platform {
            Platform::DISCORD => vec![],
            _ => vec![Arg {
                name: "code".into(),
                desc: "Code (if any, leave blank if on Discord)".into(),
                kind: ArgKind::String,
                optional: true,
            }],
        }
    }

    fn hidden(&self, _platform: Platform) -> bool {
        true
    }
}

impl TryFrom<&ArgMap> for Args {
    type Error = error::Error;

    fn try_from(_value: &ArgMap) -> Result<Self, Self::Error> {
        Ok(Args { code: None })
    }
}
