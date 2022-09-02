use crate::msg::CommandCache;
use back::{
    cmds::ArgValue,
    msg::{
        self, Chat, ChatMeta, Invocation, InvocationKind, Location, Payload, Permissions, Ping,
        Platform, Response, StreamEvent, User,
    },
    CHANNEL_NAME,
};
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use regex::Regex;
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    http::Http,
    model::{
        self,
        channel::{Channel, Message},
        gateway::{ActivityType, Presence, Ready},
        interactions::{
            application_command::{
                ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
                ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
            },
            autocomplete::AutocompleteInteraction,
        },
        prelude::*,
    },
};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, oneshot};
use tracing::info_span;
use tracing::Instrument;

pub(crate) static OWNER_ID: Lazy<UserId> = Lazy::new(|| {
    dotenv::var("STREAMER_ID")
        .unwrap()
        .parse::<UserId>()
        .unwrap_or_default()
});
static AUSSIEBOT_ID: Lazy<UserId> = Lazy::new(|| {
    dotenv::var("AUSSIEBOT_ID")
        .unwrap()
        .parse::<UserId>()
        .unwrap_or_default()
});
pub(crate) static GUILD_ID: Lazy<GuildId> = Lazy::new(|| {
    GuildId(
        dotenv::var("GUILD_ID")
            .unwrap()
            .parse::<u64>()
            .unwrap_or_default(),
    )
});
static MEMBER_ROLE_ID: Lazy<RoleId> = Lazy::new(|| {
    dotenv::var("MEMBER_ROLE_ID")
        .unwrap()
        .parse::<RoleId>()
        .unwrap_or_default()
});

const MEE6_ID: UserId = UserId(159985870458322944);
const EINLLAMA_ID: UserId = UserId(624224573176545288);

// for debouncing spurious status changes
const STATUS_DEBOUNCE_SECS: u64 = 8;

static PING_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)\spinged\syou\sfrom\s(\S+)'s\s(\S+)(?:!|:)").unwrap());
static PING_DISC_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)\s\(<@!?(\d+)>\)\spinged\syou").unwrap());
static URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://(?:www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b(?:[-a-zA-Z0-9()@:%_\+.~#?&/=]*)").unwrap()
});

#[derive(Clone)]
pub(crate) struct Handler {
    pub(crate) msg_out_tx: mpsc::Sender<(Location, Response)>,
    pub(crate) was_streaming: Arc<AtomicBool>,
    pub(crate) stream_url: Arc<Mutex<Arc<String>>>,
    pub(crate) cancel_chan: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pub(crate) stream_announced: Arc<AtomicBool>,
    pub(crate) mee6_last_url: Arc<Mutex<Arc<String>>>,
    pub(crate) cmd_cache: Arc<RwLock<Option<CommandCache>>>,
    pub(crate) streamer_id: Arc<RwLock<UserId>>,
}

impl Handler {
    fn default_activity() -> Option<Activity> {
        Some(Activity::playing("with deez nuts"))
    }
}

#[async_trait]
impl EventHandler for Handler {
    #[tracing::instrument(skip_all, fields(author, guild))]
    async fn message(&self, ctx: Context, msg: Message) {
        match msg.author.id {
            MEE6_ID => {
                //println!("{}", Local::now());

                self.handle_mee6(&ctx, &msg).await;
                return;
            }
            EINLLAMA_ID => {
                self.handle_mee6(&ctx, &msg).await;
            }
            x if x == *AUSSIEBOT_ID => {
                self.handle_aussiebot(&ctx, &msg);
                return;
            }
            _ => {}
        }

        tracing::Span::current().record("author", &msg.author.name.as_str());
        tracing::Span::current().record("guild", &&*format!("{:?}", msg.guild_id));

        if let Some(ref referenced_msg) = msg.referenced_message {
            // check if aussiebot sent the orig msg
            if referenced_msg.author.id == *AUSSIEBOT_ID {
                self.handle_reply(&ctx, &msg).await;
            }
        }

        #[allow(clippy::match_single_binding)]
        match msg.guild_id {
            _ => {
                //Some(id) if id == *GUILD_ID => {
                // convert Message to Chat
                let chat = from_message(msg, &ctx).await;

                tracing::info!("relaying chat");

                // send ok to dumper
                Response {
                    platform: Platform::DISCORD,
                    channel: &*CHANNEL_NAME,
                    payload: Payload::Chat(chat),
                }
                .send(Location::Pubsub, &self.msg_out_tx)
                .await;
            } //_ => {}
        }
    }

