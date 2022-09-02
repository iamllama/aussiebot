use crate::{
    error::{self, Error},
    msg::Platform,
};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_postgres::{NoTls, Row};

pub(crate) type ModActionRow = (Option<String>, String, String, String, u64);
pub(crate) type ModActionDump = Vec<(Platform, Vec<ModActionRow>)>;

pub(crate) async fn op(db: Pool<PostgresConnectionManager<NoTls>>) -> error::Result<ModActionDump> {
    let client = db.get().await.unwrap();

    let (r1, r2, r3) = futures_util::future::join3(
        client.query(include_str!("sql/select/modaction_youtube.sql"), &[]),
        client.query(include_str!("sql/select/modaction_discord.sql"), &[]),
        client.query(include_str!("sql/select/modaction_twitch.sql"), &[]),
    )
    .await;

    let pairs =
        [r1, r2, r3]
            .into_iter()
            .zip([Platform::YOUTUBE, Platform::DISCORD, Platform::TWITCH]);

    tokio::task::spawn_blocking(|| {
        pairs
            .map(|(rows, platform)| {
                (
                    platform,
                    rows.unwrap_or_default()
                        .iter()
                        .filter_map(|row| match handle_row(row) {
                            Ok(row) => Some(row),
                            Err(e) => {
                                tracing::error!("{}", e);
                                None
                            }
                        })
                        .collect(),
                )
            })
            .collect::<ModActionDump>()
    })
    .await
    .map_err(Error::Join)
}

fn handle_row(row: &Row) -> error::Result<ModActionRow> {
    let disp_name = row.try_get::<_, String>(0).ok();
    let platform_id = row.try_get::<_, String>(1)?;
    let action = row.try_get::<_, String>(2)?;
    let reason = row.try_get::<_, String>(3)?;
    let at = row
        .try_get::<_, SystemTime>(4)?
        .duration_since(UNIX_EPOCH)?
        .as_secs();

    Ok((disp_name, platform_id, action, reason, at))
}
