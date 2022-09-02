use crate::discord::{Handler, GUILD_ID};
use back::{
    cmds::{Arg, ArgKind, ArgsDump, ModAction},
    msg::{
        self, discord::DiscordAction, ChatMeta, Location, Message, Payload, Permissions, Ping,
        Platform, Response, User, PLATFORMS,
    },
    pubsub, CHANNEL_NAME,
};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serenity::{
    builder::{
        CreateApplicationCommandOption, CreateAutocompleteResponse, EditInteractionResponse,
    },
    json::{self, Value},
    model::{
        self,
        id::{ChannelId, RoleId, UserId},
        interactions::application_command::{
            ApplicationCommand, ApplicationCommandOptionType, ApplicationCommandType,
        },
        Timestamp,
    },
    utils::MessageBuilder,
    CacheAndHttp,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{sync::mpsc, task::JoinHandle};

pub(crate) type CommandCache = HashMap<String, (String, bool, msg::Permissions, Vec<Arg>)>;

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct Server {
    pub(crate) pub_in_tx: mpsc::Sender<pubsub::Msg>, // redis <- msg resp
    pub(crate) msg_out_tx: mpsc::Sender<(Location, Response)>,
    pub(crate) handler: Handler,
    pub(crate) cache: Arc<CacheAndHttp>,
    pub(crate) cmd_cache: Arc<RwLock<Option<CommandCache>>>,
}

static LLAMA_PING: Lazy<Arc<User>> = Lazy::new(|| {
    Arc::new(User {
        id: "624224573176545288".to_owned().into(),
        name: "".to_owned().into(),
        perms: Permissions::ADMIN,
    })
});

static STREAM_ANNOUNCE_CHAN_ID: Lazy<ChannelId> = Lazy::new(|| {
    dotenv::var("STREAM_ANNOUNCE_CHAN_ID")
        .unwrap()
        .parse::<ChannelId>()
        .unwrap_or_default()
});
static BOT_CHAN_ID: Lazy<ChannelId> = Lazy::new(|| {
    dotenv::var("BOT_CHAN_ID")
        .unwrap()
        .parse::<ChannelId>()
        .unwrap_or_default()
});

impl Server {
    // TODO: generalise chans
    #[tracing::instrument(skip_all)]
    async fn msg(&self, msg: Message, _: Location) {
        tracing::info!("\x1b[93mMessage received\x1b[0m");

        let Message {
            platform,
            channel,
            payload,
        } = msg;

        // Discord is a UI platform, it receives all and checks platform applicability for each payload type
        if channel.as_str() != CHANNEL_NAME.as_str() {
            return;
        }

        match payload {
            Payload::Autocomplete(ac) => {
                let (token, id) = match ac.meta {
                    Some(ChatMeta::DiscordInteraction(token, id, _, _)) => (token, id),
                    _ => return,
                };

                let mut response = CreateAutocompleteResponse::default();

                // max 25 autocomplete options
                for (key, value) in ac.choices.into_iter().take(25) {
                    response.add_string_choice(key, value);
                }

                let data = json::hashmap_to_json_map(response.0);

                // Autocomplete response type is 8
                let map = serde_json::json!({
                    "type": 8,
                    "data": data,
                });

                let res = self
                    .cache
                    .http
                    .as_ref()
                    .create_interaction_response(id, &token, &map)
                    .await;
                if let Err(why) = res {
                    tracing::error!(why=?why,"Error sending autocomplete choices");
                }
            }
            // a Message should be visible
            Payload::Message { user, msg, meta } if platform.contains(Platform::DISCORD) => {
                tracing::info!(user = ?user, msg = msg.as_str(), meta = ?meta, "Payload::Message");
                let msg = match user {
                    Some((Platform::DISCORD, user)) => {
                        let new_msg = format!("<@{}> {}", user.id, msg);
                        Arc::new(new_msg)
                    }
                    Some((platform, user)) => {
                        let new_msg = format!("{} ({}) {}", user.name, platform, msg);
                        Arc::new(new_msg)
                    }
                    _ => msg,
                };

                let mut was_interaction = false;
                let mut was_shown = false;

                if let Some(ChatMeta::DiscordInteraction(ref token, _, ephemeral, _is_dm)) = meta {
                    // resolve interaction
                    tracing::debug!(token = %token, "editing original interaction response");

                    was_interaction = true;
                    let mut edit = EditInteractionResponse::default();

                    // FIXME: not all messages need to be broadcasted
                    if !ephemeral
                    /*&& !is_dm*/
                    {
                        was_shown = true;
                        edit.content(&msg);
                    } else {
                        edit.content("<:daAussie:829181617322852394>"); // TODO: config
                    }

                    let map = serenity::json::hashmap_to_json_map(edit.0);
                    let res = self
                        .cache
                        .http
                        .edit_original_interaction_response(token, &Value::from(map))
                        .await;
                    if let Err(why) = res {
                        tracing::error!(why=?why,"Error editing orig. interaction resp.");
                    }
                }

                if !was_interaction || !was_shown {
                    // send to relevant channel
                    let channel = match meta {
                        Some(ChatMeta::Discord1(cid, _))
                        | Some(ChatMeta::Discord2(cid, _, _, _)) => {
                            // reply on channel with id `cid`
                            ChannelId(cid)
                        }
                        _ => *BOT_CHAN_ID, // default to preset bot chan
                    };
                    tracing::info!(channel = %channel, "sending message");
                    if let Err(why) = channel.say(&self.cache.http, &msg).await {
                        tracing::error!(why=?why,"Error sending message");
                    }
                }
            }
            Payload::StreamAnnouncement(url, msg) => {
                // backend decides if we announce, but do one last check in case mee6 pings just before backend tells us to announce
                let last_url = self.handler.mee6_last_url.lock().clone();

                tracing::info!(
                    msg = msg.as_str(),
                    last_url = last_url.as_str(),
                    url = url.as_str(),
                    "StreamAnnouncement"
                );

                // if MEE6 pings then equality will hold
                if last_url.as_str() != url.as_str()
                // && self
                //     .handler
                //     .stream_announced
                //     .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                //     .is_ok()
                {
                    tracing::debug!("annoncing");
                    let chan = &*STREAM_ANNOUNCE_CHAN_ID;
                    if let Err(why) = chan.say(&self.cache.http, &msg).await {
                        tracing::error!("Error sending message: {:?}", why);
                    }
                } else {
                    tracing::info!("MEE6 already pinged stream, not announcing");
                }
            }
            Payload::Ping(ping) if platform == Platform::DISCORD => {
                self.ping(ping).await;
            }
            Payload::ConfigChanged => {
                // get new arg schema
                Response {
                    platform: Platform::DISCORD,
                    channel: &*CHANNEL_NAME,
                    payload: Payload::DumpArgs(Platform::DISCORD),
                }
                .send(Location::Pubsub, &self.msg_out_tx)
                .await;
            }
            Payload::ArgsDump(dump) => {
                tracing::info!(dump=?dump,"\x1b[93mArgs schema received\x1b[0m");
                self.args_dump(dump).await;
            }
            Payload::ModAction(user, action, reason) if platform.contains(Platform::DISCORD) => {
                self.mod_action(user, action, reason).await;
            }
            Payload::ModAction(user, action, reason) => {
                // send a debug dm
                self.ping(Ping {
                    pinger: None,
                    pingee: LLAMA_PING.clone(),
                    msg: Some(
                        format!(
                            "Mod action for {}: {:?}\nreason: {}",
                            user.name, action, reason
                        )
                        .into(),
                    ),
                    meta: None,
                })
                .await;
            }
            Payload::Discord(action) => match action {
                DiscordAction::AddRole(inner) => {
                    self.role(inner, true).await;
                }
                DiscordAction::RemoveRole(inner) => {
                    self.role(inner, false).await;
                }
                DiscordAction::StreamerId(streamer_id) => {
                    let id = streamer_id.parse::<UserId>();
                    if let Ok(id) = id {
                        tracing::info!(id=?id, "Setting streamer_id");
                        *self.handler.streamer_id.write() = id;
                    }
                }
            },
            _ => {}
        }
    }

    #[tracing::instrument(skip(self))]
    async fn mod_action(
        &self,
        user: Arc<User>,
        action: ModAction,
        reason: Arc<String>,
    ) -> Option<()> {
        let user_id = user.id.parse::<UserId>().ok()?;
        //let mut member = self.cache.cache.member(*GUILD_ID, user_id)?;

        match action {
            ModAction::None => {}
            ModAction::Warn => {}
            ModAction::Remove => {}
            ModAction::Timeout(duration) => {
                let timestamp_now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_secs()
                    .wrapping_add(duration as u64);
                let time = Timestamp::from_unix_timestamp(timestamp_now.try_into().ok()?).ok()?;
                // TODO
                // if let Err(why) = member
                //     .disable_communication_until_datetime(&self.cache.http, time)
                //     .await
                // {
                //     tracing::error!(why=?why,"Error timing out {}", user_id);
                // }
                tracing::info!("Timed out {} ({}) till {}", user.name, user_id, time);
            }
            ModAction::Kick => {}
            ModAction::Ban => {}
        }

        Some(())
    }

    #[tracing::instrument(skip(self))]
    async fn role(
        &self,
        msg::discord::Role {
            user_id,
            role_id,
            guild_id,
            reason,
        }: msg::discord::Role,
        is_add: bool,
    ) -> Option<()> {
        let user_id = *user_id.parse::<UserId>().ok()?.as_u64();
        let role_id = *role_id.parse::<RoleId>().ok()?.as_u64();
        let guild_id = guild_id
            .and_then(|id| (&*id).parse::<u64>().ok())
            .unwrap_or(*GUILD_ID.as_u64());
        let member = self.cache.cache.member(guild_id, user_id)?;
        let _reason = reason.as_ref().map(|r| r.as_str());
        if is_add {
            // to avoid checking possibly stale cache, use
            // self.cache.http.add_member_role(guild_id, user_id, role_id, audit_log_reason)
            // right now we use the GUILD_MEMBERS intent and the guild_member_update event to update the cache
            // but its privileged, and an extra load on resources
            if member.roles.contains(&RoleId(role_id)) {
                tracing::warn!(roles=?member.roles, "member already has role {}", role_id);
                return None;
            }
            if let Err(why) = self
                .cache
                .http
                .add_member_role(guild_id, user_id, role_id, _reason)
                .await
            {
                tracing::error!("{}", why);
            }
        } else {
            if !member.roles.contains(&RoleId(role_id)) {
                tracing::warn!(roles=?member.roles, "member doesn't have role {}", role_id);
                return None;
            }
            if let Err(why) = self
                .cache
                .http
                .remove_member_role(guild_id, user_id, role_id, _reason)
                .await
            {
                tracing::error!("{}", why);
            }
        }

        Some(())
    }

    fn _create_option<'a>(
        option: &'a mut CreateApplicationCommandOption,
        arg: &Arg,
    ) -> &'a mut CreateApplicationCommandOption {
        option.name(arg.name.as_str());
        option.description(arg.desc.as_str());
        if !arg.optional {
            // don't set param if we don't have to
            option.required(true);
        }
        match arg.kind {
            ArgKind::String => {
                option.kind(ApplicationCommandOptionType::String);
            }
            ArgKind::Integer { min, max } => {
                option.kind(ApplicationCommandOptionType::Integer);
                if let Some(min) = min {
                    option.min_int_value(min);
                }
                if let Some(max) = max {
                    option.max_int_value(max);
                }
            }
            ArgKind::Bool => {
                option.kind(ApplicationCommandOptionType::Boolean);
            }
            ArgKind::User => {
                option.kind(ApplicationCommandOptionType::User);
            }
            ArgKind::Platform => {
                option.kind(ApplicationCommandOptionType::String);
                for platform in PLATFORMS {
                    option.add_string_choice(platform, platform);
                }
            }
            ArgKind::SubCommandGroup(ref subcmds) => {
                option.kind(ApplicationCommandOptionType::SubCommandGroup);
                for cmd in subcmds {
                    // SubCommandGroups can only have SubCommand-type args
                    // TODO: don't panic
                    assert!(matches!(cmd.kind, ArgKind::SubCommand(_)));
                    option.create_sub_option(|subopt| Self::_create_option(subopt, cmd));
                }
            }
            ArgKind::SubCommand(ref subcmds) => {
                option.kind(ApplicationCommandOptionType::SubCommand);
                for cmd in subcmds {
                    // SubCommands cannot contain nested SubCommands nor SubCommandGroups
                    assert!(!matches!(
                        cmd.kind,
                        ArgKind::SubCommandGroup(_) | ArgKind::SubCommand(_)
                    ));
                    option.create_sub_option(|subopt| Self::_create_option(subopt, cmd));
                }
            }
            ArgKind::Autocomplete => {
                option.kind(ApplicationCommandOptionType::String);
                option.set_autocomplete(true);
            }
        }

        option
    }

    async fn args_dump(&self, dump: ArgsDump) {
        use crate::discord::FromPerms;
        // let guild_id = &*GUILD_ID;
        // let _ = guild_id
        //     .set_application_commands(&self.cache.http, |commands| commands)
        //     .await;

        let mut config: CommandCache = HashMap::new();

        for (prefix, description, _hidden, perms, args) in dump {
            // TODO: validation
            // let prefix_len = prefix.floor_char_boundary(32);
            // let prefix = prefix.truncate(prefix_len);

            // trim to 32
            let prefix = match prefix.char_indices().nth(32) {
                None => prefix,
                Some((idx, _)) => (&prefix[..idx]).to_string(),
            };

            // if prefix is repeated, prioritise latest one with non-empty args
            match (args.is_empty(), config.get(&prefix)) {
                (true, Some((_, _, _, _args))) if !_args.is_empty() => {}
                _ => {
                    config.insert(prefix, (description, _hidden, perms, args));
                }
            }
        }
        let config = config;

        let commands =
            ApplicationCommand::set_global_application_commands(&self.cache.http, |commands| {
                for (prefix, (description, _hidden, perms, args)) in &config {
                    commands.create_application_command(|command| {
                        command.name(prefix);
                        command.description(description);
                        command.kind(ApplicationCommandType::ChatInput); // slash cmd
                        command.default_member_permissions(model::Permissions::from_perms(
                            perms,
                            model::Permissions::USE_SLASH_COMMANDS,
                        ));
                        command.dm_permission(true);

                        for arg in args {
                            command.create_option(|option| Self::_create_option(option, arg));
                        }

                        command
                    });
                }
                commands
            })
            .await;

        if let Err(why) = commands {
            tracing::error!(why=%why,"failed to set global slash commands");
        } else {
            let cmd_cache = self.cmd_cache.clone();
            let _ = tokio::task::spawn_blocking(move || {
                *cmd_cache.write() = Some(config);
            })
            .await;
            tracing::info!(commands=?commands,"\x1b[92mglobal slash commands set\x1b[0m");
        }
    }

    #[tracing::instrument(skip(self))]
    async fn ping(&self, ping: Ping) -> Option<()> {
        let Ping {
            pinger,
            pingee: _pingee,
            msg,
            meta,
        } = ping;

        let msg = match (&pinger, msg) {
            (Some((Platform::DISCORD, pinger)), Some(msg)) => MessageBuilder::new()
                .push_line(format!("{} (<@{}>) pinged you:", pinger.name, pinger.id))
                .push_quote_line_safe(msg)
                .push_line("(_reply to respond_)")
                .build(),
            (Some((platform, pinger)), Some(msg)) => MessageBuilder::new()
                .push_line(format!(
                    "{} pinged you from {}'s {}:",
                    pinger.name, &*CHANNEL_NAME, platform
                ))
                .push_quote_line_safe(msg)
                .push_line("(_reply to respond_)")
                .build(),
            (Some((Platform::DISCORD, pinger)), _) => {
                format!(
                    "{} (<@{}>) pinged you!\n(_reply to respond_)",
                    pinger.name, pinger.id
                )
            }
            (Some((platform, pinger)), _) => {
                format!(
                    "{} pinged you from {}'s {}!\n(_reply to respond_)",
                    pinger.name, &*CHANNEL_NAME, platform
                )
            }
            (_, Some(msg)) => (&*msg).to_owned(),
            _ => return None,
        };

        let id = _pingee.id.parse::<UserId>().ok()?;
        let pingee = id.to_user(&self.cache).await.ok()?;

        tracing::info!(id=?id,pingee=?pingee, "sending ping");

        // ping implies privacy
        if let Some(ChatMeta::DiscordInteraction(token, _, ephemeral, is_dm)) = meta {
            tracing::debug!(token = %token, "editing original interaction response after Ping");

            // dms are private anyway
            let ephemeral = ephemeral || is_dm;
            let mut edit = EditInteractionResponse::default();

            match (ephemeral, pinger) {
                (true, Some((Platform::DISCORD, ref user))) if user.id == _pingee.id => {
                    // ephemeral, pingee is pinger
                    // use the interaction itself
                    edit.content(msg);
                }
                (true, None) => {
                    // no explicit pinger, assune internal reply, spawned by pingee
                    edit.content(msg);
                }
                (false, Some((Platform::DISCORD, ref user))) if user.id == _pingee.id => {
                    // non-ephemeral, pingee is pinger
                    // send a dm and update the orig. interaction
                    edit.content("Check DMs");
                    pingee
                        .direct_message(&self.cache, |m| m.content(msg))
                        .await
                        .ok()?;
                }
                (false, None) => {
                    // same as above, but send a dm for privacy
                    edit.content("Check DMs");
                    pingee
                        .direct_message(&self.cache, |m| m.content(msg))
                        .await
                        .ok()?;
                }
                _ => {
                    // pinger isn't the pingee so just update the interaction and ping pingee
                    edit.content(format!("Pinged <@{}>", pingee.id));
                    pingee
                        .direct_message(&self.cache, |m| m.content(msg))
                        .await
                        .ok()?;
                }
            }

            let map = serenity::json::hashmap_to_json_map(edit.0);
            let res = self
                .cache
                .http
                .edit_original_interaction_response(&*token, &Value::from(map))
                .await;

            if let Err(why) = res {
                tracing::error!(why=?why,"failed to edit orig. interaction resp.");
            }
        } else {
            pingee
                .direct_message(&self.cache, |m| m.content(msg))
                .await
                .ok()?;
        }

        Some(())
    }

    // TODO: this is copied from aussiebot_back::msg::Server
    async fn msg_rx_loop(self, mut msg_in_rx: mpsc::Receiver<(Location, String)>) {
        while let Some(msg) = msg_in_rx.recv().await {
            let (loc, msg) = msg;
            //println!("msg recv: {} from {:?}", msg, loc);
            let server = self.clone();
            //tokio::spawn(async move {
            let msg = tokio::task::spawn_blocking(move || {
                let de = serde_json::from_str::<Message>(&msg);
                (msg, de)
            })
            .await;
            match msg {
                Ok((_, Ok(msg))) => {
                    tokio::spawn(async move {
                        server.msg(msg, loc).await;
                    });
                }
                Ok((orig_msg, Err(e))) => {
                    tracing::error!(orig_msg = ?orig_msg, loc = ?loc, "INVALID MSG: {}", e);
                }
                Err(e) => {
                    tracing::error!("{}", e);
                }
            }
            //});
        }
    }

    async fn msg_tx_loop(self, mut msg_out_rx: mpsc::Receiver<(Location, Response)>) {
        while let Some(msg) = msg_out_rx.recv().await {
            let (loc, msg) = msg;
            // serialise msg
            let msg = tokio::task::spawn_blocking(move || serde_json::to_string(&msg)).await;
            if let Ok(Ok(msg)) = msg {
                // TODO: by making an arc we just defer cloning to the edges, i.e before writing out to each ws' stream. pubsub can take a &str, but not ws
                let msg = Arc::new(msg);
                // route accordingly
                match loc {
                    Location::Pubsub | Location::Broadcast => {
                        let _ = self.pub_in_tx.send(msg).await;
                    }
                    _ => unimplemented!(),
                }
            }
        }
    }

    /// Start the server, consuming it
    pub fn start(
        self,
        msg_in_rx: mpsc::Receiver<(Location, String)>,
        msg_out_rx: mpsc::Receiver<(Location, Response)>,
    ) -> JoinHandle<()> {
        tracing::info!("\x1b[92m-------------Starting message loop-------------\x1b[0m");

        // handle response messages
        let server = self.clone();
        tokio::spawn(server.msg_tx_loop(msg_out_rx));

        // process received messages
        tokio::spawn(self.msg_rx_loop(msg_in_rx))
    }
}
