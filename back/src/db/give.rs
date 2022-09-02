use crate::{
    error::{self, Error},
    msg::Platform,
};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use std::{fmt::Display, sync::Arc};
use tokio_postgres::{NoTls, Transaction};

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum GiveSource {
    Id(Platform, Arc<String>),
    Linked(Platform, Platform, Arc<String>),
    None,
}

#[derive(Debug)]
pub(crate) enum GiveTarget {
    Name(Platform, Arc<String>),
    User(Platform, Arc<String>, Arc<String>),
    Linked(Platform),
    Spend,
}

#[derive(Debug)]
pub(crate) struct GiveOp {
    pub(crate) from: GiveSource,
    pub(crate) to: GiveTarget,
    pub(crate) amount: i32,
    pub(crate) min: i64,
    pub(crate) max: i64,
}

#[derive(Debug)]
pub enum GiveError {
    SamePlatform,
    InvalidPlatform,
    Deduct,
    Deposit,
    AmountBelowMin { amount: i32, min: i32 },
}

impl Display for GiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

type Ret = i32;

//impl super::Actor {
pub(crate) async fn op(
    db: Pool<PostgresConnectionManager<NoTls>>,
    args: GiveOp,
) -> error::Result<Ret> {
    // start transaction
    let mut client = db.get().await.unwrap();
    let client = client.build_transaction().start().await?;

    match (&args.from, &args.to) {
        (GiveSource::Linked(platorig, platfrom, id), GiveTarget::Linked(platto)) => {
            if platfrom == platto {
                return Err(GiveError::SamePlatform.into());
            }

            let link_sql = match *platorig {
                Platform::YOUTUBE => include_str!("sql/select/points_youtube.sql"),
                Platform::DISCORD => include_str!("sql/select/points_discord.sql"),
                Platform::TWITCH => include_str!("sql/select/points_twitch.sql"),
                _ => unreachable!(),
            };

            // println!("Linked from: {} {}, to: {}", id, platfrom, platto);
            // println!("link_sql: {}", link_sql);

            // check if link exists
            let row = client.query_one(link_sql, &[&id.as_str()]).await.unwrap();

            let get_id = |p: Platform| match p {
                Platform::YOUTUBE => row.try_get::<_, String>(0).map_err(Error::Postgres),
                Platform::DISCORD => row.try_get::<_, String>(1).map_err(Error::Postgres),
                Platform::TWITCH => row.try_get::<_, String>(2).map_err(Error::Postgres),
                _ => Err(GiveError::InvalidPlatform.into()),
            };

            let (platfrom, platto) = (*platfrom, *platto);

            // check if 'from' platform is linked
            let from_id = get_id(platfrom)?;
            // check if 'to' platform is linked
            let to_id = get_id(platto)?;

            let (client, amount) = get_amount(client, platfrom, &from_id, &args).await?;
            let client = handle_deduct_id(client, platfrom, &from_id, amount).await?;
            let client = handle_deposit_id(client, platto, &to_id, amount).await?;
            client.commit().await?;
            Ok(amount)
        }
        (GiveSource::Id(platfrom, from_id), GiveTarget::Name(platto, to_name)) => {
            let (client, amount) = get_amount(client, *platfrom, &**from_id, &args).await?;
            let client = handle_deduct_id(client, *platfrom, &**from_id, amount).await?;
            let client = handle_deposit_name(client, *platto, &**to_name, amount).await?;
            client.commit().await?;
            Ok(amount)
        }
        (GiveSource::Id(platfrom, from_id), GiveTarget::User(platto, to_id, _to_name)) => {
            let (client, amount) = get_amount(client, *platfrom, &**from_id, &args).await?;
            let client = handle_deduct_id(client, *platfrom, &**from_id, amount).await?;
            let client = handle_deposit_id(client, *platto, &**to_id, amount).await?;
            client.commit().await?;
            Ok(amount)
        }
        (GiveSource::Id(platfrom, from_id), GiveTarget::Spend) => {
            let (client, amount) = get_amount(client, *platfrom, &**from_id, &args).await?;
            let client = handle_deduct_id(client, *platfrom, &**from_id, amount).await?;
            client.commit().await?;
            Ok(amount)
        }
        (GiveSource::None, GiveTarget::Name(platto, to_name)) => {
            let client = handle_deposit_name(client, *platto, &**to_name, args.amount).await?;
            client.commit().await?;
            Ok(args.amount)
        }
        (GiveSource::None, GiveTarget::User(platto, to_id, _to_name)) => {
            let client = handle_deposit_id(client, *platto, &**to_id, args.amount).await?;
            client.commit().await?;
            Ok(args.amount)
        }
        (GiveSource::None, GiveTarget::Spend) => panic!("Invalid combination"),
        (_, GiveTarget::Linked(_)) | (GiveSource::Linked(_, _, _), _) => {
            panic!("Both or neither of GiveSource and GiveTarget must be Linked")
        }
    }
}

