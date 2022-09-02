use super::{CmdDump, Command, CommandConfig, ConfigDump, Context, DFAWrapper};
use crate::{error, msg::Permissions};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{ser::Serialize, Deserialize, Deserializer, Serializer};
use std::sync::Arc;

impl Serialize for CommandConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let config = ConfigDump {
            filters: self.filters.iter().map(|c| c.dump()).collect(),
            commands: self.commands.iter().map(|c| c.dump()).collect(),
            timers: self.timers.iter().map(|c| c.dump()).collect(),
        };

        config.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CommandConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dump = ConfigDump::deserialize(deserializer)?;

        let ConfigDump {
            filters,
            commands,
            timers,
        } = dump;

        Ok(CommandConfig {
            filters: reinflate(filters),
            commands: reinflate(commands),
            timers: reinflate(timers),
        })
    }
}

fn reinflate(deflated: Vec<CmdDump>) -> Arc<Vec<Command>> {
    // TODO: warn of ignored invalue commands
    Arc::new(deflated.into_iter().filter_map(Command::new).collect()) // ignore invalid Commands
}

#[inline]
pub(crate) fn can_autocorrect(prefix: &str, dfaw: &Option<DFAWrapper>) -> Option<bool> {
    if let Some(DFAWrapper(dfa)) = dfaw {
        // check similarity
        let edit_distance = dfa.eval(prefix).to_u8();
        Some(edit_distance <= 2)
    } else {
        None
    }
}

pub(crate) static PREFIX_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s*$").unwrap());

pub(crate) async fn ratelimit_user<'a>(
    ctx: &Context<'a>,
    ratelimit_user: u64,
    ctype: &'static str,
    cname: &'a str,
    lock: &'static str,
) -> error::Result<bool> {
    let user = ctx.user;
    let key = { format!("{}_{}_{}", lock, cname, &user.id) };
    if user.perms >= Permissions::MOD || ratelimit_user == 0 {
        return Ok(false);
    }
    // check if rate-limited locally
    if !ctx.lock.lock(&key, ratelimit_user).await? {
        tracing::debug!(concat!("\x1b[33m{} rate-limited locally\x1b[0m"), ctype);
        return Ok(true);
    }
    Ok(false)
}

pub(crate) async fn ratelimit_global<'a>(
    ctx: &Context<'a>,
    ratelimit: u64,
    ratelimit_user: u64,
    ctype: &'static str,
    cname: &'a str,
    lock: &'static str,
) -> error::Result<bool> {
    let user = ctx.user;
    // only rate-limit if perm < Mod
    if user.perms < Permissions::MOD && (ratelimit > 0 || ratelimit_user > 0) {
        // key's a fn of cmd AND name, in case multiple instances are present, i.e multiple Text cmds
        // TODO: memoize
        let ratelimit_key = format!("{}_{}", lock, cname);

        // check if rate-limited globally
        if ratelimit > 0 && !ctx.lock.lock(&ratelimit_key, ratelimit).await? {
            //println!(concat!("\x1b[33m{} rate-limited globally\x1b[0m"), ctype);
            tracing::debug!(concat!("\x1b[33m{} rate-limited globally\x1b[0m"), ctype);
            return Ok(true);
        }

        // check if rate-limited locally
        if ratelimit_user > 0
            && !ctx
                .lock
                .lock(&format!("{}_{}", &ratelimit_key, &user.id), ratelimit_user)
                .await?
        {
            tracing::debug!(concat!("\x1b[33m{} rate-limited locally\x1b[0m"), ctype);
            // release the global ratelimit lock
            ctx.lock.unlock(ratelimit_key).await?;
            return Ok(true);
        }
    }

    Ok(false)
}

#[inline]
pub(crate) fn check_autocorrect(
    prefix: &str,
    input: &str,
    autocorrect: bool,
    dfaw: &Option<DFAWrapper>,
) -> Option<bool> {
    if prefix != input {
        if !autocorrect || !can_autocorrect(input, dfaw)? {
            return None;
        }
        Some(true)
    } else {
        Some(false)
    }
}
