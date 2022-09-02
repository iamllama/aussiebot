use super::{
    util, Arg, ArgKind, ArgValue, CmdDesc, Context, Invokable, ModAction, RespHandle, RunRes,
};
use crate::{
    cache::{self, Cache, RespType},
    db::{
        self,
        give::{GiveOp, GiveSource, GiveTarget},
        Db, Resp,
    },
    error, lock,
    msg::{
        ArgMap, ArgMapError, Chat, Invocation, Location, Payload, Permissions, Platform, Response,
        User,
    },
};
use back_derive::command;
use once_cell::sync::Lazy;
use rand::{distributions::Bernoulli, prelude::Distribution};
use regex::Regex;
use std::fmt::Write as _;
use std::{sync::Arc, time::Duration}; // import without risk of name clashing

static RR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s(\d+|all)\s*").unwrap());

#[derive(Debug)]
struct Args {
    amount: i32,
}

type Heister = (Platform, Arc<User>, i32);
type Handles = (cache::Handle, db::Handle, lock::Handle, RespHandle);

#[command(locks(rate, active, members))]
/// Win big or get timed out/banned (either way, there is no mod abuse 👀)
pub struct RussianRoulette {
    /// Command prefix
    #[cmd(def("!rr"), constr(non_empty))]
    prefix: String,
    /// Autocorrect prefix
    autocorrect: bool,
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Permissions
    #[cmd(defl("Permissions::NONE"))]
    perms: Permissions,
    /// Duration (in seconds)
    #[cmd(def(10u64), constr(pos))]
    duration: u64,
    /// Cooldown per user (in seconds)
    #[cmd(constr(pos))]
    ratelimit_user: u64,
    /// Min amount
    #[cmd(def(10i64), constr(pos))]
    min_amount: i64,
    /// Max amount
    #[cmd(def(100_000i64), constr(pos))]
    max_amount: i64,
    /// % chance of win
    #[cmd(def(33u64), constr(range = "0..=100"))]
    win_prob_pct: u64,
    /// Payoff (x wager)
    #[cmd(def(5u64), constr(pos))]
    payoff: u64,
    /// Penalty on loss
    #[cmd(defl("ModAction::Timeout(300)"), constr(range = "1..=86400"))]
    penalty: ModAction,
}

