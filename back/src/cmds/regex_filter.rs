use super::{Context, ModAction, RunRes};
use crate::{
    error,
    msg::{Chat, Invocation, Permissions, Platform},
};
use back_derive::command;
use regex::Regex;

#[command(filter)]
/// Filter chat by matching username, id and/or message against regex patterns
pub struct RegexFilter {
    /// Apply to anyone below permission level
    #[cmd(defl("Permissions::NONE"))]
    apply_to: Permissions,
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Mod action
    #[cmd(defl("ModAction::None"), constr(range = "1..=86400"))]
    action: ModAction,
    /// Username matches
    #[cmd(defl(r#"Regex::new("").unwrap()"#))]
    user_pattern: Regex,
    /// Message matches
    #[cmd(defl(r#"Regex::new("").unwrap()"#))]
    msg_pattern: Regex,
    /// User id matches
    #[cmd(defl(r#"Regex::new("").unwrap()"#))]
    id_pattern: Regex,
}

impl RegexFilter {
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
        self.run(chat).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        _ctx: &Context<'_>,
        _invocation: &Invocation,
    ) -> Option<RunRes> {
        None
    }

    #[tracing::instrument(level = "trace", skip_all, name = "RegexFilter")]
    async fn run(&self, chat: &Chat) -> error::Result<RunRes> {
        // fill filter cache if empty
        //super::Filter::fill_cache(ctx);

        let filter_action = RunRes::Filtered(self.action);
        let mut triggered: [Option<bool>; 3] = [None; 3];

        //if let Some(ref cache) = *ctx.filter_cache.read() {
        if !self.user_pattern.as_str().is_empty() {
            let cond = self.user_pattern.is_match(&chat.user.name);
            if cond {
                tracing::info!(
                    "\x1b[91mUsername {} matches '{}'\x1b[0m",
                    chat.user.name,
                    self.user_pattern
                );
            }
            triggered[0] = Some(cond);
        }

        if !self.id_pattern.as_str().is_empty() {
            let cond = self.id_pattern.is_match(&chat.user.id);
            if cond {
                tracing::info!(
                    "\x1b[91mUser id {} matches '{}'\x1b[0m",
                    chat.user.id,
                    self.id_pattern
                );
            }
            triggered[1] = Some(cond);
        }

        if !self.msg_pattern.as_str().is_empty() {
            let cond = self.msg_pattern.is_match(&chat.msg);
            if cond {
                tracing::info!(
                    "\x1b[91mMessage from {} matches '{}'\x1b[0m",
                    chat.user.name,
                    self.msg_pattern
                );
            }
            triggered[2] = Some(cond);
        }

        // None => filter not enabled
        // Some(false) => filter not tripped
        // Some(true) => tripped

        // returns false if any enabled filter was left untripped, otherwise returns true if any filter was tripped
        let (_, tripped) = triggered
            .into_iter()
            .fold((true, false), |acc, res| match (acc, res) {
                (_, Some(false)) => (false, false),
                ((true, _), Some(true)) => (true, true),
                _ => acc,
            });

        if tripped {
            Ok(filter_action)
        } else {
            Ok(RunRes::Ok)
        }
        // } else {
        //     None
        // }
    }
}
