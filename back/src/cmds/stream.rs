use super::{CmdDesc, Context, Invokable, RunRes};
use crate::{
    //cache::{Cache, RespType},
    error::{self},
    msg::{Chat, Invocation, InvocationKind, Location, Payload, Platform, Response, StreamEvent},
};
use back_derive::command;
//use bb8_redis::redis;
use std::sync::Arc;

#[command(locks(id, url))]
/// Announce a stream
pub struct Stream {
    /// Platforms to annouce on
    #[cmd(defl("Platform::ANNOUNCE"))]
    platforms: Platform,
    /// Announcement message
    #[cmd(def("Hey @everyone <:PogChampGG:795488853091811389> <:PogChampGG:795488853091811389> <:PogChampGG:795488853091811389> today **AussieGG** brings you:\n{url}", constr(range = "1..=500")))]
    message: String,
}

impl Stream {
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
        if !self.enabled || self.message.is_empty() {
            return None;
        }

        if matches!(invocation.kind, Some(InvocationKind::Init)) {
            return match self.init(ctx).await {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::error!("{}", e);
                    None
                }
            };
        }

        let event = match invocation.kind {
            Some(InvocationKind::StreamEvent(ref evt)) => evt,
            _ => return None,
        };

        match self.run(ctx, event).await {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    #[tracing::instrument(skip(self, ctx), name = "Stream")]
    async fn run(&self, ctx: &Context<'_>, event: &StreamEvent) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), event = ?event);

        if let StreamEvent::Started(url, _id) = event {
            self.announce(ctx, url.clone()).await;
        }

        Ok(RunRes::Ok)
    }

    async fn announce(&self, ctx: &Context<'_>, url: Arc<String>) {
        let message = self.message.replace("{url}", &*url).replace("\\n", "\n");
        let message = Arc::new(message);
        tracing::info!(message = %message, "announcing stream");
        Response {
            platform: self.platforms,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::StreamAnnouncement(url.clone(), message.clone()),
        }
        .send(Location::Pubsub, ctx.resp)
        .await;
    }

    async fn init(&self, _ctx: &Context<'_>) -> error::Result<RunRes> {
        Ok(RunRes::Noop)
    }
}

impl CmdDesc for Stream {
    #[inline]
    fn platform(&self) -> Platform {
        Platform::empty()
    }
}

impl Invokable for Stream {}
