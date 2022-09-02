use super::{CmdDesc, Context, RunRes};
use crate::{
    db::Db,
    error,
    msg::{Chat, Invocation, Platform},
};
use back_derive::command;
use once_cell::sync::Lazy;
use regex::Regex;

/*

Rolled 68, @AndukaR Uchiha won 4000 Points and now has 62456 Points
​@AndukaR Uchiha, you have 58456 Points.

Top 10 Points: 1. The One and Only (2005123), 2. Alex Hensley (1589281), 3. Merlijn (1531923), 4. chilli-chan (836975), 5. AtticNinja919 (750884), 6. Dave Rohël (732558), 7. L J (700050), 8.
*/

static GAMBLE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^Rolled (?:\d+), @(.+) (?:won|lost) (?:\d+) (?:\S+) and now has (\d+) (?:\S+)")
        .unwrap()
});

static POINTS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^@?(.+), you have (\d+) (?:\S+).").unwrap());

// static HOURS_REGEX: Lazy<Regex> =
//     Lazy::new(|| Regex::new(r"^@?(.+), you have (\d+) (?:\S+).").unwrap());

#[command(cmd)]
/// Scrape points from streamlabs' chatbot
pub struct Streamlabs {
    /// Streamlabs' ID
    #[cmd(def("UCNL8jaJ9hId96P13QmQXNtA"))]
    streamlabs_id: String,
}

impl Streamlabs {
    #[tracing::instrument(level = "trace", skip_all, name = "Streamlabs")]
    pub(super) async fn chat(&self, ctx: &Context<'_>, chat: &Chat) -> error::Result<RunRes> {
        if !self.enabled || self.streamlabs_id.is_empty() || *ctx.user.id != self.streamlabs_id {
            return Ok(RunRes::Disabled);
        }

        let msg = &chat.msg;

        if let Some(cap) = GAMBLE_REGEX.captures(msg) {
            let points = cap[2].parse::<i32>()?;
            self.handle_points(ctx, &cap[1], points).await?;
        } else if let Some(cap) = POINTS_REGEX.captures(msg) {
            let points = cap[2].parse::<i32>()?;
            self.handle_points(ctx, &cap[1], points).await?;
        }
        // else if let Some(_cap) = HOURS_REGEX.captures(msg) {
        //     self.handle_hours().await;
        // }

        Ok(RunRes::Noop)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        _ctx: &Context<'_>,
        _invocation: &Invocation,
    ) -> Option<RunRes> {
        None
    }

    async fn handle_points(&self, ctx: &Context<'_>, name: &str, points: i32) -> error::Result<()> {
        Db::SetPoints(ctx.platform, name.to_owned().into(), points)
            .exec(ctx.db)
            .await?;
        Ok(())
    }

    //async fn handle_hours(&self) {}
}

impl CmdDesc for Streamlabs {
    #[inline]
    fn platform(&self) -> crate::msg::Platform {
        Platform::YOUTUBE
    }
}