    //#[tracing::instrument(skip_all, fields(was_streaming, is_streaming))]
    async fn presence_update(&self, ctx: Context, new_data: Presence) {
        //println!("presence_update: {:?}", new_data);
        // check if streamer
        {
            if new_data.user.id != *self.streamer_id.read() {
                return;
            }
        }

        // is there at least one streaming-related activity?
        let is_streaming = new_data
            .activities
            .iter()
            .find(|activity| (activity.kind == ActivityType::Streaming));

        // get the stream's url if any
        let (is_streaming, stream_url, stream_name) = if let Some(act) = is_streaming {
            (true, act.url.as_ref(), act.name.to_owned())
        } else {
            (false, None, "".to_owned())
        };

        // read previous stream state
        let was_streaming = self.was_streaming.load(Ordering::Acquire);

        if !was_streaming && (!is_streaming || stream_url.is_none()) {
            // not streaming -> not streaming
            // or ignore possibly empty stream url
            return;
        }

        tracing::info!(
            was_streaming = was_streaming,
            is_streaming = is_streaming,
            "Streamer's presence changed"
        );

        if !was_streaming {
            // not streaming -> streaming
            // abort cancel task if any
            if let Some(chan) = self.cancel_chan.lock().take() {
                let _ = chan.send(());
            }
            let new_url = Arc::new(stream_url.unwrap().to_string());
            tracing::info!(new_url = new_url.as_str(), "not streaming -> streaming");
            // update stream state
            *self.stream_url.lock() = new_url.clone();
            self.was_streaming.store(true, Ordering::Release);

            // update presence
            let act_fut = ctx.set_presence(
                Some(Activity::streaming(stream_name, &*new_url)),
                OnlineStatus::Online,
            );

            // send stream detection event
            let resp_fut = Response {
                platform: Platform::DISCORD,
                channel: &*CHANNEL_NAME,
                payload: Payload::StreamEvent(StreamEvent::DetectStart(new_url)),
            }
            .send(Location::Pubsub, &self.msg_out_tx);

            tokio::join!(act_fut, resp_fut);
        } else if !is_streaming {
            // streaming -> not streaming
            tracing::info!("streaming -> not streaming");
            // abort cancel task if any
            if let Some(chan) = self.cancel_chan.lock().take() {
                let _ = chan.send(());
            }
            // spawn cancel task
            let (tx, cancel_chan) = oneshot::channel::<()>();
            *self.cancel_chan.lock() = Some(tx);
            let prev_url = self.stream_url.lock().clone();
            let was_streaming = self.was_streaming.clone();
            let msg_out_tx = self.msg_out_tx.clone();
            let stream_announced = self.stream_announced.clone();
            //println!("SPAWNING CANCEL TASK");
            tracing::info!("SPAWNING CANCEL TASK");
            tokio::spawn(
                async move {
                    let sleep =
                        tokio::time::sleep(std::time::Duration::from_secs(STATUS_DEBOUNCE_SECS));
                    tokio::pin!(sleep);
                    loop {
                        tokio::select! {
                          _ = &mut sleep => break,
                          _ = cancel_chan => return
                        }
                    }
                    //println!("IN CANCEL TASK");
                    tracing::info!("IN CANCEL TASK");
                    // update stream state
                    stream_announced.store(false, Ordering::Release);
                    was_streaming.store(false, Ordering::Release);

                    // update presence
                    let act_fut = ctx.set_presence(Self::default_activity(), OnlineStatus::Online);

                    // send stream stop event
                    let resp_fut = Response {
                        platform: Platform::DISCORD,
                        channel: &*CHANNEL_NAME,
                        payload: Payload::StreamEvent(StreamEvent::DetectStop(prev_url)),
                    }
                    .send(Location::Pubsub, &msg_out_tx);

                    tokio::join!(resp_fut, act_fut);
                }
                .instrument(info_span!("Cancel Task")),
            );
        } else {
            // streaming -> streaming
            // abort any cancel tasks
            if let Some(chan) = self.cancel_chan.lock().take() {
                let _ = chan.send(());
            }
            let prev_url = self.stream_url.lock().clone();
            tracing::info!(stream_url = ?stream_url, prev_url = prev_url.as_str(), "streaming -> streaming");
            match stream_url {
                Some(url) if url.as_str() != prev_url.as_str() => {
                    // stream changed while streamer mode was still on
                    tracing::info!("Stream url changed");
                    // update stream state
                    let new_url = Arc::new(url.to_string());
                    *self.stream_url.lock() = new_url.clone();
                    self.stream_announced.store(false, Ordering::Release);

                    // update presence
                    let act_fut = ctx.set_presence(
                        Some(Activity::streaming(stream_name, &*new_url)),
                        OnlineStatus::Online,
                    );

                    // send stream detection event
                    let resp_fut = Response {
                        platform: Platform::DISCORD,
                        channel: &*CHANNEL_NAME,
                        payload: Payload::StreamEvent(StreamEvent::DetectStart(new_url)),
                    }
                    .send(Location::Pubsub, &self.msg_out_tx);

                    tokio::join!(act_fut, resp_fut);
                }
                _ => {}
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!("{} is connected!", ready.user.name);
        // announce startup here
        let resp_fut = Response {
            platform: Platform::DISCORD,
            channel: &*CHANNEL_NAME,
            payload: Payload::NotifyStart,
        }
        .send(Location::Pubsub, &self.msg_out_tx);

        // update presence
        let act_fut = ctx.set_presence(Self::default_activity(), OnlineStatus::Online);

        tokio::join!(resp_fut, act_fut);
    }

    #[tracing::instrument(skip_all, fields(author))]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        tracing::debug!(interaction=?interaction);

        match interaction {
            Interaction::ApplicationCommand(command) => {
                self.application_command(command, &ctx.http).await;
            }
            Interaction::Autocomplete(ac) => {
                tracing::Span::current().record("author", &ac.user.name.as_str());
                self.autocomplete(ac, &ctx.http).await;
            }
            _ => return,
        }
    }

    async fn reaction_add(&self, _ctx: Context, add_reaction: Reaction) {
        self.handle_reaction(add_reaction, true).await;
    }

    async fn reaction_remove(&self, _ctx: Context, removed_reaction: Reaction) {
        //tracing::debug!("{:?}", _removed_reaction);
        self.handle_reaction(removed_reaction, false).await;
    }

    #[tracing::instrument(skip(self, _ctx))]
    async fn reaction_remove_all(
        &self,
        _ctx: Context,
        _channel_id: ChannelId,
        _removed_from_message_id: MessageId,
    ) {
        //self.handle_reaction(removed_reaction, false).await;
        //tracing::debug!(_channel_id=%_channel_id, "{:?}", _removed_from_message_id);
    }
}

fn _parse_opt(opt: &ApplicationCommandInteractionDataOption) -> Option<(String, ArgValue)> {
    match opt.kind {
        ApplicationCommandOptionType::SubCommand
        | ApplicationCommandOptionType::SubCommandGroup => {
            //tracing::debug!("SUBCOMMAND(GROUP): {:#?}", opt);
            let args: HashMap<String, ArgValue> =
                HashMap::from_iter(opt.options.iter().filter_map(_parse_opt));
            //tracing::debug!("SUBCOMMAND(GROUP) argmap: {:#?}", args);
            return Some((opt.name.clone(), ArgValue::SubCommand(args)));
        }
        _ => {}
    }
    opt.resolved.as_ref().map(|_opt| {
        let ret = match _opt {
            ApplicationCommandInteractionDataOptionValue::String(s) => ArgValue::String(s.clone()),
            ApplicationCommandInteractionDataOptionValue::Integer(i) => ArgValue::Integer(*i),
            ApplicationCommandInteractionDataOptionValue::Boolean(b) => ArgValue::Bool(*b),
            ApplicationCommandInteractionDataOptionValue::User(user, maybe_member) => {
                let perms = if let Some(member) = maybe_member {
                    perms_from_partial_member(member)
                } else {
                    Permissions::NONE
                };

                ArgValue::User(User {
                    id: user.id.to_string().into(),
                    name: user.name.clone().into(),
                    perms,
                })
            }
            _ => unimplemented!(),
        };

        (opt.name.clone(), ret)
    })
}

impl Handler {
    #[tracing::instrument(skip_all, fields(author=command.user.name.as_str()))]
    async fn application_command(&self, command: ApplicationCommandInteraction, http: &Arc<Http>) {
        let prefix = command.data.name.to_owned();

        // we need to decide now if resp is hidden or not, so query cmd cache
        let ephemeral = {
            let cmd_cache = self.cmd_cache.read();
            if cmd_cache.is_none() {
                return;
            }
            match cmd_cache.as_ref().unwrap().get(&prefix) {
                Some((_, ephemeral, _, _)) => *ephemeral,
                _ => return,
            }
        };

        let args: HashMap<String, ArgValue> =
            HashMap::from_iter(command.data.options.iter().filter_map(_parse_opt));

        // send back token as meta
        let perms = perms_from_maybe_member(command.member.as_ref());

        let name = command.user.tag();

        let user = User {
            id: command.user.id.to_string().into(),
            name: name.into(),
            perms,
        };

        let is_dm = command
            .channel_id
            .to_channel(http)
            .await
            .map(|chan| match chan {
                Channel::Guild(_c) => false,
                Channel::Private(_c) => true,
                Channel::Category(_c) => false,
                _ => unimplemented!(),
            })
            .unwrap_or(false);

        let defer_fut = command.create_interaction_response(http, |f| {
            f.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                .interaction_response_data(|f| f.ephemeral(ephemeral))
        });

        let resp_fut = Response {
            platform: Platform::DISCORD,
            channel: &*CHANNEL_NAME,
            payload: Payload::InvokeCommand(Invocation {
                user: user.into(),
                cmd: prefix.into(),
                args,
                meta: Some(ChatMeta::DiscordInteraction(
                    command.token.to_owned().into(),
                    command.id.0,
                    ephemeral,
                    is_dm,
                )),
                kind: None,
            }),
        }
        .send(Location::Pubsub, &self.msg_out_tx);

        let (defer_res, _) = tokio::join!(defer_fut, resp_fut);

        if let Err(why) = defer_res {
            tracing::error!(why=%why,"Couldn't respond to slash command");
        }
    }

