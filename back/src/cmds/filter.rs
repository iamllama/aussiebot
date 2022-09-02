use super::{Context, FilterCache, ModAction, RunRes};
use crate::{
    error,
    msg::{Chat, Invocation, Permissions, Platform},
};
use back_derive::command;
use std::sync::Arc;

#[command(filter)]
/// Filter chat based on username and message
pub struct Filter {
    /// Apply to anyone below permission level
    #[cmd(defl("Permissions::NONE"))]
    apply_to: Permissions,
    /// Platforms
    #[cmd(defl("Platform::CHAT"))]
    platforms: Platform,
    /// Mod action
    #[cmd(defl("ModAction::None"), constr(range = "1..=86400"))]
    action: ModAction,
    /// Username contains
    user_contains: String,
    /// Message contains
    msg_contains: String,
    /// User id contains  (case-sensitive)
    id_contains: String,
}

impl Filter {
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

    /// cache lowercase copies of chat.user's fields
    pub(crate) fn fill_cache(ctx: &Context<'_>, chat: &Chat) {
        // fill filter cache if empty
        if matches!(*ctx.filter_cache.read(), None) {
            *ctx.filter_cache.write() = Some(FilterCache {
                id: Arc::new(chat.user.id.to_lowercase()),
                name: Arc::new(chat.user.name.to_lowercase()),
                msg: Arc::new(chat.msg.to_lowercase()),
            });
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn chat(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        if self.can_run(ctx).is_none() {
            return Ok(RunRes::Disabled);
        }
        // match self.run(ctx, chat).await {
        //     Ok(r) => Some(r),
        //     Err(e) => {
        //         tracing::error!("{}", e);
        //         None
        //     }
        // }
        self.run(ctx, chat).await
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        _ctx: &Context<'_>,
        _invocation: &Invocation,
    ) -> Option<RunRes> {
        None
    }

    #[tracing::instrument(level = "trace", skip_all, name = "Filter")]
    async fn run(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        // fill filter cache if empty
        Filter::fill_cache(ctx, chat);

        let filter_action = RunRes::Filtered(self.action);
        let mut triggered: [Option<bool>; 3] = [None; 3];

        if let Some(ref cache) = *ctx.filter_cache.read() {
            if !self.user_contains.is_empty() {
                let cond = cache.name.contains(&self.user_contains);
                if cond {
                    tracing::info!(
                        "\x1b[91mUsername {} contains '{}'\x1b[0m",
                        chat.user.name,
                        self.user_contains
                    );
                }
                triggered[0] = Some(cond);
            }

            if !self.id_contains.is_empty() {
                let cond = cache.id.contains(&self.id_contains);
                if cond {
                    tracing::info!(
                        "\x1b[91mUser id {} contains '{}'\x1b[0m",
                        cache.id,
                        self.id_contains
                    );
                }
                triggered[1] = Some(cond);
            }

            if !self.msg_contains.is_empty() {
                let cond = cache.msg.contains(&self.msg_contains);
                if cond {
                    tracing::info!(
                        "\x1b[91mMessage from {} contains '{}'\x1b[0m",
                        chat.user.name,
                        self.msg_contains
                    );
                }
                triggered[2] = Some(cond);
            }

            // None => filter not enabled
            // Some(false) => filter not tripped
            // Some(true) => tripped

            // returns false if any enabled filter was left untripped, otherwise returns true if any filter was tripped
            let (_, tripped) =
                triggered
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
        } else {
            Err("expected filter cache to be filled".into())
        }
    }
}
