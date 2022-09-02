use crate::{error, msg::Platform};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use std::{sync::Arc, time::SystemTime};
use tokio_postgres::NoTls;

#[derive(Debug)]
pub(crate) struct HoursOp {
    pub(crate) platform: Platform,
    pub(crate) id: Arc<String>,
    pub(crate) max_diff: i64,
}

pub(crate) async fn op(
    db: Pool<PostgresConnectionManager<NoTls>>,
    args: HoursOp,
) -> error::Result<i32> {
    let HoursOp {
        platform,
        id,
        max_diff,
    } = args;

    let now = SystemTime::now();

    let select_hours_sql = match platform {
        Platform::YOUTUBE => include_str!("./sql/select/hours_youtube.sql"),
        Platform::TWITCH => include_str!("./sql/select/hours_twitch.sql"),
        Platform::DISCORD => include_str!("./sql/select/hours_discord.sql"),
        _ => todo!(),
    };

    let upsert_hours_sql = match platform {
        Platform::YOUTUBE => include_str!("./sql/upsert/hours_youtube.sql"),
        Platform::TWITCH => include_str!("./sql/upsert/hours_twitch.sql"),
        Platform::DISCORD => include_str!("./sql/upsert/hours_discord.sql"),
        _ => todo!(),
    };

    let mut client = db.get().await?;
    let client = client.build_transaction().start().await?;

    // query hours, use default values if not found
    let (new_watchtime, new_last_seen) =
        if let Ok(row) = client.query_one(select_hours_sql, &[&id.as_str()]).await {
            let last_seen = row.get::<_, SystemTime>(0_usize);
            let watchtime = row.get::<_, i32>(1_usize);

            // calculate watchtime delta
            let diff = now
                .duration_since(last_seen)?
                // ignore if non-monotonic (e.g if Chat just inserted (last_seen=now()) before this runs)
                .as_secs()
                // clamp before casting
                .min(i32::MAX as u64) as i32;

            if max_diff > 0 && diff >= max_diff.min(i32::MAX as i64) as i32 {
                // too long since last message
                tracing::debug!("diff {} > max_diff {}", diff, max_diff);
                (watchtime, now)
            } else {
                (watchtime + diff, now)
            }
        } else {
            (0, now)
        };

    //upsert hours
    let _ = client
        .query(
            upsert_hours_sql,
            &[&id.as_str(), &new_watchtime, &new_last_seen],
        )
        .await?;

    // commit transaction
    client.commit().await?;

    Ok(new_watchtime)
}
