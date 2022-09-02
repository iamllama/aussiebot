pub mod discord;
pub(crate) mod util;

use crate::{
    cache::{self, Cache, RespType},
    cmds::{self, ArgValue, ArgsDump, Command, CommandConfig, ModAction, RunRes, SchemaDump},
    db::{self, modaction::ModActionDump},
    error::{self, Error},
    lock, pubsub, ws,
};
use bb8_redis::redis;
use bitflags::bitflags;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap, fmt::Display, net::SocketAddr, ops::ControlFlow, str::FromStr, sync::Arc,
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

bitflags! {
  pub struct Permissions: u32 {
    const NONE = 1 << 0;
    const MEMBER = 1 << 1;
    const MOD = 1 << 2;
    const ADMIN = 1 << 3;
    const OWNER = 1 << 4;
  }

  pub struct Platform: u32 {
    //const ALL = 0;
    const YOUTUBE = 1 << 0;
    const TWITCH = 1 << 1;
    const DISCORD = 1 << 2;
    const WEB = 1 << 3;
    const STREAM = Self::YOUTUBE.bits | Self::TWITCH.bits;
    const CHAT = Self::STREAM.bits | Self::DISCORD.bits;
    // const UI = Self::WEB.bits;
    const ANNOUNCE = Self::DISCORD.bits | Self::WEB.bits;
  }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::NONE
    }
}

macro_rules! impl_platform_display {
  ($($name:ident $disp:literal),+) => {
    impl Display for Platform {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          match self {
            $(&Platform::$name => {
              write!(f, $disp)
            }),+,
            _ => write!(f, "{:?}", self)
          }
        }
    }
  }
}

#[derive(Debug)]
pub struct PlatformError {
    got: String,
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("invalid platform {:?}", self.got))
    }
}

impl std::error::Error for PlatformError {}

macro_rules! impl_platform_fromstr {
  ($($name:ident $({ $($alt:ident),+ })?),+ $(,)?) => {
    impl FromStr for Platform {
      type Err = PlatformError;

      fn from_str(s: &str) -> Result<Self, Self::Err> {
          match s.as_ref() {
            $(stringify!($name) $($(| stringify!($alt))+)? => Ok(Platform::$name)),+,
            _ => Err(PlatformError { got: s.to_owned() })
          }
      }
    }
  }
}

pub const PLATFORMS: [&str; 3] = ["Youtube", "Discord", "Twitch"];
impl_platform_display!(YOUTUBE "Youtube", DISCORD "Discord", TWITCH "Twitch");
impl_platform_fromstr!(
    YOUTUBE {
        y,
        yt,
        youtube,
        Youtube
    },
    TWITCH {
        t,
        tw,
        twitch,
        Twitch
    },
    DISCORD {
        d,
        disc,
        discord,
        Discord
    },
    WEB
);

pub(crate) const CHAT_PLATFORMS: [Platform; 3] =
    [Platform::YOUTUBE, Platform::DISCORD, Platform::TWITCH];

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct User {
    // https://serde.rs/feature-flags.html#-features-rc
    pub id: Arc<String>,
    pub name: Arc<String>,
    pub perms: Permissions,
}

/// Optional platform-specific metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ChatMeta {
    /// donations
    Youtube(Arc<String>),
    /// chan id, chan name
    Discord1(u64, Arc<String>),
    /// chan id, chan name, attachments (filename,url), stickers
    Discord2(
        u64,
        Arc<String>,
        Arc<Vec<(String, String)>>,
        Arc<Vec<String>>,
    ),
    /// attachments (filename,url), stickers
    Discord3(Arc<Vec<(String, String)>>, Arc<Vec<String>>),
    /// guild id,
    Discord4(Arc<String>),
    /// interaction token, interaction id, ephemeral, is_dm
    DiscordInteraction(Arc<String>, u64, bool, bool),
    // DiscordDM(Arc<Vec<(String, String)>>, Arc<Vec<String>>), // attachments (filename,url), stickers
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub user: Arc<User>,
    pub msg: Arc<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ChatMeta>,
}

pub type ArgMap = HashMap<String, ArgValue>;

#[derive(Debug)]
pub struct ArgMapError;