impl RussianRoulette {
    fn parse_arguments(&self, chat: &Chat) -> error::Result<Option<(bool, Args)>> {
        let captures = match RR_REGEX.captures(&chat.msg) {
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

        // parse and validate wager
        let amount = if &captures[2] == "all" {
            -1
        } else {
            captures[2].parse::<i32>()?
        };

        Ok(Some((autocorrect, Args { amount })))
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
            stringify!(RussianRoulette),
            &self.name,
            &*RUSSIANROULETTE_LOCK_RATE,
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
            stringify!(RussianRoulette),
            &self.name,
            &*RUSSIANROULETTE_LOCK_RATE,
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

    #[tracing::instrument(level = "trace", skip_all, name = "RussianRoulette")]
    async fn run(&self, ctx: &Context<'_>, args: Args) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str(), args = ?args);

        let user = ctx.user;

        // consume amount
        let op = GiveOp {
            amount: args.amount,
            from: GiveSource::Id(ctx.platform, user.id.clone()),
            to: GiveTarget::Spend,
            min: self.min_amount,
            max: self.max_amount,
        };

        let amount = match Db::Give(op).exec(ctx.db).await? {
            Resp::Give(amount) => amount,
            _ => unreachable!(),
        };

        let heister: Heister = (ctx.platform, user.clone(), amount * self.payoff as i32);

        let serialised_heister =
            tokio::task::spawn_blocking(move || serde_json::to_string(&heister)).await??;

        let member_key = Arc::new(format!("{}_{}", &*RUSSIANROULETTE_LOCK_MEMBERS, self.name));

        let active_key = Arc::new(format!("{}_{}", &*RUSSIANROULETTE_LOCK_ACTIVE, self.name));

        // store user and amount in cache
        let resp = Cache::HashSet(
            member_key.clone(),
            user.id.clone(),
            serialised_heister,
            true,
        )
        .exec(ctx.cache)
        .await;

        match resp {
            Ok(RespType::Bool(true)) => {}
            Ok(_) => unreachable!(),
            Err(e) => {
                // TODO: find a way to rollback the db op
                Self::refund(ctx, amount).await?;
                return Err(e);
            }
        }

        // check if heist is currently running
        let starting_heist = ctx.lock.lock(&*active_key, self.duration as u64 + 5).await;

        let starting_heist = match starting_heist {
            Ok(b) => b,
            Err(e) => {
                // TODO: find a way to rollback the db op
                Self::refund(ctx, amount).await?;
                return Err(e);
            }
        };

        let immunity_msg = if user.perms < Permissions::MOD {
            ""
        } else {
            "(immune) "
        };

        let duration = self.duration as u64;
        let penalty = self.penalty;
        let win_prob_pct = self.win_prob_pct as f64 / 100.0;
        let handles = (
            ctx.cache.clone(),
            ctx.db.clone(),
            ctx.lock.clone(),
            ctx.resp.clone(),
        );

        let msg = if starting_heist {
            tokio::spawn(async move {
                if let Err(e) = Self::handle_end(
                    member_key,
                    active_key,
                    duration,
                    penalty,
                    win_prob_pct,
                    handles,
                )
                .await
                {
                    tracing::error!("{}", e);
                }
            });

            format!(
                "{}started a game of russian roulette with the '{}' penalty for {} point{}!",
                immunity_msg,
                self.penalty,
                amount,
                if amount != 1 { "s" } else { "" },
            )
        } else {
            format!(
                "{}joined the russian roulette game with {} point{}!",
                immunity_msg,
                amount,
                if amount != 1 { "s" } else { "" },
            )
        }
        .to_owned();

        tracing::info!("{}", msg);

        Response {
            platform: Platform::CHAT,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Message {
                user: Some((ctx.platform, user.clone())),
                msg: msg.into(),
                meta: ctx.meta.clone(),
            },
        }
        .send(Location::Pubsub, ctx.resp)
        .await;

        Ok(RunRes::Ok)
    }

    async fn refund(ctx: &Context<'_>, amount: i32) -> error::Result<db::Resp> {
        Db::Give(GiveOp {
            amount,
            from: GiveSource::None,
            to: GiveTarget::User(ctx.platform, ctx.user.id.clone(), ctx.user.name.clone()),
            min: 0,
            max: 0,
        })
        .exec(ctx.db)
        .await
    }

    async fn handle_end(
        member_key: Arc<String>,
        active_key: Arc<String>,
        duration: u64,
        penalty: ModAction,
        win_prob: f64,
        (cache, db, lock, resp_handle): Handles,
    ) -> error::Result<()> {
        tokio::time::sleep(Duration::from_secs(duration)).await;

        // get all heisters
        let resp = Cache::HashGetAll(member_key.clone()).exec(&cache).await?;

        let heisters = match resp {
            RespType::VecStringString(survivors) => survivors,
            _ => unreachable!(),
        };

        // decide fates
        let heisters: Vec<((String, String), bool)> = {
            let mut rng = rand::thread_rng();
            let fates = Bernoulli::new(win_prob).unwrap().sample_iter(&mut rng);
            // collect here because rng is !Send
            heisters.into_iter().zip(fates).collect()
        };

        let futures = heisters
            .into_iter()
            .map(|s| Self::handle_heister(s, db.clone(), resp_handle.clone(), penalty));

        let res: Vec<(Arc<String>, i32)> = futures_util::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        let num_survivors = res.len();

        let msg = if num_survivors == 0 {
            "The game is over, there were no survivors monkaW".to_owned()
        } else {
            let mut survivor_msg = "The game is over! Survivors: ".to_owned();
            let penultimate_i = num_survivors.saturating_sub(2);
            let mut res = res.into_iter().enumerate().peekable();
            while let Some((i, (name, amount))) = res.next() {
                // add survivors' names and winnings to reply
                write!(survivor_msg, "{} ({})", name, amount).unwrap();
                if res.peek().is_some() {
                    survivor_msg.push_str(if i != penultimate_i { ", " } else { " and " });
                }
            }
            survivor_msg
        };

        let _ = tokio::join!(lock.unlock(&*member_key), lock.unlock(&*active_key));

        Response {
            platform: Platform::CHAT,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Message {
                user: None,
                msg: msg.into(),
                meta: None,
            },
        }
        .send(Location::Pubsub, &resp_handle)
        .await;

        Ok(())
    }

    #[tracing::instrument(skip(heister, db, resp))]
    async fn handle_heister(
        ((_id, heister), survived): ((String, String), bool),
        db: db::Handle,
        resp: RespHandle,
        action: ModAction,
    ) -> Option<(Arc<String>, i32)> {
        let heister =
            tokio::task::spawn_blocking(move || serde_json::from_str::<Heister>(&heister).unwrap())
                .await
                .ok()?;

        tracing::debug!("heister: {:?} survived: {}", heister, survived);

        let (platform, user, amount) = heister;

        if survived {
            // deposit payoff
            Db::Give(GiveOp {
                amount,
                from: GiveSource::None,
                to: GiveTarget::User(platform, user.id.clone(), user.name.clone()),
                min: 0,
                max: 0,
            })
            .exec(&db)
            .await;

            Some((user.name.clone(), amount))
        } else if user.perms < Permissions::MOD {
            let reason = Arc::new("RussianRoulette".to_owned());
            tracing::info!(action=%action, "\x1b[91menacting penalty\x1b[0m");
            // log mod action
            super::Log::mod_action(db, platform, user.id.clone(), action, reason.clone());
            // enact penalty
            Response {
                platform,
                channel: &*crate::CHANNEL_NAME,
                payload: Payload::ModAction(user, action, reason),
            }
            .send(Location::Broadcast, &resp)
            .await;

            None
        } else {
            // Some((user.name.clone(), 0))
            None
        }
    }
}

impl CmdDesc for RussianRoulette {
    #[inline]
    fn platform(&self) -> Platform {
        self.platforms
    }

    #[inline]
    fn description(&self, platform: Platform) -> Option<String> {
        if platform.contains(Platform::DISCORD) {
            return Some(format!(
                "Win big ({}x) or get the penalty ({})! *either way, there is no mod abuse 👀",
                self.payoff, self.penalty
            ));
        }

        None
    }
}

impl Invokable for RussianRoulette {
    //fn args<'a>() -> &'a [Arg] {
    fn args(&self, _platform: Platform) -> Vec<Arg> {
        vec![Arg {
            name: "amount".into(),
            desc: "Amount to gamble (leaving this blank means max)".into(),
            kind: ArgKind::Integer {
                min: Some(self.min_amount),
                max: Some(self.max_amount),
            },
            optional: true,
        }]
    }
}

impl TryFrom<&ArgMap> for Args {
    type Error = ArgMapError;

    fn try_from(value: &ArgMap) -> Result<Self, Self::Error> {
        let amount = match value.get("amount") {
            Some(ArgValue::Integer(x)) => *x as i32,
            Some(_) => return Err(ArgMapError),
            None => -1,
        };

        Ok(Args { amount })
    }
}
