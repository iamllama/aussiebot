use super::{util, Arg, ArgKind, ArgValue, CmdDesc, Context, Invokable, RunRes};
use crate::{
    cache::{self, Cache, RespType},
    error,
    msg::{
        ArgMap, ArgMapError, Autocomplete, Chat, ChatMeta, Invocation, InvocationKind, Payload,
        Permissions, Ping, Platform, Response,
    },
};
use back_derive::command;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

#[derive(Debug)]
enum Args {
    Search(String),
    List,
    EditLast {
        name: Option<String>,
    },
    Add {
        link: String,
        name: String,
        silent: bool,
    },
    Clear,
    EditSearch {
        search: String,
        name: Option<String>,
    },
}

/// (link, name)
type Item = (String, String);

#[command(locks(rate, cache))]
/// Store memes for future use
pub struct MemeBank {
    /// Command prefix
    #[cmd(def("!meme"), constr(non_empty))]
    prefix: String,
    /// Autocorrect prefix
    autocorrect: bool,
    /// Permissions
    #[cmd(defl("Permissions::NONE"))]
    perms: Permissions,
    /// Cooldown per user (in seconds)
    #[cmd(constr(pos))]
    ratelimit_user: u64,
    /// Automatically add sent attachments
    #[cmd(def(true))]
    scrape_attachments: bool,
}

impl MemeBank {
    fn _parse_arguments(&self, _chat: &Chat) -> Option<(bool, Args)> {
        None
    }