async fn get_amount<'a>(
    client: Transaction<'a>,
    platform: Platform,
    source: impl AsRef<str>,
    args: &'a GiveOp,
) -> error::Result<(Transaction<'a>, i32)> {
    let amount = args.amount;
    let min = args.min as i32;
    let max = args.max as i32;

    let amount = if amount == -1 {
        // all
        let points_sql = match platform {
            Platform::YOUTUBE => include_str!("sql/select/youtube_id_lock.sql"),
            Platform::DISCORD => include_str!("sql/select/discord_id_lock.sql"),
            Platform::TWITCH => include_str!("sql/select/twitch_id_lock.sql"),
            _ => return Err(GiveError::InvalidPlatform.into()),
        };

        // query points
        let amount = client
            .query_one(points_sql, &[&source.as_ref()])
            .await
            .unwrap();

        amount.get::<_, i32>(2_usize)
    } else {
        amount
    };

    if amount < min {
        return Err(GiveError::AmountBelowMin { amount, min }.into());
    }

    // clamp amount
    let amount = amount.min(max);

    Ok((client, amount))
}

async fn handle_deduct_id(
    client: Transaction<'_>,
    platform: Platform,
    source: impl AsRef<str>,
    amount: i32,
) -> error::Result<Transaction<'_>> {
    let deduct_sql = match platform {
        Platform::YOUTUBE => include_str!("sql/update/decr_points_youtube.sql"),
        Platform::DISCORD => include_str!("sql/update/decr_points_discord.sql"),
        Platform::TWITCH => include_str!("sql/update/decr_points_twitch.sql"),
        _ => return Err(GiveError::InvalidPlatform.into()),
    };

    // try deducting from src
    let decremented = client
        .query(deduct_sql, &[&source.as_ref(), &amount])
        .await
        .unwrap();

    // rollback on failure
    if decremented.is_empty() {
        tracing::debug!(
            "\x1b[91mFailed to deduct {} point{} from {}\x1b[0m",
            amount,
            if amount != 1 { "s" } else { "" },
            source.as_ref(),
        );
        return Err(GiveError::Deduct.into());
    }

    Ok(client)
}

async fn handle_deposit_name(
    client: Transaction<'_>,
    platform: Platform,
    target: impl AsRef<str>,
    amount: i32,
) -> error::Result<Transaction<'_>> {
    let deposit_sql = match platform {
        Platform::YOUTUBE => include_str!("sql/update/incr_points_youtube_name.sql"),
        Platform::DISCORD => include_str!("sql/update/incr_points_discord_name.sql"),
        _ => return Err(GiveError::InvalidPlatform.into()),
    };
    _handle_deposit(client, target, amount, deposit_sql).await
}

async fn handle_deposit_id(
    client: Transaction<'_>,
    platform: Platform,
    target: impl AsRef<str>,
    amount: i32,
) -> error::Result<Transaction<'_>> {
    let deposit_sql = match platform {
        Platform::YOUTUBE => include_str!("sql/update/incr_points_youtube_id.sql"),
        Platform::DISCORD => include_str!("sql/update/incr_points_discord_id.sql"),
        Platform::TWITCH => include_str!("sql/update/incr_points_twitch_id.sql"),
        _ => return Err(GiveError::InvalidPlatform.into()),
    };
    _handle_deposit(client, target, amount, deposit_sql).await
}

async fn _handle_deposit<'a>(
    client: Transaction<'a>,
    target: impl AsRef<str>,
    amount: i32,
    deposit_sql: &'a str,
) -> error::Result<Transaction<'a>> {
    // try depositing into dest
    let incremented = client
        .query(deposit_sql, &[&target.as_ref(), &amount])
        .await?;

    // rollback on failure
    if incremented.is_empty() {
        tracing::debug!(
            "\x1b[91mFailed to deposit {} point{} into {}\x1b[0m",
            amount,
            if amount != 1 { "s" } else { "" },
            target.as_ref()
        );
        return Err(GiveError::Deposit.into());
    }

    Ok(client)
}
