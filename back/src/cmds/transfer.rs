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
use std::str::FromStr;

static TRANSFER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)\s(\d+|all)\sfrom\s(\S+)\sto\s(\S+)\s*").unwrap());

#[derive(Debug)]
struct Args {
    amount: i32,
    from: Platform,
    to: Platform,
}

#[command(locks(rate))]
/// Transfer points between platforms
pub struct Transfer {
    /// Command prefix
    #[cmd(def("!transfer"), constr(non_empty))]
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
    #[cmd(def(10i64), constr(pos))]
    min_amount: i64,
    /// Max amount
    #[cmd(def(10_000i64), constr(pos))]
    max_amount: i64,
}

impl Transfer {
    fn parse_arguments(&self, chat: &Chat) -> error::Result<Option<(bool, Args)>> {
        let captures = match TRANSFER_REGEX.captures(&chat.msg) {
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

        // parse and validate amount
        let amount = if &captures[2] == "all" {
            -1
        } else {
            captures[2].parse::<i32>()?
        };

        let from = Platform::from_str(&captures[3]).unwrap();
        let to = Platform::from_str(&captures[4]).unwrap();

        Ok(Some((autocorrect, Args { amount, from, to })))
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

        let (autocorrect, args) = match self.parse_arguments(chat)? {
            Some(t) => t,
            None => return Ok(RunRes::Noop),
        };

        if autocorrect {
            return Ok(RunRes::Autocorrect(self.prefix.clone()));
        }

        match util::ratelimit_user(
            ctx,
            self.ratelimit_user,
            stringify!(Transfer),
            &self.name,
            &*TRANSFER_LOCK_RATE,
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
            stringify!(Transfer),
            &self.name,
            &*TRANSFER_LOCK_RATE,
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

    #[tracing::instrument(level = "trace", skip_all, name = "Transfer")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str(), args = ?args);

        let op = GiveOp {
            amount: args.amount,
            from: GiveSource::Linked(ctx.platform, args.from, ctx.user.id.clone()),
            to: GiveTarget::Linked(args.to),
            min: self.min_amount,
            max: self.max_amount,
        };

        // exec op
        match Db::Give(op).exec(ctx.db).await? {
            Resp::Give(amount) => {
                // send reply
                let msg = format!(
                    "transferred {} point{} from {} to {}",
                    amount,
                    if args.amount != 1 { "s" } else { "" },
                    args.from,
                    args.to
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
                .send(Location::Pubsub, ctx.resp)
                .await;

                Ok(RunRes::Ok)
            }
            _ => unreachable!(),
        }
    }
}

impl Invokable for Transfer {
    fn args(&self, _platform: Platform) -> Vec<Arg> {
        vec![
            Arg {
                name: "from".into(),
                desc: "Platform to transfer from".into(),
                kind: ArgKind::Platform,
                optional: false,
            },
            Arg {
                name: "to".into(),
                desc: "Platform to transfer to".into(),
                kind: ArgKind::Platform,
                optional: false,
            },
            Arg {
                name: "amount".into(),
                desc: "Amount to transfer (leaving this blank means max)".into(),
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
            _ => -1,
        };

        let from = match value.get("from") {
            Some(ArgValue::String(p)) => Platform::from_str(p),
            _ => return Err(ArgMapError.into()),
        }?;

        let to = match value.get("to") {
            Some(ArgValue::String(p)) => Platform::from_str(p),
            _ => return Err(ArgMapError.into()),
        }?;

        Ok(Args { amount, from, to })
    }
}
