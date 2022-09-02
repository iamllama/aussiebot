use crate::cmds::{config_path, ConfigFile};
use crate::error::{self, Error};
use crate::{
    cache::{self, Cache, RespType},
    msg::{Location, Payload, Permissions, Ping, Platform, Response, User},
};
use bb8_redis::redis;
use once_cell::sync::Lazy;
use rand::Rng;
use serde_derive::{Deserialize, Serialize};
use std::path::Path;
use std::{collections::HashMap, sync::Arc};
use tokio::fs;
use tokio::sync::mpsc;

#[derive(Debug, Deserialize, Serialize)]
pub enum AuthMsg {
    ListUsers,
    RequestCode(Arc<String>),
    Login(Arc<String>, Arc<String>),
}

#[derive(Debug, Serialize, PartialEq)]
pub(crate) enum AuthError {
    Ratelimited,
    ServerError,
}

#[derive(Debug, Serialize, PartialEq)]
pub(crate) enum AuthResp {
    Users(Arc<Vec<String>>),
    InvalidUser,
    CodeReady,
    CodeExpired,
    AuthSuccess(Arc<String>),
    AuthFail,
    AuthError(AuthError),
}

type AuthMap = HashMap<String, (Arc<String>, usize)>; // name => (discord id, code validity duration)

#[derive(Clone)]
pub struct Handle {
    cache: cache::Handle,
    msg_out_tx: mpsc::Sender<(Location, Response)>,
    users: Arc<AuthMap>,
    usernames: Arc<Vec<String>>,
}

pub static MAX_AUTH_RATELIMIT_COUNT: Lazy<usize> = Lazy::new(|| {
    dotenv::var("MAX_AUTH_RATELIMIT_COUNT")
        .unwrap_or_default()
        .parse()
        .unwrap_or(10)
});
pub static MAX_AUTH_RATELIMIT_BURST: Lazy<usize> = Lazy::new(|| {
    dotenv::var("MAX_AUTH_RATELIMIT_BURST")
        .unwrap_or_default()
        .parse()
        .unwrap_or(20)
});

fn ratelimit_key(ip: impl AsRef<str>) -> String {
    format!(
        "aussiebot!{}!loginrl!{}",
        &*super::CHANNEL_NAME,
        ip.as_ref()
    )
}

fn code_key(user: impl AsRef<str>) -> String {
    format!(
        "aussiebot!{}!login!{}",
        &*super::CHANNEL_NAME,
        user.as_ref()
    )
}

fn gen_code() -> String {
    let code1 = rand::thread_rng().gen::<u64>();
    let code2 = rand::thread_rng().gen::<u64>();
    format!("{:08X}{:08X}", code1, code2)
}

impl Handle {
    pub fn new(
        cache: cache::Handle,
        msg_out_tx: mpsc::Sender<(Location, Response)>,
        users: AuthMap,
    ) -> Self {
        // TOOD: query a database table
        let users = Arc::new(users);

        let mut usernames = vec![];
        for user in users.keys() {
            usernames.push(user.to_string());
        }
        let usernames = Arc::new(usernames);

        Self {
            cache,
            msg_out_tx,
            users,
            usernames,
        }
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn handle(&self, peer_ip: &str, msg: AuthMsg) -> error::Result<AuthResp> {
        let rl_key = Arc::new(ratelimit_key(peer_ip));

        let rl_count = match Cache::Increment(rl_key.clone(), 1, *MAX_AUTH_RATELIMIT_BURST)
            .exec(&self.cache)
            .await?
        {
            RespType::U64(c) => c as usize,
            _ => unreachable!(),
        };

        tracing::debug!("{} = {}", rl_key, rl_count);

        if rl_count > *MAX_AUTH_RATELIMIT_COUNT {
            return Ok(AuthResp::AuthError(AuthError::Ratelimited));
        }

        match msg {
            AuthMsg::ListUsers => Ok(AuthResp::Users(self.usernames.clone())),
            AuthMsg::RequestCode(user) => {
                // check if user is in authmap
                let id_expiry = self.users.get(&*user);
                let (id, expiry) = match id_expiry {
                    Some(id_expiry) => id_expiry,
                    None => return Ok(AuthResp::InvalidUser),
                };
                let expiry = *expiry;

                // generate new password
                let code = Arc::new(gen_code());
                let key = Arc::new(code_key(&*user));

                // store code
                tracing::debug!(
                    "\x1b[93msetting {} to code {} for {} seconds\x1b[0m",
                    key,
                    code,
                    expiry
                );

                let cache_resp = Cache::Set(key.clone(), code.clone(), expiry, false)
                    .exec(&self.cache)
                    .await?;

                if !matches!(cache_resp, RespType::Bool(true)) {
                    tracing::error!("could not set key {} to code {}", key, code);
                    return Ok(AuthResp::AuthError(AuthError::ServerError));
                }

                // send code to discord
                //let msg = format!("{}, your code is\n`{}`\n(valid for 24 hours)", user, code);
                let msg = format!("`{}`", code);
                let pingee = Arc::new(User {
                    id: id.clone(),
                    name: "".to_owned().into(),
                    perms: Permissions::NONE,
                });

                Response {
                    platform: Platform::DISCORD,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(Ping {
                        pinger: None,
                        pingee,
                        msg: Some(msg.into()),
                        meta: None,
                    }),
                }
                .send(Location::Pubsub, &self.msg_out_tx)
                .await;

                Ok(AuthResp::CodeReady)
            }
            AuthMsg::Login(user, code) => {
                if !self.users.contains_key(&*user) {
                    return Ok(AuthResp::AuthFail);
                }

                let key = code_key(&*user); //format!(CODE_KEY, &*super::CHANNEL_NAME, user);
                let resp = Cache::Get(key.into()).exec(&self.cache).await;
                match resp {
                    Err(Error::Redis(e)) if e.kind() == redis::ErrorKind::TypeError => {
                        Ok(AuthResp::CodeExpired)
                    }
                    Err(e) => Err(e),
                    Ok(RespType::String(cod)) if cod.as_str() == code.as_str() => {
                        // clear ratelimit
                        Cache::Delete(rl_key.clone()).exec(&self.cache).await?;

                        Ok(AuthResp::AuthSuccess(user))
                    }
                    Ok(RespType::String(_)) => {
                        if rl_count == *MAX_AUTH_RATELIMIT_COUNT {
                            // the next request will be ratelimited, so stop here
                            Ok(AuthResp::AuthError(AuthError::Ratelimited))
                        } else {
                            Ok(AuthResp::AuthFail)
                        }
                    }
                    Ok(_) => unreachable!(),
                }
            }
        }
    }
}

pub async fn load() -> error::Result<AuthMap> {
    let contents =
        fs::read_to_string(Path::new(&*crate::CONFIG_DIR).join(config_path(ConfigFile::Users)))
            .await?;

    // deserialise
    let authmap: AuthMap = serde_json::from_str(&contents)?;

    Ok(authmap)
}

// pub(super) async fn save(users: &AuthMap) -> Result<(), std::io::Error> {
//     let dump = serde_json::to_string_pretty(&users).unwrap();
//     fs::write(
//         Path::new(&*crate::CONFIG_DIR).join(config_path(ConfigFile::Users)),
//         dump,
//     )
//     .await
// }
