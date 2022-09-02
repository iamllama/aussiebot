use super::{util, Arg, ArgKind, ArgValue, Context, Invokable, RunRes};
use crate::db::give::{GiveSource, GiveTarget};
use crate::db::Resp;
use crate::db::{give::GiveOp, Db};
use crate::error;
use crate::msg::{
    ArgMap, ArgMapError, Chat, Invocation, Location, Payload, Permissions, Platform, Response,
};
use back_derive::command;
use once_cell::sync::Lazy;
use regex::Regex;

static GIVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s@?(.+)\s(\d+|all)\s*").unwrap());

#[derive(Debug)]
struct Args {
    amount: i32,
    to: GiveTarget,
}

#[command(locks(rate))]
/// Give and receive points
pub struct Give {
    /// Command prefix
    #[cmd(def("!give"), constr(non_empty))]
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
    /// Min amount
    #[cmd(def(10_i64), constr(pos))]
    min_amount: i64,
    /// Max amount
    #[cmd(def(10_000_i64), constr(pos))]
    max_amount: i64,
}

impl Give {
    fn parse_arguments(
        &self,
        ctx: &Context<'_>,
        chat: &Chat,
    ) -> error::Result<Option<(bool, Args)>> {
        let captures = match GIVE_REGEX.captures(&chat.msg) {
            Some(cap) => cap,
            None => return Ok(None),
        };

        // check command prefix
        let autocorrect = match util::check_autocorrect(
            &self.prefix,
            &captures[1],
            self.autocorrect,
            &self.levenshtein,
        ) {
            Some(a) => a,
            None => return Ok(None),
        };

        // get dest
        let to = captures[2].to_owned();

        // check if src != dest
        if ctx.user.name.as_str() == to {
            return Ok(None);
        }

        let to = GiveTarget::Name(ctx.platform, to.into());

        // parse and validate wager
        let amount = if &captures[3] == "all" {
            -1
        } else {
            captures[3].parse::<i32>()?
        };

        Ok(Some((autocorrect, Args { amount, to })))
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

        let (autocorrect, args) = match self.parse_arguments(ctx, chat)? {
            Some(t) => t,
            None => return Ok(RunRes::Noop),
        };

        if autocorrect {
            return Ok(RunRes::Autocorrect(self.prefix.clone()));
        }

        match util::ratelimit_user(
            ctx,
            self.ratelimit_user,
            stringify!(Give),
            &self.name,
            &*GIVE_LOCK_RATE,
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

        match args.to {
            GiveTarget::Name(platform, name)
                if platform == ctx.platform && *name == *ctx.user.name =>
            {
                return None
            }
            GiveTarget::User(platform, id, _)
                if platform == ctx.platform && *id == *ctx.user.id =>
            {
                return None
            }
            _ => {}
        }

        match util::ratelimit_user(
            ctx,
            self.ratelimit_user,
            stringify!(Give),
            &self.name,
            &*GIVE_LOCK_RATE,
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

    #[tracing::instrument(level = "trace", skip_all, name = "Give")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str(), args = ?args);

        let to_name = match args.to {
            GiveTarget::Name(_, ref name) => name.clone(),
            GiveTarget::User(_, _, ref name) => name.clone(),
            _ => unreachable!(),
        };

        let op = GiveOp {
            amount: args.amount,
            from: GiveSource::Id(ctx.platform, ctx.user.id.clone()),
            to: args.to, //: GiveTarget::Name(ctx.platform, to.clone()),
            min: self.min_amount,
            max: self.max_amount,
        };

        // exec op
        let resp = Db::Give(op).exec(ctx.db).await?;
        match resp {
            Resp::Give(amount) => {
                // send reply
                let msg = format!(
                    "gave {} {} point{}",
                    to_name,
                    amount,
                    if args.amount != 1 { "s" } else { "" },
                );

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

                Ok(RunRes::Ok)
            }
            _ => unreachable!(),
        }
    }
}

impl Invokable for Give {
    //fn args<'a>() -> &'a [Arg] {
    fn args(&self, _platform: Platform) -> Vec<Arg> {
        vec![
            Arg {
                name: "to".into(),
                desc: "Person to give to".into(),
                kind: ArgKind::User,
                optional: false,
            },
            Arg {
                name: "amount".into(),
                desc: "Amount to give (leaving this blank means max)".into(),
                kind: ArgKind::Integer {
                    min: Some(self.min_amount),
                    max: Some(self.max_amount),
                },
                optional: true,
            },
        ]
    }
}

impl TryFrom<&ArgMap> for Args {
    type Error = error::Error;

    fn try_from(value: &ArgMap) -> Result<Self, Self::Error> {
        let amount = match value.get("amount") {
            Some(ArgValue::Integer(x)) => *x as i32,
            Some(_) => return Err(ArgMapError.into()),
            None => -1,
        };

        let to = match value.get("to") {
            Some(ArgValue::User(u)) => {
                GiveTarget::User(Platform::DISCORD, u.id.clone(), u.name.clone())
                // TODO: dont assume platform
            }
            _ => return Err(ArgMapError.into()),
        };

        Ok(Args { amount, to })
    }
}
