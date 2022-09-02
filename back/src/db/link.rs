use crate::{error, msg::Platform};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use std::sync::Arc;
use tokio_postgres::{types::ToSql, NoTls};

#[derive(Debug)]
pub(crate) struct LinkOp {
    pub(crate) platform: Platform,
    pub(crate) discord_id: Arc<String>,
    pub(crate) platform_id: Arc<String>,
}

pub(crate) async fn op(
    db: Pool<PostgresConnectionManager<NoTls>>,
    args: LinkOp,
) -> error::Result<()> {
    let delete_sql = [
        include_str!("sql/delete/link_yt.sql"),
        include_str!("sql/delete/link_tw.sql"),
    ];

    let upsert_sql = match args.platform {
        Platform::YOUTUBE => include_str!("sql/upsert/link_yt.sql"),
        Platform::TWITCH => include_str!("sql/upsert/link_tw.sql"),
        _ => unreachable!(),
    };

    // start transaction
    let mut client = db.get().await?;
    let client = client.build_transaction().start().await?;

    // delete existing links
    let arg: [&(dyn ToSql + Sync); 1] = [&args.discord_id.as_str()];
    let res = futures_util::future::join_all(delete_sql.map(|sql| client.query(sql, &arg))).await;
    for item in res {
        item?;
    }

    // insert link
    client
        .query_one(
            upsert_sql,
            &[&args.platform_id.as_str(), &args.discord_id.as_str()],
        )
        .await?;

    client.commit().await?;

    Ok(())
}