impl std::fmt::Display for ArgMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid argmap")
    }
}

impl std::error::Error for ArgMapError {}

#[derive(Debug, Serialize, Deserialize)]
pub enum InvocationKind {
    Invoke,
    Autocomplete,
    Reaction { message_id: String, emoji: String },
    StreamEvent(StreamEvent),
    Init,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Invocation {
    pub user: Arc<User>,
    pub cmd: Arc<String>,
    pub args: ArgMap,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ChatMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// None implies InvocationKind::Invoke
    pub kind: Option<InvocationKind>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ping {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinger: Option<(Platform, Arc<User>)>,
    pub pingee: Arc<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<Arc<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ChatMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StreamSignal {
    Start(Arc<String>),
    Stop(Arc<String>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StreamEvent {
    /// A platform has detected a stream start
    DetectStart(Arc<String>),
    /// A chat platform has started following a stream
    Started(Arc<String>, Arc<String>),
    /// A platform has detected a stream stop
    DetectStop(Arc<String>),
    /// A chat platform has stopped following a stream
    Stopped(Arc<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Autocomplete {
    /// key-value autocomplete choices
    pub choices: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ChatMeta>,
}

// TODO: split into recv and resp
#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Payload {
    // recv
    //#[serde(skip_serializing)]
    Chat(Chat),
    InvokeCommand(Invocation),
    // #[serde(skip_serializing)]
    StreamEvent(StreamEvent),
    // TODO: not right
    Ping(Ping),
    // #[serde(skip_serializing)]
    // SetConfig(Vec<cmds::OwnedCmdDump>),
    // #[serde(skip_serializing)]
    DumpConfig,
    // #[serde(skip_serializing)]
    DumpSchema,
    // #[serde(skip_serializing)]
    DumpLog(Platform), // TODO: add an optional arg for max num of latest items
    DumpModActions,
    DumpArgs(Platform),
    //------------------------------
    // send
    // #[serde(skip_deserializing)]
    ConfigSaved,
    // #[serde(skip_deserializing)]
    ConfigChanged,
    // #[serde(skip_deserializing)]
    /// user, action, reason
    ModAction(Arc<User>, ModAction, Arc<String>),
    // #[serde(skip_deserializing)]
    StreamSignal(StreamSignal),
    StreamAnnouncement(Arc<String>, Arc<String>),
    // #[serde(skip_deserializing)]
    /// Aussiebot's replies to users
    Message {
        #[serde(skip_serializing_if = "Option::is_none")]
        user: Option<(Platform, Arc<User>)>,
        msg: Arc<String>, // TODO: arc breaks json string newlines
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<ChatMeta>,
    },
    // #[serde(skip_deserializing)]
    Autocorrect(Arc<User>, Vec<String>),
    #[serde(skip_deserializing)] // SchemaDump has Value refs
    SchemaDump(Arc<SchemaDump>),
    // #[serde(skip_deserializing)]
    LogDump(Vec<(Platform, Vec<String>)>),
    //------------------------------
    // both
    // #[serde(skip_deserializing)]
    ConfigDump(CommandConfig),
    ModActionsDump(ModActionDump),
    ArgsDump(ArgsDump),
    Autocomplete(Autocomplete),
    /// Discord-specific functionality
    Discord(discord::DiscordAction),
    /// Sent when a platform has started and is ready
    NotifyStart,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub platform: Platform,
    pub channel: String,
    pub payload: Payload,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// Responses triggered by a received Message inherit its platform tag.
    /// This generally marks source that triggered the response.
    /// UI platforms (i.e Web, Discord) generally accept all messages, with chatbot functionality checking platform applicability.
    pub platform: Platform,
    pub channel: &'static str,
    pub payload: Payload,
}

impl Response {
    #[tracing::instrument(level = "trace", skip(chan))]
    pub async fn send(self, loc: Location, chan: &mpsc::Sender<(Location, Response)>)
    /*-> error::Result<()>*/
    {
        tracing::trace!("sending");
        if let Err(e) = chan.send((loc, self)).await {
            tracing::error!("{}", e);
        }
        //Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Location {
    Pubsub,
    /// Addr, username
    Websocket(Arc<String>, SocketAddr),
    Websockets(Option<Vec<(Arc<String>, SocketAddr)>>),
    Broadcast,
}

#[derive(Clone)]
pub struct Server {
    pub pub_in_tx: mpsc::Sender<pubsub::Msg>, // redis <- msg resp
    pub ws_in_tx: mpsc::Sender<ws::Msg>,      // ws <- msg resp
    pub msg_out_tx: mpsc::Sender<(Location, Response)>,
    pub commands: Arc<RwLock<Arc<Vec<Command>>>>,
    pub filters: Arc<RwLock<Arc<Vec<Command>>>>,
    pub timers: Arc<RwLock<Arc<Vec<Command>>>>,
    pub db: db::Handle,
    pub cache: cache::Handle,
    pub lock: lock::Handle,
    pub cancel_tasks: Arc<RwLock<Option<watch::Sender<()>>>>,
}

// '!' to avoid conflicting with lock variables
pub static CONFIG_FILE_LOCK: Lazy<String> =
    Lazy::new(|| format!("aussiebot!config_{}", &*super::CHANNEL_NAME));

impl Server {
    async fn msg(&self, msg: Message, location: Location) {
        let Message {
            platform,
            channel,
            payload,
        } = msg;

        tracing::info!(platform=%platform, location=?location,"\x1b[93mMessage received\x1b[0m");

        #[allow(clippy::op_ref)]
        if channel.as_str() != &*crate::CHANNEL_NAME {
            return;
        }

        match payload {
            Payload::NotifyStart => self.started(platform, location).await,
            Payload::Chat(chat) => self.chat(platform, &chat, location).await,
            Payload::InvokeCommand(invocation) => {
                self.invoke(platform, &invocation, location).await;
            }
            Payload::StreamEvent(event) => {
                self.stream_event(platform, event, location).await;
            }
            Payload::DumpConfig => {
                let dump = self.dump_config();
                //if let Ok(Ok(dump)) = dump {
                // send resp
                Response {
                    platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::ConfigDump(dump),
                }
                .send(location, &self.msg_out_tx)
                .await;
                //}
            }
            Payload::DumpSchema => {
                let dump = Arc::new(cmds::schema(platform));
                // send resp
                Response {
                    platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::SchemaDump(dump),
                }
                .send(location, &self.msg_out_tx)
                .await;
            }
            Payload::ConfigDump(config) => {
                tracing::debug!("ConfigDump: {:#?}", config);

                // acquire lock on disk config (max 5 seconds)
                let locked = self.lock.lock(&*CONFIG_FILE_LOCK, 5).await.unwrap();

                if locked {
                    // set config
                    // TODO: filter out invalid commands from active config
                    self.handle_cmds_with_tasks(&config.commands, &config.timers);
                    *self.commands.write() = config.commands.clone();
                    *self.filters.write() = config.filters.clone();
                    *self.timers.write() = config.timers.clone();

                    // dump to disk
                    let _ = futures_util::future::join3(
                        cmds::save_cmds(&config.commands),
                        cmds::save_filters(&config.filters),
                        cmds::save_timers(&config.timers),
                    )
                    .await;

                    // send ok to dumper
                    Response {
                        platform,
                        channel: &*crate::CHANNEL_NAME,
                        payload: Payload::ConfigSaved,
                    }
                    .send(location, &self.msg_out_tx)
                    .await;

                    // broadcast config change notif
                    Response {
                        platform,
                        channel: &*crate::CHANNEL_NAME,
                        payload: Payload::ConfigChanged,
                    }
                    .send(Location::Broadcast, &self.msg_out_tx)
                    .await;

                    let _ = self.lock.unlock(&*CONFIG_FILE_LOCK).await;
                }
            }
            Payload::DumpLog(platform) => {
                let list = cmds::log::Log::list(&self.cache, &platform).await;
                if let Some(list) = list {
                    // TODO: list may be huge, impl partial fetching or smth
                    Response {
                        platform,
                        channel: &*crate::CHANNEL_NAME,
                        payload: Payload::LogDump(list),
                    }
                    .send(location, &self.msg_out_tx)
                    .await;
                }
            }
            Payload::Ping(ping) => {
                tracing::info!("\x1b[93mPing received\x1b[0m");
                if ping.pingee.id.is_empty() {
                    // TODO: fill pingee ID from db if missing
                }
                // forward ping
                Response {
                    platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::Ping(ping),
                }
                .send(Location::Broadcast, &self.msg_out_tx)
                .await;
            }
            Payload::DumpModActions => {
                let list = cmds::log::Log::list_mod_actions(&self.db).await;
                match list {
                    Ok(list) => {
                        Response {
                            platform,
                            channel: &*crate::CHANNEL_NAME,
                            payload: Payload::ModActionsDump(list),
                        }
                        .send(location, &self.msg_out_tx)
                        .await;
                    }
                    Err(e) => {
                        tracing::error!("{}", e);
                    }
                }
            }
            Payload::DumpArgs(args_platform) => {
                self.dump_args(platform, location, args_platform).await
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(skip_all, fields(name = invocation.user.name.as_str(), cmd = invocation.cmd.as_str()))]
    async fn invoke(&self, platform: Platform, invocation: &Invocation, location: Location) {
        tracing::info!(args=?invocation.args, kind=?invocation.kind, user=?invocation.user, "\x1b[93mInvocation received\x1b[0m");

        let ctx = cmds::Context {
            user: &invocation.user,
            meta: &invocation.meta,
            platform,
            location,
            resp: &self.msg_out_tx,
            db: &self.db,
            cache: &self.cache,
            lock: &self.lock,
            filter_cache: RwLock::new(None),
        };

        // ignore filters and timers
        let commands = self.commands.read().clone();
        let _ =
            futures_util::future::join_all(commands.iter().map(|cmd| cmd.invoke(&ctx, invocation)))
                .await;
    }

    /// Process a chat message
    #[tracing::instrument(skip_all, fields(name = chat.user.name.as_str()))]
    async fn chat(&self, platform: Platform, chat: &Chat, location: Location) {
        tracing::info!(user=?chat.user, meta=?chat.meta, msg=%chat.msg,"\x1b[93mChat received\x1b[0m");

        // it's ok to take refs because each chat msg gets its own task with its own `self` instance
        let ctx = cmds::Context {
            user: &chat.user,
            meta: &chat.meta,
            platform,
            location,
            resp: &self.msg_out_tx,
            db: &self.db,
            cache: &self.cache,
            lock: &self.lock,
            filter_cache: RwLock::new(None),
        };

        if let Some((mod_action, filter_name)) = self.filter_chat(&ctx, chat).await {
            tracing::info!(
                "Filter tripped, name: {}, action: {:?}",
                filter_name,
                mod_action
            );
            if ctx.user.perms < Permissions::MOD {
                // send resp
                Response {
                    platform: ctx.platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::ModAction(ctx.user.clone(), mod_action, filter_name),
                }
                .send(Location::Broadcast, ctx.resp)
                .await;
            }
        } else {
            // await Timer.runs' as well, to count messages
            let timers = self.timers.read().clone();
            let commands = self.commands.read().clone();
            let iter = commands.iter().chain(timers.iter()); //timers.iter().chain(commands.iter());

            let res = futures_util::future::join_all(iter.map(|cmd| cmd.chat(&ctx, chat))).await;
            tracing::debug!(res=?res);

            self.autocorrect(&ctx, &res).await;
        }

        // send chat to any and all web clients
        Response {
            platform,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::Chat(chat.clone()),
        }
        .send(Location::Websockets(None), &self.msg_out_tx)
        .await;
    }

    /// Send autocorrect suggestions if any, if no command was successfully run
    async fn autocorrect(&self, ctx: &cmds::Context<'_>, res: &[error::Result<RunRes>]) {
        // accumulate suggestions, unless at least one successful command call
        let res = res.iter().try_fold(vec![], |mut sugg, curr| match curr {
            Ok(RunRes::Ok) => {
                tracing::debug!("at least one RunRes::Ok found, stopping autocorrect");
                ControlFlow::Break(())
            }
            Ok(RunRes::Autocorrect(prefix)) => {
                sugg.push(prefix.to_owned());
                ControlFlow::Continue(sugg)
            }
            _ => ControlFlow::Continue(sugg),
        });

        let autocorrect_list = match res {
            ControlFlow::Continue(res) => res,
            _ => return,
        };

        // send suggestions if any
        if !autocorrect_list.is_empty() {
            tracing::info!(suggestions=?autocorrect_list, "autocorrect");
            // send resp
            Response {
                platform: ctx.platform,
                channel: &*crate::CHANNEL_NAME,
                payload: Payload::Autocorrect(ctx.user.clone(), autocorrect_list),
            }
            .send(ctx.location.clone(), ctx.resp)
            .await;
        }
    }

    /// Run filters and return the most severe filter action and the name of the filter that issued it
    async fn filter_chat(
        &self,
        ctx: &cmds::Context<'_>,
        chat: &Chat,
    ) -> Option<(ModAction, Arc<String>)> {
        let filters = self.filters.read().clone();

        let filtered =
            futures_util::future::join_all(filters.iter().map(|cmd| cmd.chat(ctx, chat))).await;

        let most_severe_action = tokio::task::spawn_blocking(move || {
            filtered
                .iter()
                .enumerate()
                .fold::<(usize, Option<ModAction>), _>((0, None), |acc, (curr_i, res)| {
                    match (&acc.1, &res) {
                        (None, Ok(RunRes::Filtered(curr))) => (curr_i, Some(*curr)),
                        // return the most severe filter action
                        (Some(prev), Ok(RunRes::Filtered(curr))) if curr > prev => {
                            (curr_i, Some(*curr))
                        }
                        // return acc otherwise
                        _ => acc,
                    }
                })
        })
        .await
        .unwrap();

        if let (i, Some(action)) = most_severe_action {
            let filter_name = Arc::new(filters[i].name().to_owned());
            if action > ModAction::None {
                // log mod action
                cmds::log::Log::mod_action(
                    ctx.db.clone(),
                    ctx.platform,
                    ctx.user.id.clone(),
                    action,
                    filter_name.clone(),
                );
            }
            Some((action, filter_name))
        } else {
            None
        }
    }

    #[tracing::instrument(skip(self))]
    async fn started(&self, platform: Platform, location: Location) {
        match platform {
            Platform::DISCORD => {
                self.dump_args(platform, location, platform).await;
            }
            platform if Platform::STREAM.contains(platform) => {
                let url_key = format!("aussiebot!{}!streamurl!{}", &*super::CHANNEL_NAME, platform);
                if let Ok(RespType::String(url)) =
                    Cache::Get(url_key.into()).exec(&self.cache).await
                {
                    tracing::info!(url=%url,"sending start");
                    Response {
                        platform,
                        channel: &*crate::CHANNEL_NAME,
                        payload: Payload::StreamSignal(StreamSignal::Start(url.into())),
                    }
                    .send(Location::Broadcast, &self.msg_out_tx)
                    .await;
                }
            }
            _ => {}
        }
    }

    async fn dump_args(&self, platform: Platform, location: Location, args_platform: Platform) {
        let cmds = self.commands.read().clone();
        let args: ArgsDump = cmds
            .iter()
            .filter_map(|cmd| cmd.args_schema(args_platform))
            .collect();
        Response {
            platform,
            channel: &*crate::CHANNEL_NAME,
            payload: Payload::ArgsDump(args),
        }
        .send(location, &self.msg_out_tx)
        .await;
    }

    fn dump_config(&self) -> cmds::CommandConfig {
        //Result<Result<String, serde_json::Error>, tokio::task::JoinError> {
        let commands = self.commands.read().clone();
        let filters = self.filters.read().clone();
        let timers = self.timers.read().clone();

        cmds::CommandConfig {
            filters,
            commands,
            timers,
        }
    }

    fn handle_cmds_with_tasks(&self, commands: &[Command], timers: &[Command]) {
        // cancel existing timer/log tasks if any
        if let Some(cancel_chan) = self.cancel_tasks.write().take() {
            let _ = cancel_chan.send(());
            // cancel_chan is dropped here anyway
        }

        let (cancel_chan_tx, cancel_chan_rx) = watch::channel(()); //spmc

        // start new timer tasks
        for timer in timers {
            if let Command::Timer(t) = timer {
                t.init(cancel_chan_rx.clone(), &self.cache, &self.msg_out_tx);
            }
        }

        // start new log tasks
        for command in commands {
            if let Command::Log(log) = command {
                log.init(cancel_chan_rx.clone(), &self.cache);
            }
        }

        // set new task cancel chan
        *self.cancel_tasks.write() = Some(cancel_chan_tx);
    }

    #[tracing::instrument(skip(self))]
    async fn stream_event(&self, platform: Platform, event: StreamEvent, location: Location) {
        match event {
            StreamEvent::DetectStart(ref url) => {
                // broadcast stream start signal
                // the relevant chatbot should start up correspondingly
                tracing::info!(url=%url,"sending start");
                Response {
                    platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::StreamSignal(StreamSignal::Start(url.clone())),
                }
                .send(Location::Broadcast, &self.msg_out_tx)
                .await;
            }
            // TODO: add url
            StreamEvent::DetectStop(ref url) => {
                // broadcast stream stop signal
                tracing::info!(url=%url,"sending stop");
                Response {
                    platform,
                    channel: &*crate::CHANNEL_NAME,
                    payload: Payload::StreamSignal(StreamSignal::Stop(url.clone())),
                }
                .send(Location::Broadcast, &self.msg_out_tx)
                .await;
            }
            StreamEvent::Started(ref url, ref id) => {
                // fetch swap stream id, announce if different
                let id_key = format!("aussiebot!{}!streamid!{}", &*super::CHANNEL_NAME, platform);
                let url_key = format!("aussiebot!{}!streamurl!{}", &*super::CHANNEL_NAME, platform);
                let (_, prev_id) = tokio::join!(
                    Cache::Set(url_key.into(), url.clone(), 0, false).exec(&self.cache),
                    Cache::SetGet(id_key.into(), id.clone(), 0).exec(&self.cache)
                );
                tracing::debug!(prev_id = ?prev_id, id = %id, url = %url,"\x1b[93mStreamEvent::Started\x1b[0m");
                let announce = match prev_id {
                    Err(Error::Redis(e)) if e.kind() == redis::ErrorKind::TypeError => true,
                    Ok(RespType::String(prev_id)) if prev_id.as_str() != id.as_str() => true,
                    _ => false,
                };

                if announce {
                    let invocation = Invocation {
                        cmd: Arc::new("@stream_event".into()),
                        args: HashMap::with_capacity(0),
                        kind: Some(InvocationKind::StreamEvent(event)),
                        meta: None,
                        user: Arc::new(User::default()),
                    };

                    self.invoke(platform, &invocation, location).await;
                }
            }
            StreamEvent::Stopped(vid) => {
                tracing::info!(vid = %vid, "stop event");
            }
        }
    }

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
                    Location::Pubsub => {
                        let _ = self.pub_in_tx.send(msg).await;
                    }
                    Location::Websocket(username, addr) => {
                        let _ = self
                            .ws_in_tx
                            .send((Some(vec![(username, addr)]), msg))
                            .await;
                    }
                    Location::Websockets(addrs) => {
                        let _ = self.ws_in_tx.send((addrs, msg)).await;
                    }
                    Location::Broadcast => {
                        let _ = tokio::join!(
                            self.pub_in_tx.send(msg.clone()),
                            self.ws_in_tx.send((None, msg))
                        );
                    }
                }
            }
        }
    }

    /// Start the server, consuming it
    #[tracing::instrument(skip_all)]
    pub fn start(
        self,
        msg_in_rx: mpsc::Receiver<(Location, String)>,
        msg_out_rx: mpsc::Receiver<(Location, Response)>,
    ) -> JoinHandle<()> {
        tracing::info!("\x1b[92m-------------Starting message loop-------------\x1b[0m");

        // init timers
        let commands = self.commands.read().clone();
        let timers = self.timers.read().clone();
        self.handle_cmds_with_tasks(&commands, &timers);

        // handle response messages
        let server = self.clone();
        tokio::spawn(server.msg_tx_loop(msg_out_rx));

        // process received messages
        tokio::spawn(self.msg_rx_loop(msg_in_rx))
    }
}
