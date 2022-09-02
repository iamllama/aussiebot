use super::{CmdDesc, Context, Invokable, RunRes};
use crate::{
    error,
    msg::{
        discord::{self, DiscordAction},
        Chat, ChatMeta, Invocation, InvocationKind, Payload, Platform, Response,
    },
};
use back_derive::command;
use std::sync::Arc;

#[command(cmd)]
/// Let users self-assign a role by reacting to a message (Discord-specific)
pub struct ReactionRole {
    /// Emoji (or ID if custom)
    #[cmd(def("🤔"))]
    emoji: String,
    /// Message ID to watch for reactions on
    message_id: String,
    /// Role ID to add/remove
    role_id: String,
}

impl ReactionRole {
    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn chat(&self, _ctx: &Context<'_>, _chat: &Chat) -> error::Result<RunRes> {
        Ok(RunRes::Noop)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        ctx: &Context<'_>,
        invocation: &Invocation,
    ) -> Option<RunRes> {
        if !self.enabled || self.role_id.is_empty() {
            return None;
        }

        // check message id and eomji
        match invocation.kind {
            Some(InvocationKind::Reaction {
                ref message_id,
                ref emoji,
            }) if self.message_id == *message_id && self.emoji == *emoji => {}
            _ => return None,
        }

        let is_add = match invocation.cmd.as_str() {
            "@reaction_add" => true,
            "@reaction_rem" => false,
            _ => return None,
        };

        let guild_id = match invocation.meta {
            Some(ChatMeta::Discord4(ref guild_id)) => Some(guild_id.clone()),
            _ => None,
        };

        match self.run(ctx, is_add, guild_id).await {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    #[tracing::instrument(skip(self, ctx), name = "ReactionRole")]
    async fn run(
        &self,
        ctx: &Context<'_>,
        is_add: bool,
        guild_id: Option<Arc<String>>,
    ) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.id.as_str(), role=%self.role_id, is_add = %is_add);

        let inner = discord::Role {
            user_id: ctx.user.id.clone(),
            role_id: self.role_id.clone().into(),
            guild_id,
            reason: Some(
                if self.name.is_empty() {
                    "ReactionRole".to_owned()
                } else {
                    format!("ReactionRole ({})", self.name)
                }
                .into(),
            ),
        };

        let action = if is_add {
            DiscordAction::AddRole(inner)
        } else {
            DiscordAction::RemoveRole(inner)
        };

        Response {
            platform: ctx.platform,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Discord(action),
        }
        .send(ctx.location.clone(), ctx.resp)
        .await;

        Ok(RunRes::Ok)
    }
}

impl CmdDesc for ReactionRole {
    #[inline]
    fn platform(&self) -> Platform {
        Platform::DISCORD
    }
}

impl Invokable for ReactionRole {}