    // TODO: DRY
    #[tracing::instrument(skip_all, fields(author=command.user.name.as_str()))]
    async fn autocomplete(&self, command: AutocompleteInteraction, http: &Arc<Http>) {
        let prefix = command.data.name.to_owned();

        // we need to decide now if resp is hidden or not, so query cmd cache
        let ephemeral = {
            let cmd_cache = self.cmd_cache.read();
            if cmd_cache.is_none() {
                return;
            }
            match cmd_cache.as_ref().unwrap().get(&prefix) {
                Some((_, ephemeral, _, _)) => *ephemeral,
                _ => return,
            }
        };

        let args: HashMap<String, ArgValue> =
            HashMap::from_iter(command.data.options.iter().filter_map(_parse_opt));

        // send back token as meta
        let perms = perms_from_maybe_member(command.member.as_ref());

        let name = command.user.tag();

        let user = User {
            id: command.user.id.to_string().into(),
            name: name.into(),
            perms,
        };

        let is_dm = command
            .channel_id
            .to_channel(http)
            .await
            .map(|chan| match chan {
                Channel::Guild(_c) => false,
                Channel::Private(_c) => true,
                Channel::Category(_c) => false,
                _ => unimplemented!(),
            })
            .unwrap_or(false);

        Response {
            platform: Platform::DISCORD,
            channel: &*CHANNEL_NAME,
            payload: Payload::InvokeCommand(Invocation {
                user: user.into(),
                cmd: prefix.into(),
                args,
                meta: Some(ChatMeta::DiscordInteraction(
                    command.token.to_owned().into(),
                    command.id.0,
                    ephemeral,
                    is_dm,
                )),
                kind: Some(InvocationKind::Autocomplete),
            }),
        }
        .send(Location::Pubsub, &self.msg_out_tx)
        .await;
    }