    fn can_run(&self, ctx: &Context<'_>) -> Option<()> {
        if !self.enabled {
            return None;
        }

        // check if platform is applicable
        if !ctx.platform.contains(Platform::DISCORD) {
            return None;
        }

        // check perms
        if ctx.user.perms < self.perms {
            return None;
        }

        Some(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn chat(&self, ctx: &Context<'_>, _chat: &Chat) -> error::Result<RunRes> {
        if self.can_run(ctx).is_none() || !self.scrape_attachments {
            return Ok(RunRes::Disabled);
        }

        let meta = match ctx.meta {
            Some(m) => m,
            None => return Ok(RunRes::Noop),
        };

        let attachments = match meta {
            ChatMeta::Discord2(_, _, att, _) | ChatMeta::Discord3(att, _) if !att.is_empty() => att,
            _ => return Ok(RunRes::Noop),
        };

        // TODO use linkify to detect a tenor/giphy/discordcdn link in chat msg

        tracing::debug!(attachments=?attachments);

        let add_fut = attachments.iter().map(|(name, link)| {
            self.run(
                ctx,
                Args::Add {
                    link: link.to_owned(),
                    name: name.to_owned(),
                    silent: true,
                },
                None,
            )
        });

        let res = futures_util::future::join_all(add_fut).await;

        for r in res {
            r?;
        }

        Ok(RunRes::Noop)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) async fn invoke(
        &self,
        ctx: &Context<'_>,
        invocation: &Invocation,
    ) -> Option<RunRes> {
        self.can_run(ctx)?;

        super::check_invoke_prefix(&self.prefix, &invocation.cmd)?;

        let args = Args::try_from(&invocation.args).ok()?;

        match util::ratelimit_user(
            ctx,
            self.ratelimit_user,
            stringify!(MemeBank),
            &self.name,
            &*MEMEBANK_LOCK_RATE,
        )
        .await
        {
            Ok(false) => {}
            Ok(true) => return None,
            Err(e) => {
                tracing::error!("{}", e);
                return None;
            }
        }

        // if matches!(args, Args::Add { .. }) && ctx.user.perms < Permissions::MOD {
        //     // only trust mods and up for now
        //     Response {
        //         platform: ctx.platform,
        //         channel: &*crate::CHANNEL_NAME,
        //         payload: Payload::Ping(Ping {
        //             pinger: None,
        //             pingee: ctx.user.clone(),
        //             msg: Some("Not yet allowed".to_owned().into()),
        //             meta: ctx.meta.clone(),
        //         }),
        //     }
        //     .send(ctx.location.clone(), ctx.resp)
        //     .await;

        //     return None;
        // }

        match self.run(ctx, args, invocation.kind.as_ref()).await {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    async fn add(item: Item, key: Arc<String>, cache: &cache::Handle) -> error::Result<()> {
        let item = tokio::task::spawn_blocking(move || serde_json::to_string(&item)).await??;

        let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;

        let timestamp = duration
            .as_secs()
            .wrapping_mul(1000) // overflow is ok, since overlap is practically impossible
            .wrapping_add(duration.subsec_millis() as u64) // extra resolution
            .to_string();

        Cache::Zadd(key, timestamp.into(), item.into())
            .exec(cache)
            .await?;

        Ok(())
    }

    async fn get_all(
        key: Arc<String>,
        cache: &cache::Handle,
    ) -> error::Result<impl Iterator<Item = (isize, Item)>> {
        let res = match Cache::Zrangewithscores(key, 0, -1).exec(cache).await? {
            RespType::VecStringScore(list) => list,
            _ => unreachable!(),
        };

        // parse items
        // reverse, newest first
        let res = futures_util::future::join_all(res.into_iter().rev().map(|(item, timestamp)| {
            tokio::task::spawn_blocking(move || (timestamp, serde_json::from_str::<Item>(&item)))
        }))
        .await;

        tracing::debug!(res=?res);

        Ok(res.into_iter().filter_map(|x| match x {
            Ok((ts, Ok(x))) => Some((ts, x)),
            _ => None,
        }))
    }

    async fn autocomplete(
        res: impl Iterator<Item = (isize, Item)>,
        search: impl AsRef<str>,
    ) -> Vec<(String, String)> {
        res.into_iter()
            .enumerate()
            .filter_map(|(i, (_ts, r))| match r {
                (_link, name) if name.starts_with(search.as_ref()) => {
                    Some((name, i.to_string())) // value's max length is 100, so use index instead
                }
                _ => None,
            })
            .collect()
    }

    async fn parse_choice(
        res: impl Iterator<Item = (isize, Item)>,
        search: impl AsRef<str>,
    ) -> Option<(isize, Item)> {
        let search = search.as_ref();
        let _search = search.parse::<usize>();

        match _search {
            Ok(i) => res.into_iter().nth(i),
            //.unwrap_or(("⚠ Meme not found".to_owned(), "".to_owned())),
            Err(_) => {
                // wasn't an index, treat as partial name
                res.into_iter()
                    .find(|(_ts, r)| matches!(r, (_link, name) if name.starts_with(search)))
                //.unwrap_or(("⚠ Meme not found".to_owned(), "".to_owned()))
            }
        }
    }

    #[tracing::instrument(skip(self, ctx), name = "MemeBank")]
    async fn run(
        &self,
        ctx: &Context<'_>,
        args: Args,
        kind: Option<&InvocationKind>,
    ) -> error::Result<RunRes> {
        tracing::debug!(name = self.name.as_str(), user = ctx.user.name.as_str(), args = ?args);

        let key = Arc::new(format!("{}_{}", &*MEMEBANK_LOCK_CACHE, ctx.user.id));

        match args {
            Args::Search(search) => {
                tracing::debug!(search=%search, "searching");

                let res = Self::get_all(key, ctx.cache).await?;

                match kind {
                    Some(&InvocationKind::Autocomplete) => {
                        // filter and get matches
                        let choices: Vec<(String, String)> = Self::autocomplete(res, search).await;
                        tracing::debug!(choices=?choices);
                        Response {
                            platform: ctx.platform,
                            channel: &*crate::CHANNEL_NAME,
                            payload: Payload::Autocomplete(Autocomplete {
                                choices,
                                meta: ctx.meta.clone(),
                            }),
                        }
                        .send(ctx.location.clone(), ctx.resp)
                        .await;
                    }
                    Some(_) => unimplemented!(),
                    // implicit InvocationKind::Invoke
                    None => {
                        // try to parse as index into choices
                        let (_ts, (link, name)) = Self::parse_choice(res, search)
                            .await
                            .unwrap_or((0, ("⚠ Not found".to_owned(), "".to_owned())));

                        if !name.is_empty() {
                            tracing::debug!(link=%link, name=%name, "FOUND");
                        }

                        Response {
                            platform: ctx.platform,
                            channel: &*crate::CHANNEL_NAME,
                            payload: Payload::Ping(Ping {
                                pinger: None,
                                pingee: ctx.user.clone(),
                                msg: Some(link.into()),
                                meta: ctx.meta.clone(),
                            }),
                        }
                        .send(ctx.location.clone(), ctx.resp)
                        .await;
                    }
                }
            }
            Args::EditSearch { search, name } => {
                tracing::debug!(search=%search, "edit-searching");

                let res = Self::get_all(key.clone(), ctx.cache).await?;

                match kind {
                    Some(&InvocationKind::Autocomplete) => {
                        let choices: Vec<(String, String)> = Self::autocomplete(res, search).await;
                        tracing::debug!(choices=?choices);
                        Response {
                            platform: ctx.platform,
                            channel: &*crate::CHANNEL_NAME,
                            payload: Payload::Autocomplete(Autocomplete {
                                choices,
                                meta: ctx.meta.clone(),
                            }),
                        }
                        .send(ctx.location.clone(), ctx.resp)
                        .await;
                    }
                    Some(_) => unimplemented!(),
                    // implicit InvocationKind::Invoke
                    None => {
                        // try to parse as index into choices
                        let (ts, (link, _name)) = match Self::parse_choice(res, search).await {
                            Some(x) => x,
                            None => {
                                Response {
                                    platform: ctx.platform,
                                    channel: &*crate::CHANNEL_NAME,
                                    payload: Payload::Ping(Ping {
                                        pinger: None,
                                        pingee: ctx.user.clone(),
                                        msg: Some("⚠ Not found".to_owned().into()),
                                        meta: ctx.meta.clone(),
                                    }),
                                }
                                .send(ctx.location.clone(), ctx.resp)
                                .await;
                                return Ok(RunRes::Noop);
                            }
                        };

                        let ts = Arc::new(ts.to_string());

                        // remove old key by score (ts)
                        Cache::Zremrangebyscore(key.clone(), ts.clone(), ts.clone())
                            .exec(ctx.cache)
                            .await?;

                        // add if applicable
                        let msg = if let Some(name) = name {
                            let msg = format!("Renamed `{}` to `{}`", _name, name);
                            Self::add((link, name), key, ctx.cache).await?;
                            msg
                        } else {
                            format!("Removed `{}`: {}", _name, link)
                        };

                        Response {
                            platform: ctx.platform,
                            channel: &*crate::CHANNEL_NAME,
                            payload: Payload::Ping(Ping {
                                pinger: None,
                                pingee: ctx.user.clone(),
                                msg: Some(msg.into()),
                                meta: ctx.meta.clone(),
                            }),
                        }
                        .send(ctx.location.clone(), ctx.resp)
                        .await;
                    }
                }
            }
            Args::List => {
                let res = match Cache::Zrangewithscores(key, 0, -1).exec(ctx.cache).await? {
                    RespType::VecStringScore(list) => list,
                    _ => unreachable!(),
                };

                // parse items
                let res =
                    futures_util::future::join_all(res.into_iter().map(|(item, _timestamp)| {
                        tokio::task::spawn_blocking(move || serde_json::from_str::<Item>(&item))
                    }))
                    .await;

                let mut count = 0;

                let mut choices: String = if !res.is_empty() {
                    res.into_iter()
                        .rev() // newest first
                        .filter_map(|r| match r {
                            Ok(Ok((_link, name))) => {
                                count += 1;
                                Some(format!(":small_orange_diamond: {}\n", name) /*format!("`{}`: {}\n", name, link)*/)
                                // could very easily exceed max length of 2000, so no links for now
                            }
                            _ => None,
                        })
                        .collect()
                } else {
                    "⚠ No items saved".to_owned()
                };

                choices.push_str(&format!(
                    "\n(_{} item{} in total_)",
                    count,
                    if count != 1 { "s" } else { "" }
                ));
                let choices = choices;

                tracing::debug!(choices=%choices);

                Response {
                    platform: ctx.platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(Ping {
                        pinger: None,
                        pingee: ctx.user.clone(),
                        msg: Some(choices.into()),
                        meta: ctx.meta.clone(),
                    }),
                }
                .send(ctx.location.clone(), ctx.resp)
                .await;
            }
            Args::EditLast { name } => {
                //Cache::ZPopMax
                let mut res = match Cache::Zpopmax(key.clone(), 1).exec(ctx.cache).await? {
                    RespType::VecStringScore(l) => l,
                    _ => unreachable!(),
                };
                let (item, _score) = if let Some(x) = res.pop() {
                    x
                } else {
                    // TODO: error msg
                    return Ok(RunRes::Noop);
                };

                let (link, _name): Item =
                    tokio::task::spawn_blocking(move || serde_json::from_str::<Item>(&item))
                        .await??;

                let msg = if let Some(name) = name {
                    let msg = format!("Renamed `{}` to `{}`", _name, name);

                    Self::add((link, name), key, ctx.cache).await?;

                    msg
                } else {
                    format!("Removed `{}`: {}", _name, link)
                };

                Response {
                    platform: ctx.platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(Ping {
                        pinger: None,
                        pingee: ctx.user.clone(),
                        msg: Some(msg.into()),
                        meta: ctx.meta.clone(),
                    }),
                }
                .send(ctx.location.clone(), ctx.resp)
                .await;
            }
            Args::Add { link, name, silent } => {
                let url = Url::parse(&link)?;

                let msg = match url.host_str() {
                    Some(
                        "discord.com" // message links
                        | "cdn.discordapp.com" // attachments
                        | "media.discordapp.net" // cached attachments
                        | "tenor.com"
                        | "giphy.com",
                    ) => {
                        let msg = format!("Added `{}`: {}", name, link);

                        Self::add((link, name), key, ctx.cache).await?;

                        msg
                    }
                    _ => {
                        tracing::warn!(link=%link,"invalid link");
                        "⚠ Invalid link".to_owned()
                    }
                };

                if !silent {
                    Response {
                        platform: ctx.platform,
                        channel: &*crate::CHANNEL_NAME,
                        payload: Payload::Ping(Ping {
                            pinger: None,
                            pingee: ctx.user.clone(),
                            msg: Some(msg.into()),
                            meta: ctx.meta.clone(),
                        }),
                    }
                    .send(ctx.location.clone(), ctx.resp)
                    .await;
                }
            }
            Args::Clear => {
                Cache::Delete(key).exec(ctx.cache).await?;

                Response {
                    platform: ctx.platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(Ping {
                        pinger: None,
                        pingee: ctx.user.clone(),
                        msg: Some("Items cleared".to_owned().into()),
                        meta: ctx.meta.clone(),
                    }),
                }
                .send(ctx.location.clone(), ctx.resp)
                .await;
            }
        }

        Ok(RunRes::Ok)
    }
}

impl CmdDesc for MemeBank {
    #[inline]
    fn platform(&self) -> Platform {
        Platform::DISCORD
    }
}

impl Invokable for MemeBank {
    fn args(&self, platform: Platform) -> Vec<Arg> {
        if platform != Platform::DISCORD {
            return vec![];
        }

        let mut edit_subcmds = vec![
            Arg {
                name: "remove".into(),
                desc: "Remove a meme".into(),
                kind: ArgKind::SubCommand(vec![Arg {
                    name: "search".into(),
                    desc: "Search term".into(),
                    kind: ArgKind::Autocomplete,
                    optional: false,
                }]),
                optional: true,
            },
            Arg {
                name: "rename".into(),
                desc: "Rename a meme".into(),
                kind: ArgKind::SubCommand(vec![
                    Arg {
                        name: "search".into(),
                        desc: "Search term".into(),
                        kind: ArgKind::Autocomplete,
                        optional: false,
                    },
                    Arg {
                        name: "name".into(),
                        desc: "New name".into(),
                        kind: ArgKind::String,
                        optional: false,
                    },
                ]),
                optional: true,
            },
        ];

        // convenience methods
        if self.scrape_attachments {
            edit_subcmds.extend_from_slice(&[
                Arg {
                    name: "remove-last".into(),
                    desc: "Remove the last saved meme".into(),
                    kind: ArgKind::SubCommand(vec![]),
                    optional: true,
                },
                Arg {
                    name: "rename-last".into(),
                    desc: "Rename the last saved meme".into(),
                    kind: ArgKind::SubCommand(vec![Arg {
                        name: "name".into(),
                        desc: "New name".into(),
                        kind: ArgKind::String,
                        optional: false,
                    }]),
                    optional: true,
                },
            ])
        }

        let edit_subcmds = edit_subcmds;

        vec![
            Arg {
                name: "get".into(),
                desc: "Get a meme".into(),
                kind: ArgKind::SubCommand(vec![Arg {
                    name: "search".into(),
                    desc: "Search term".into(),
                    kind: ArgKind::Autocomplete,
                    optional: false,
                }]),
                optional: true,
            },
            Arg {
                name: "list".into(),
                desc: "List all memes".into(),
                kind: ArgKind::SubCommand(vec![]),
                optional: true,
            },
            Arg {
                name: "edit".into(),
                desc: "Rename/remove a saved meme".into(),
                kind: ArgKind::SubCommandGroup(edit_subcmds),
                optional: true,
            },
            Arg {
                name: "add".into(),
                desc: "Manually save a meme".into(),
                kind: ArgKind::SubCommand(vec![
                    Arg {
                        name: "link".into(),
                        desc: "Link to the embed (must be a discord link)".into(),
                        kind: ArgKind::String,
                        optional: false,
                    },
                    Arg {
                        name: "name".into(),
                        desc: "Name".into(),
                        kind: ArgKind::String,
                        optional: false,
                    },
                ]),
                optional: true,
            },
            Arg {
                name: "clear".into(),
                desc: "Clear memes".into(),
                kind: ArgKind::SubCommand(vec![]),
                optional: true,
            },
        ]
    }

    fn hidden(&self, _platform: Platform) -> bool {
        true
    }
}

impl TryFrom<&ArgMap> for Args {
    type Error = ArgMapError;

    fn try_from(value: &ArgMap) -> Result<Self, Self::Error> {
        if let Some(ArgValue::SubCommand(c)) = value.get("get") {
            let search = match c.get("search") {
                Some(ArgValue::String(x)) => x.to_owned(),
                _ => return Err(ArgMapError),
            };
            Ok(Args::Search(search))
        } else if let Some(ArgValue::SubCommand(_c)) = value.get("list") {
            Ok(Args::List)
        } else if let Some(ArgValue::SubCommand(c)) = value.get("edit") {
            if let Some(ArgValue::SubCommand(c)) = c.get("remove") {
                let search = match c.get("search") {
                    Some(ArgValue::String(x)) => x.to_owned(),
                    _ => return Err(ArgMapError),
                };
                Ok(Args::EditSearch { search, name: None })
            } else if let Some(ArgValue::SubCommand(c)) = c.get("rename") {
                let search = match c.get("search") {
                    Some(ArgValue::String(x)) => x.to_owned(),
                    _ => return Err(ArgMapError),
                };
                let name = match c.get("name") {
                    Some(ArgValue::String(x)) => x.to_owned(),
                    _ => return Err(ArgMapError),
                };
                Ok(Args::EditSearch {
                    search,
                    name: Some(name),
                })
            } else if let Some(ArgValue::SubCommand(_c)) = c.get("remove-last") {
                Ok(Args::EditLast { name: None })
            } else if let Some(ArgValue::SubCommand(c)) = c.get("rename-last") {
                let name = match c.get("name") {
                    Some(ArgValue::String(x)) => x.to_owned(),
                    _ => return Err(ArgMapError),
                };
                Ok(Args::EditLast { name: Some(name) })
            } else {
                Err(ArgMapError)
            }
        } else if let Some(ArgValue::SubCommand(c)) = value.get("add") {
            let link = match c.get("link") {
                Some(ArgValue::String(x)) => x.to_owned(),
                _ => return Err(ArgMapError),
            };
            let name = match c.get("name") {
                Some(ArgValue::String(x)) => x.to_owned(),
                _ => return Err(ArgMapError),
            };
            Ok(Args::Add {
                link,
                name,
                silent: false,
            })
        } else if let Some(ArgValue::SubCommand(_c)) = value.get("clear") {
            Ok(Args::Clear)
        } else {
            Err(ArgMapError)
        }
    }
}