    #[tracing::instrument(skip_all, fields(new_last_url))]
    async fn handle_mee6(&self, _ctx: &Context, msg: &Message) -> Option<()> {
        let captures = URL_REGEX.captures(&msg.content)?;
        let url = Arc::new(captures[0].to_owned());
        *self.mee6_last_url.lock() = url.clone();

        tracing::Span::current().record("new_last_url", &url.as_str());

        Response {
            platform: Platform::DISCORD,
            channel: &*CHANNEL_NAME,
            payload: Payload::StreamEvent(StreamEvent::DetectStart(url)),
        }
        .send(Location::Pubsub, &self.msg_out_tx)
        .await;
        None
    }

    fn handle_aussiebot(&self, _ctx: &Context, _msg: &Message) -> Option<()> {
        None
    }

    #[tracing::instrument(skip_all, ret)]
    async fn handle_reply(&self, ctx: &Context, msg: &Message) -> Option<()> {
        let referenced_msg = msg.referenced_message.as_ref().unwrap();
        let orig_msg = &referenced_msg.content;

        // check if it was a ping response
        let (pingee_name, platform, pingee_id) =
            if let Some(captures) = PING_REGEX.captures(orig_msg) {
                (
                    Arc::new(captures[1].to_owned()),
                    Platform::from_str(&captures[3]).ok()?,
                    "".to_owned().into(),
                )
            } else if let Some(captures) = PING_DISC_REGEX.captures(orig_msg) {
                (
                    Arc::new(captures[1].to_owned()),
                    Platform::DISCORD,
                    Arc::new(captures[2].to_owned()),
                )
            } else {
                return None;
            };
        let pinger_nick = msg
            .author_nick(&ctx.http)
            .await
            .unwrap_or_else(|| msg.author.tag());
        let pinger_id = msg.author.id.to_string();

        tracing::info!(
          platform = %platform,
            content = msg.content.as_str(),
            pingee_name = pingee_name.as_str(),
            "sending",
        );

        Response {
            platform,
            channel: &*CHANNEL_NAME,
            payload: Payload::Ping(Ping {
                pinger: Some((
                    Platform::DISCORD,
                    Arc::new(User {
                        id: Arc::new(pinger_id),
                        name: Arc::new(pinger_nick),
                        perms: Permissions::NONE,
                    }),
                )),
                pingee: Arc::new(User {
                    id: pingee_id,
                    name: pingee_name,
                    perms: Permissions::NONE,
                }),
                msg: Some(msg.content_safe(&ctx.cache).into()),
                meta: None,
            }),
        }
        .send(Location::Broadcast, &self.msg_out_tx)
        .await;

        Some(())
    }

    #[tracing::instrument(skip(self))]
    async fn handle_reaction(&self, reaction: Reaction, is_add: bool) {
        let user_id = match reaction.user_id {
            Some(u) => u.to_string(),
            None => return,
        };

        let user = User {
            id: user_id.into(),
            name: "".to_owned().into(),
            perms: Permissions::NONE,
        };

        let emoji = match reaction.emoji {
            ReactionType::Custom { id, .. } => id.to_string(),
            ReactionType::Unicode(emoji) => emoji,
            _ => todo!(),
        };

        let message_id = reaction.message_id.to_string();

        let cmd = if is_add {
            "@reaction_add"
        } else {
            "@reaction_rem"
        }
        .to_owned()
        .into();

        Response {
            platform: Platform::DISCORD,
            channel: &*CHANNEL_NAME,
            payload: Payload::InvokeCommand(Invocation {
                user: user.into(),
                cmd,
                args: HashMap::with_capacity(0),
                meta: reaction
                    .guild_id
                    .map(|id| ChatMeta::Discord4(id.to_string().into())),
                kind: Some(InvocationKind::Reaction { message_id, emoji }),
            }),
        }
        .send(Location::Pubsub, &self.msg_out_tx)
        .await;
    }
}

static EMOJI_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"<a?:([^<>:]+):(?:\d+)>").unwrap());

#[tracing::instrument(skip_all, ret)]
async fn from_message(msg: Message, ctx: &Context) -> Chat {
    let _content = msg.content_safe(&ctx.cache);
    let content = EMOJI_REGEX.replace_all(&_content, ":$1:"); // clean up emojis

    //let _guild_name = msg.guild_field(&ctx.cache, |g| g.name.to_owned());

    let perms = perms_from_msg(&msg, ctx).await;

    // let nick = msg
    //     .author_nick(&ctx.http)
    //     .await
    //     .unwrap_or_else(|| msg.author.tag());
    let author_tag = msg.author.tag();

    let Message {
        attachments,
        sticker_items,
        ..
    } = msg;

    let att_data = Vec::from_iter(attachments.into_iter().map(|att| (att.filename, att.url)));
    //.collect::<Vec<(String, String)>>();

    let stk_names = Vec::from_iter(sticker_items.into_iter().map(|stk| stk.name));
    //.collect::<Vec<String>>();

    let channel = msg
        .channel_id
        .to_channel(&ctx.http)
        .await
        .map(|chan| match chan {
            Channel::Guild(c) => (c.id, c.name, false),
            Channel::Private(c) => (c.id, "DMs".into(), /*true*/ false),
            Channel::Category(c) => (c.id, c.name, false),
            _ => unimplemented!(),
        });

    let channel = channel.map(|(cid, cname, dms)| (cid, Arc::new(cname), dms));

    tracing::info!(channel=?channel, content=%content);

    let meta = match (channel, att_data.is_empty(), stk_names.is_empty()) {
        // (Ok((_cid, _cname, true)), true, true) => {
        //     Some(ChatMeta::DiscordDM(Arc::new(att_data), Arc::new(stk_names)))
        // }
        (Ok((cid, cname, _)), true, true) => Some(ChatMeta::Discord1(cid.into(), cname)),
        (Ok((cid, cname, _)), false, false)
        | (Ok((cid, cname, _)), true, false)
        | (Ok((cid, cname, _)), false, true) => Some(ChatMeta::Discord2(
            cid.into(),
            cname,
            Arc::new(att_data),
            Arc::new(stk_names),
        )),
        (_, false, false) | (_, true, false) | (_, false, true) => {
            Some(ChatMeta::Discord3(Arc::new(att_data), Arc::new(stk_names)))
        }
        _ => None,
    };

    Chat {
        user: Arc::new(User {
            id: Arc::new(msg.author.id.to_string()),
            name: Arc::new(author_tag),
            perms,
        }),
        msg: Arc::new(content.to_string()),
        meta,
    }
}

async fn perms_from_msg(msg: &Message, ctx: &Context) -> Permissions {
    if msg.author.id == *OWNER_ID {
        return Permissions::OWNER;
    }

    let member = if let Some(guild) = GUILD_ID.to_guild_cached(&ctx.cache) {
        if msg.author.id == guild.owner_id {
            return Permissions::OWNER;
        }
        guild.member(&ctx.http, msg.author.id).await
    } else {
        msg.member(&ctx.http).await
    };

    if let Ok(member) = member {
        if let Ok(perms) = member.permissions(&ctx.cache) {
            if perms.contains(model::Permissions::ADMINISTRATOR) {
                Permissions::ADMIN
            } else if perms
                .intersects(model::Permissions::MODERATE_MEMBERS | model::Permissions::KICK_MEMBERS)
            {
                Permissions::MOD
            } else if member.roles.contains(&*MEMBER_ROLE_ID) {
                Permissions::MEMBER
            } else {
                Permissions::NONE
            }
        } else if member.roles.contains(&*MEMBER_ROLE_ID) {
            Permissions::MEMBER
        } else {
            Permissions::NONE
        }
    } else {
        Permissions::NONE
    }
}

pub(crate) trait FromPerms {
    fn from_perms(perms: &msg::Permissions, default: model::Permissions) -> Self;
}

impl FromPerms for model::Permissions {
    fn from_perms(perms: &msg::Permissions, default: model::Permissions) -> Self {
        if perms.contains(msg::Permissions::OWNER) || perms.contains(msg::Permissions::ADMIN) {
            model::Permissions::ADMINISTRATOR
        } else if perms.contains(msg::Permissions::MOD) {
            model::Permissions::KICK_MEMBERS
        } else {
            default
        }
    }
}

fn perms_from_maybe_member(maybe_member: Option<&Member>) -> msg::Permissions {
    if let Some(member) = maybe_member {
        if let Some(perms) = member.permissions {
            if perms.contains(model::Permissions::ADMINISTRATOR) {
                Permissions::ADMIN
            } else if perms
                .intersects(model::Permissions::MODERATE_MEMBERS | model::Permissions::KICK_MEMBERS)
            {
                Permissions::MOD
            } else if member.roles.contains(&*MEMBER_ROLE_ID) {
                Permissions::MEMBER
            } else {
                Permissions::NONE
            }
        } else if member.roles.contains(&*MEMBER_ROLE_ID) {
            Permissions::MEMBER
        } else {
            Permissions::NONE
        }
    } else {
        Permissions::NONE
    }
}

fn perms_from_partial_member(member: &PartialMember) -> msg::Permissions {
    if let Some(perms) = member.permissions {
        if perms.contains(model::Permissions::ADMINISTRATOR) {
            Permissions::ADMIN
        } else if perms
            .intersects(model::Permissions::MODERATE_MEMBERS | model::Permissions::KICK_MEMBERS)
        {
            Permissions::MOD
        } else if member.roles.contains(&*MEMBER_ROLE_ID) {
            Permissions::MEMBER
        } else {
            Permissions::NONE
        }
    } else if member.roles.contains(&*MEMBER_ROLE_ID) {
        Permissions::MEMBER
    } else {
        Permissions::NONE
    }
}
