include!(concat!(env!("OUT_DIR"), "/timestamp.rs"));

use crate::{
    msg::{self, Permissions, Platform},
    resp::{NotifyType, Payload, Response, StreamSignal, CHANNEL_NAME},
    util::{self, acquire_lock, release_lock},
    DbPool, RedisPool,
};

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use rand::{distributions::Uniform, prelude::*};
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

pub type TimerHandle = Arc<tokio::task::JoinHandle<()>>;
type LockedCmdlist = RwLock<Vec<Command>>;

pub struct CommandListInner {
    pub filters: LockedCmdlist,
    pub commands: LockedCmdlist,
    pub timers: LockedCmdlist,
    pub is_main_instance: bool,
    pub timer_handles: RwLock<Vec<TimerHandle>>,
}

// unlocked & owning version of CommandListInner
// TODO: remove the need for this
pub struct UnlockedCmdListInner {
    pub filters: Vec<Command>,
    pub commands: Vec<Command>,
    pub timers: Vec<Command>,
    pub is_main_instance: bool,
    pub timer_handles: Vec<TimerHandle>,
}

pub type CommandList = Arc<CommandListInner>;

pub struct Context<'a> {
    pub chat: &'a msg::Chat,
    pub time: Instant,
    cmdlist: CommandList, // arc to CommandListInner
}

impl<'a> Context<'a> {
    pub fn new(cmdlist: CommandList, chat: &'a msg::Chat) -> Self {
        // TODO: nandle PoisonError
        // let cmdlist = match cmdlist.read() {
        //     Ok(cmdlist) => cmdlist.clone(),
        //     Err(p_err) => (*p_err.get_ref()).clone(),
        // };
        // note: parking lot's locks do not support poison errors

        // We removed the CommandListInner.clone here but we still clone the commands, filter and timers list in .filter and .run
        // Now though, timers and commands aren't cloned if filters are tripped

        Self {
            cmdlist,
            chat,
            time: Instant::now(),
        }
    }

    /// Run filters and return the most severe filter action and the name of the filter that issued it
    pub async fn filter(&self) -> Option<(ModAction, String)> {
        let filters = self.cmdlist.filters.read().clone();

        let filtered =
            futures_util::future::join_all(filters.iter().map(|cmd| cmd.run(self))).await;

        if let (i, Some(action)) = filtered.most_severe_action() {
            let filter_name = filters[i].name();
            println!(
                "\x1b[91mFilter {:?}'s action for '{}': {:?}\x1b[0m",
                filter_name, self.chat.src.name, action
            );
            Some((action, filter_name.to_owned()))
        } else {
            None
        }
    }

    /// Run commands to completion and return the results
    pub async fn run(&self) -> Vec<Option<RunResult>> {
        // await Timer.runs' as well, to count messages
        let timers = self.cmdlist.timers.read().clone();
        let commands = self.cmdlist.commands.read().clone();
        let iter = timers.iter().chain(commands.iter());

        futures_util::future::join_all(iter.map(|cmd| cmd.run(self))).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    #[serde(with = "util::serde_arc_string")]
    String(Arc<String>),
    Number(i64),
    Bool(bool),
    Permissions(Permissions),
    Platforms(Vec<msg::Platform>),
    #[serde(with = "util::serde_regex")]
    Regex(Regex),
    ModAction(ModAction),
}

/// (type, name, desc, Vec<(key, desc, Value)>)
type DumpedSchema<'a> = (
    &'a str,                        // type
    &'a str,                        // desc
    Vec<(&'a str, &'a str, Value)>, // (key, desc, Value)
);

/// (type, name, desc, Vec<(key, desc, Value)>)
/// EDIT: DumpedCommand has to mirror OwnedDumpedCmd, for dumping and loading from disk
pub type DumpedCommand<'a> = (
    &'a str,               // type
    String,                // name
    Vec<(&'a str, Value)>, // (key, desc, Value)
);

/// (type, name, Vec<(key, Value)>), with owned values
/// DumpedCommand with owned values
pub type OwnedDumpedCmd = (String, String, Vec<(String, Value)>);

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy, Serialize, Deserialize)]
pub enum ModAction {
    None = 0,
    Warn = 1,
    Remove = 2,
    Timeout = 3,
    Kick = 4,
    Ban = 5,
}

//#[derive(Debug)]
pub enum RunResult {
    Ok,
    Filtered(ModAction),
}

pub trait Filtered {
    /// Return the most severe filter action present and its index in the filter list
    fn most_severe_action(&self) -> (usize, Option<ModAction>);
}

impl Filtered for Vec<Option<RunResult>> {
    fn most_severe_action(&self) -> (usize, Option<ModAction>) {
        self.iter()
            .enumerate()
            .fold((0, None), |acc, (curr_i, res)| {
                match (&acc.1, &res) {
                    (None, Some(RunResult::Filtered(curr))) => (curr_i, Some(*curr)),
                    // return the most severe filter action
                    (Some(prev), Some(RunResult::Filtered(curr))) if curr > prev => {
                        (curr_i, Some(*curr))
                    }
                    // return acc otherwise
                    _ => acc,
                }
            })
    }
}

#[macro_export]
/// Generate available base commands
macro_rules! declare_commands {
    // TODO: remove 'filters' group, it's useless. atm it's only used for excluding commands from the default command list returned by cmds::init
    (
      $( filters: { $({ $filter_cmd:ident $({ $( $filter_arg_key:ident : $filter_arg_type:ty ),+ })?, $({ $( $filter_lock:ident )+ })? description: $filter_cmd_desc:literal $( $filter_key:ident ($filter_description:literal) => $filter_type:ident ($filter_default_value:expr) $($filter_is_valid:block)* )* })* } )?
      $({ $cmd:ident $({ $( $arg_key:ident : $arg_type:ty ),+ })?, $({ $( $lock:ident )+ })? description: $cmd_desc:literal $( $key:ident ($description:literal) => $type:ident ($default_value:expr) $($is_valid:block)* )* })*
    ) => {
      declare_commands_inner! {
        $( $({ $filter_cmd $({ $( $filter_arg_key : $filter_arg_type ),+ })?, $({ $( $filter_lock )+ })? description: $filter_cmd_desc $( $filter_key ($filter_description) => $filter_type ($filter_default_value) $($filter_is_valid)* )* })* )?
        $({ $cmd $({ $( $arg_key : $arg_type ),+ })?, $({ $( $lock )+ })? description: $cmd_desc $( $key ($description) => $type ($default_value) $($is_valid)* )* })*
      }
      // list of all commands with default config and filters
      pub fn init(redis: RedisPool, db: DbPool) -> CommandList {
        Arc::new(CommandListInner {
          filters: RwLock::new(vec![$($(Command::$filter_cmd($filter_cmd::new("", redis.clone(), db.clone()))),*)?]),
          commands: RwLock::new(vec![$(Command::$cmd($cmd::new("", redis.clone(), db.clone()))),*]),
          timers: RwLock::new(vec![]),
          is_main_instance: true,
          timer_handles: RwLock::new(vec![])
        })
      }
    };
}

macro_rules! declare_commands_inner {
  // insert any default properties here, i.e enabled, perms etc.
  ( $({ $cmd:ident $({ $( $arg_key:ident : $arg_type:ty ),+ })?, $({ $( $lock:ident )+ })? description: $cmd_desc:literal $( $key:ident ($description:literal) => $type:ident ($default_value:expr) $($is_valid:block)* )* })* ) => {
    declare_commands_inner! { 0 $({
      $cmd $({ $( $arg_key : $arg_type ),+ })?,
      {
        rate
        $( $( $lock )+ )?
      }
      // description
      description: $cmd_desc
      // enabled flag
      enabled("Enabled") => Bool(true)
      // list of platforms to run command on, empty means all
      platforms("Platforms") => Platforms(vec![])
      $( $key ($description) => $type ($default_value) $($is_valid)* )* })*
    }
  };
  ( 0 $({ $cmd:ident $({ $( $arg_key:ident : $arg_type:ty ),+ })?, $({ $( $lock:ident )+ })? description: $cmd_desc:literal $( $key:ident ($description:literal) => $type:ident ($default_value:expr) $($is_valid:block)* )* })* ) => {
    paste::paste! {
      #[allow(dead_code)]
      #[derive(Debug, Clone)]
      /// Available commands
      pub enum Command {
        $($cmd($cmd)),*,
      }

      $(
        #[allow(dead_code)]
        #[derive(Clone)]
        /// Command-specific config
        pub struct $cmd  {
          name: String,
          config: [<$cmd Config>],
          redis: RedisPool,
          db: DbPool
        }
        #[derive(Debug, Clone)]
        struct [<$cmd Config>] {
          $(
            $key: Value
          ),*
        }

        #[allow(dead_code)]
        #[derive(Debug, Default)]
        #[doc = "Arguments for [`" $cmd "`]"]
        struct [<$cmd Args>] {
          $($(
            $arg_key: $arg_type
          ),+)?
        }

        const [<$cmd:upper _DESC>]: &str = $cmd_desc;
        $( $(
          static [<$cmd:upper _LOCK_ $lock:upper>]: Lazy<String> = Lazy::new(|| format!(concat!("aussiebot_{}_", stringify!([<$cmd:lower _ $lock:lower>])), *CHANNEL_NAME) );
        )+ )?
        $(
          const [<$cmd:upper _KEY_ $key:upper _DESC>]: &str = $description;
        )*

        // custom debug impl that doesn't include redis or db
        impl std::fmt::Debug for $cmd {
          fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
              //std::fmt::Debug::fmt(&self.config, f)
              f.debug_struct(stringify!($cmd))
              .field("name", &self.name)
              .field("config", &self.config)
              .finish()
          }
        }

        impl $cmd {
          #[allow(dead_code)]
          #[doc = "Returns a new instance of the command"]
          pub fn new(name: impl Into<String>, redis: RedisPool, db: DbPool,) -> Self {
            Self {
              name: name.into(),
              config: [<$cmd Config>] {
                $(
                $key: Value::$type($default_value)
                ),*
              },
              redis, db
            }
          }

          #[allow(dead_code)]
          #[doc = "Get a config variable, returns `Some(Value)` on success"]
          fn get(&self, key: &str) -> Option<Value> {
            match key {
              $(
                stringify!($key) => Some(self.config.$key.clone())
              ),*,
              _ => None
            }
          }

          #[allow(dead_code)]
          #[doc = "Set a config variable, returns `Some(())` on success"]
          fn set(&mut self, key: &str, value: Value) -> Option<()> {
            match (key, value) {
              $(
                (stringify!($key), Value::$type(x)) => {
                  // verify input if is_valid block present
                  $(
                    let is_valid = $is_valid;
                    if !is_valid(&x) {
                      return None
                    }
                  )*
                  self.config.$key = Value::$type(x);
                  Some(())
                }
              ),*,
              _ => None
            }
          }

          #[allow(dead_code)]
          #[doc = "Dump config to `Vec` of tuples"]
          fn dump(&self) -> Vec<(&'static str, Value)> {
            vec![$( (stringify!($key), self.config.$key.clone()) ),*]
          }

          #[allow(dead_code)]
          #[doc = "Checks if command is able to run, i.e is enabled, prefix matches, valid arguments, user authorised, not rate-limited."]
          #[doc = "Returns arguments"]
          // TODO: move out of macro, to remove calls to Command::get
          async fn can_run(&self, ctx: &Context<'_>) -> Option<[<$cmd Args>]> {
            let chat = ctx.chat;
            // check if enabled
            match &self.config.enabled {
              Value::Bool(true) => {},
              _ => return None,
            };
            // check if platform is applicable
            match &self.config.platforms {
              // if platform list is non-empty and doesn't include msg's platform, return
              Value::Platforms(list) if !list.is_empty() && !list.contains(&chat.src.platform) => return None,
              _ => {}
            }
            // check prefix and arguments (must be a pure fn)
            let args = self.parse_arguments(ctx)?;

            // check permissions (apply_to overrides perms)
            if let Some(Value::Permissions(apply_to)) = self.get("apply_to") {
                if chat.src.perms > apply_to {
                    return None;
                }
            } else if let Some(Value::Permissions(perms)) = self.get("perms") {
                if chat.src.perms < perms {
                    return None;
                }
            }

            // only rate-limit if perm < Admin
            if chat.src.perms < Permissions::Admin {
                // key's a fn of cmd AND name, in case multiple instances are present, i.e multiple Text cmds
                let ratelimit_key = format!("{}_{}", &*[<$cmd:upper _LOCK_RATE>], self.name);

                // check if rate-limited globally
                let global_rl_taken = if let Some(Value::Number(t)) = self.get("ratelimit") {
                    if !acquire_lock(self.redis.clone(), &ratelimit_key, t as u64).await? {
                        println!(concat!("\x1b[33m",stringify!($cmd)," rate-limited globally\x1b[0m"));
                        return None;
                    }
                    true
                } else { false };
                // check if rate-limited locally
                if let Some(Value::Number(t)) = self.get("ratelimit_user") {
                    if !acquire_lock(self.redis.clone(), &format!("{}_{}", &ratelimit_key, &chat.src.id), t as u64).await? {
                        println!(concat!("\x1b[33m",stringify!($cmd)," rate-limited locally\x1b[0m"));
                        if global_rl_taken {
                          // release the global ratelimit lock
                          release_lock(self.redis.clone(), ratelimit_key).await?;
                        }
                        return None;
                    }
                }
            }

            println!(concat!("\x1b[36m",stringify!($cmd),": {:?} ({})\x1b[0m"), self.name, chat.src.name);
            // return args
            Some(args)
          }
        }
      )*

      impl Command {
        #[allow(dead_code)]
        pub fn get(&self, key: &str) -> Option<Value> {
          match self {
            $(
              Self::$cmd(c) => match key {
                $(
                  stringify!($key) => Some(c.config.$key.clone())
                ),*,
                _ => None
              }
            )*
          }
        }

        #[allow(dead_code)]
        pub fn set(&mut self, key: impl AsRef<str>, value: Value) -> Option<&mut Self> {
          match self {
            $(
              Self::$cmd(c) => match (key.as_ref(), value) {
                $(
                  (stringify!($key), Value::$type(x)) => {
                    // verify input if is_valid block present
                    $(
                      let is_valid = $is_valid;
                      if !is_valid(&x) {
                        return None
                      }
                    )*
                    c.config.$key = Value::$type(x);
                    Some(self)
                  }
                ),*,
                _ => None
              }
            )*
          }
        }

        #[allow(dead_code)]
        pub fn dump(&self) -> DumpedCommand {
          match self {
            $(
              Self::$cmd(inner) => (stringify!($cmd), inner.name.to_owned(), inner.dump()),
            )*
          }
        }

        #[doc = "Asynchronously run the command"]
        pub async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
          match self {
            $(
              Self::$cmd(c) => c.run(ctx).await
            ),*
          }
        }

        pub fn name(&self) -> &str {
          match self {
            $(
              Self::$cmd(c) => c.name.as_ref()
            ),*
          }
        }

        #[allow(dead_code)]
        pub fn set_name(&mut self, name: impl Into<String>) {
          match self {
            $(
              Self::$cmd(c) => { c.name = name.into() }
            ),*
          }
        }

        pub const fn description(&self) -> &'static str {
          match self {
            $(
              Self::$cmd(_) => [<$cmd:upper _DESC>]
            ),*
          }
        }

        /// Dump config schema with default values
        fn list_config() -> Vec<DumpedSchema<'static>> {
          vec![$((
            //Self::$cmd(_) => { c.name = name.into() }
            (stringify!($cmd), [<$cmd:upper _DESC>], vec![
              $(
                (stringify!($key), [<$cmd:upper _KEY_ $key:upper _DESC>], Value::$type($default_value))
              ),*
            ])
          )),*]
        }

        pub fn new(cmd_type: impl AsRef<str>, name: impl Into<String>, redis: RedisPool, db: DbPool) -> Option<Self> {
          match cmd_type.as_ref() {
            $(
              stringify!($cmd) => Some(Self::$cmd($cmd::new(
                name,
                redis.clone(),
                db.clone(),
              )))
            ),*,
            _ => None
          }
        }

      }
    }
  };
}

declare_commands! {
  filters: {
    {
      Filter,
      description: "Filter chat based on username and messsage"
      apply_to("Apply to anyone below permission level") => Permissions(0.into())
      user_contains("Username contains") => String(Arc::new("".into()))
      msg_contains("Message contains (⚠: make sure the filter's name doesn't trigger the filter)") => String(Arc::new("".into()))
      id_contains("User id contains  (case-sensitive)") => String(Arc::new("".into()))
      action("Mod action to take") => ModAction(ModAction::None)
    }
    {
      RegexFilter,
      description: "Filter chat based matching username and message against a regex pattern"
      apply_to("Apply to anyone below permission level") => Permissions(0.into())
      user_pattern("Username pattern") => Regex(Regex::new("").unwrap())
      id_pattern("User id pattern") => Regex(Regex::new("").unwrap())
      msg_pattern("Message pattern (⚠: make sure the filter's name doesn't trigger the filter)") => Regex(Regex::new("").unwrap())
      action("Mod action to take") => ModAction(ModAction::None)
    }
  }
  {
    Help,
    description: "See available commands"
    name("Commnd name") => String(Arc::new("!help".into())) { |x: &str| !x.is_empty() }
    ratelimit("Cooldown (in seconds)") => Number(30) { |&x| x >= 0 }
  }
  {
    Chat,
    description: "Command that's run on every chat message"
    points("Points awarded per chat message") => Number(5) { |&x| x >= 0 }
    //ratelimit_user("Cooldown per user (in seconds)") => Number(2) { |&x| x >= 0 }
    dono_msg("Donation message") => String(Arc::new("thanks for the donation of {amount}! widepeepoHappy widepeepoHappy widepeepoHappy".into()))
  }
  {
    Give {
      amount: i32,
      to: String
    },
    description: "Give points to someone"
    name("Command name") => String(Arc::new("!give".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(0.into())
    ratelimit_user("Cooldown per user (in seconds)") => Number(6) { |&x| x >= 0 }
    min_amount("Minimum amount") => Number(1) { |&x| x >= 0 }
    max_amount("Maximum amount") => Number(10_000) { |&x| x >= 0 }
  }
  {
    Gamble {
      amount: i32
    },
    description: "Gamble points"
    name("Command name") => String(Arc::new("!gamble".into())) { |x: &str| !x.is_empty() }
    ratelimit_user("Cooldown per user (in seconds)") => Number(6) { |&x| x >= 0 }
    min_amount("Minimum wager") => Number(10) { |&x| x >= 0 }
    max_amount("Maximum wager") => Number(10_000) { |&x| x >= 0 }
  }
  {
    Heist {
      amount: i32
    },
    {
      active
      members
    }
    description: "Start/join a heist"
    name("Command name") => String(Arc::new("!heist".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(0.into())
    duration("Duration (in seconds)") => Number(20) { |&x| x >= 0 }
    // TODO: fix ratelimit (un)locking
    min_amount("Minimum wager") => Number(10) { |&x| x >= 0 }
    max_amount("Maximum wager") => Number(10_000) { |&x| x >= 0 }
  }
  {
    Ban {
      name: String
    },
    description: "Ban someone"
    name("Command name") => String(Arc::new("!ban".into())) { |x: &str| !x.is_empty() }
    usage("Command usage") => String(Arc::new("{name} <username>".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(Permissions::Admin)
  }
  // !mod sends a DM to stream mods on discord
  {
    SummonMod {
      msg: Option<String>
    },
    description: "Ping a mod on discord"
    name("Command name") => String(Arc::new("!mod".into())) { |x: &str| !x.is_empty() }
    discord_id("Discord id of account to ping")  => String(Arc::new("".into())) { |x: &str| !x.is_empty() }
    ratelimit_user("Cooldown per user (in seconds)") => Number(120) { |&x| x >= 0 }  // each user can !mod every 120 seconds
    ratelimit("Cooldown (in seconds)") => Number(30) { |&x| x >= 0 }                // but at most 1 !mod every 30 seconds
  }
  // calls a french joke api
  {
    French,
    description: "fr*nch"
    name("Command name") => String(Arc::new("!french".into())) { |x: &str| !x.is_empty() }
    ratelimit("Cooldown (in seconds)") => Number(5) { |&x| x >= 0 }
  }
  {
    Text,
    description: "Sends a message"
    name("Command name") => String(Arc::new("!text".into())) { |x: &str| !x.is_empty() }
    ratelimit_user("Cooldown per user (in seconds)") => Number(20) { |&x| x >= 0 }
    ratelimit("Cooldown (in seconds)") => Number(5) { |&x| x >= 0 }
    msg("Message to send") => String(Arc::new("<placeholder text, change me>".into()))
    perms("Permissions") => Permissions(Permissions::None)
  }
  {
    ToggleFilters,
    description: "Toggle filters"
    name("Command name") => String(Arc::new("!filter".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(Permissions::Admin)
  }
  {
    Dump {
      what: Option<DumpSetWhat> // ignore if None
    },
    description: "Dump commands"
    name("Command name") => String(Arc::new("!dump".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(Permissions::Admin)
  }
  {
    Set {
      what: Option<DumpSetWhat>, // ignore if None
      json: String
    },
    description: "Set commands"
    name("Command name") => String(Arc::new("!set".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(Permissions::Admin)
  }
  {
    Points,
    description: "Check points"
    name("Command name") => String(Arc::new("!points".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(0.into())
    ratelimit_user("Cooldown per user (in seconds)") => Number(5) { |&x| x >= 0 }
  }
  {
    Transfer {
      amount: i32,
      from: Platform,
      to: Platform
    },
    description: "Transfer points between platforms"
    name("Command name") => String(Arc::new("!transfer".into())) { |x: &str| !x.is_empty() }
    usage("Command usage") => String(Arc::new("{name} <amount> from <platform> to <platform>".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(0.into())
    ratelimit_user("Cooldown per user (in seconds)") => Number(5) { |&x| x >= 0 }
    min_amount("Minimum wager") => Number(10) { |&x| x >= 0 }
    max_amount("Maximum wager") => Number(10_000) { |&x| x >= 0 }
  }
  {
    Timer,
    { count }
    description: "Send a message at regular intervals"
    interval("Repetition interval (in seconds)") => Number(5) { |&x| x >= 5 }
    jitter("Max random delay (in seconds)") => Number(5) { |&x| x >= 0 }
    msg("Message to send") => String(Arc::new("<placeholder timer text, change me>".into()))
    msg_count("Min. number of chat messages before triggering timer \
               (Setting this to 0 will cause messages to be sent regardless of whether anyone's talking in chat, which may not be what you want)") => Number(1) { |&x| x >= 0 }
  }
  {
    Stream {
      platform: Platform,
      vid: String
    },
    description: "Start/stop listening to a stream"
    name("Command name") => String(Arc::new("!stream".into())) { |x: &str| !x.is_empty() }
    perms("Permissions") => Permissions(Permissions::Admin)
  }
}

#[derive(Debug)]
enum DumpSetWhat {
    Schema,
    Commands,
    Filters,
    Timers,
    CmdIndex(usize),
    FilterIndex(usize),
    TimerIndex(usize),
    Disk,
}

//static CHAT_DONO_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.*)\{amount\}(.*)$").unwrap());
static CHAT_DONO_AMT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{amount\}").unwrap());
static CHAT_DONO_NAME_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{name\}").unwrap());

impl Chat {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<ChatArgs> {
        let chat = ctx.chat;
        // ignore broadcast and web
        match chat.src.platform {
            Platform::Youtube | Platform::Discord | Platform::Twitch => Some(ChatArgs {}),
            _ => None,
        }
    }

    /// Upsert points and display name (upserts are atomic, right?)
    async fn handle_points(&self, src: &msg::User, delta: i32) -> Option<()> {
        let client = self.db.get().await.unwrap();
        let sql = match src.platform {
            Platform::Youtube => include_str!("../sql/upsert/youtube_id.sql"),
            Platform::Discord => include_str!("../sql/upsert/discord_id.sql"),
            Platform::Twitch => include_str!("../sql/upsert/twitch_id.sql"),
            _ => unreachable!(),
        };
        let _ = client
            .query_one(sql, &[&src.id, &src.name, &delta])
            .await
            .unwrap();
        println!("Incremented {}'s points by {}", src.name, delta);
        Some(())
    }

    /// Send donation reply
    async fn handle_dono(
        &self,
        src: &msg::User,
        amount: &str,
        dono_msg: impl AsRef<str>,
    ) -> Option<()> {
        // replace amount and name vars
        // escape chars on amount and name to avoid regex operators
        let rep =
            CHAT_DONO_AMT_REGEX.replace_all(dono_msg.as_ref(), amount.escape_debug().to_string());
        let rep = CHAT_DONO_NAME_REGEX.replace_all(&rep, &src.name.escape_debug().to_string());

        // let pipeline = [
        //     (&CHAT_DONO_AMT_REGEX, amount.escape_debug()),
        //     (&CHAT_DONO_NAME_REGEX, src.name.escape_debug()),
        // ];

        // use std::borrow::Cow;

        // let mut msg = Cow::from(dono_msg.as_ref());

        // // let a = pipeline.into_iter().fold(initial_msg, |msg, (regex, var)| {
        // //     let new_msg =
        // // });

        // for (regex, var) in pipeline.into_iter() {
        //     msg = regex.replace_all(msg.as_ref(), var.to_string());
        // }

        // send reply
        Response::new((src.platform, &src.name, &src.id), Payload::Message(&rep))
            .send(self.redis.clone())
            .await
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        self.can_run(ctx).await?;

        // send dono message if applicable
        if let (Some(amount), Value::String(dono_msg)) = (&chat.donation, &self.config.dono_msg) {
            if !dono_msg.is_empty() {
                self.handle_dono(&chat.src, amount, dono_msg.as_str())
                    .await?;
            }
        }

        // increment points if applicable
        if let Value::Number(delta) = self.config.points {
            self.handle_points(&chat.src, delta as i32).await;
        }

        Some(RunResult::Ok)
    }
}

impl Filter {
    const fn parse_arguments(&self, _: &Context<'_>) -> Option<FilterArgs> {
        Some(FilterArgs {})
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        self.can_run(ctx).await?;

        let filter_action = if let Value::ModAction(action) = self.config.action {
            action
        } else {
            return None;
        };
        let filter_action = RunResult::Filtered(filter_action);

        let mut triggered: [Option<bool>; 3] = [None; 3];

        // conditions are ANDed together, exluding empty patterns

        match &self.config.user_contains {
            Value::String(pat) if !pat.is_empty() => {
                let cond = chat.src.name.to_lowercase().contains(pat.as_ref());
                if cond {
                    println!(
                        "\x1b[91mUsername {} contains '{}'\x1b[0m",
                        chat.src.name, pat
                    );
                }
                triggered[0] = Some(cond);
            }
            _ => {}
        }

        match &self.config.id_contains {
            Value::String(pat) if !pat.is_empty() => {
                let cond = chat.src.id.contains(pat.as_ref());
                if cond {
                    println!("\x1b[91mUser id {} contains '{}'\x1b[0m", chat.src.id, pat);
                }
                triggered[1] = Some(cond);
            }
            _ => {}
        }

        match &self.config.msg_contains {
            Value::String(pat) if !pat.is_empty() => {
                let cond = chat.msg.to_lowercase().contains(pat.as_ref());
                if cond {
                    println!(
                        "\x1b[91mMessage from {} contains '{}'\x1b[0m",
                        chat.src.name, pat
                    );
                }
                triggered[2] = Some(cond);
            }
            _ => {}
        }

        // None => filter not enabled
        // Some(false) => filter not tripped
        // Some(true) => tripped

        // returns false if any enabled filter was left untripped, otherwise returns true if any filter was tripped
        let (_, tripped) = triggered
            .into_iter()
            .fold((true, false), |acc, res| match (acc, res) {
                (_, Some(false)) => (false, false),
                ((true, _), Some(true)) => (true, true),
                _ => acc,
            });

        //println!("triggered: {:?}, tripped: {}", triggered, tripped);

        if tripped {
            Some(filter_action)
        } else {
            Some(RunResult::Ok)
        }
    }
}

impl RegexFilter {
    const fn parse_arguments(&self, _: &Context<'_>) -> Option<RegexFilterArgs> {
        Some(RegexFilterArgs {})
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        self.can_run(ctx).await?;

        let filter_action = if let Value::ModAction(action) = self.config.action {
            action
        } else {
            return None;
        };
        let filter_action = RunResult::Filtered(filter_action);

        let mut triggered: [Option<bool>; 3] = [None; 3];

        // conditions are ANDed together, exluding empty patterns

        match &self.config.user_pattern {
            Value::Regex(pat) if !pat.as_str().is_empty() => {
                let cond = pat.is_match(&chat.src.name);
                if cond {
                    println!(
                        "\x1b[91mUsername {} matches '{}'\x1b[0m",
                        chat.src.name, pat
                    );
                }
                triggered[0] = Some(cond);
            }
            _ => {}
        }

        match &self.config.id_pattern {
            Value::Regex(pat) if !pat.as_str().is_empty() => {
                let cond = pat.is_match(&chat.src.id);
                if cond {
                    println!("\x1b[91mUser id {} matches '{}'\x1b[0m", chat.src.id, pat);
                }
                triggered[1] = Some(cond);
            }
            _ => {}
        }

        match &self.config.msg_pattern {
            Value::Regex(pat) if !pat.as_str().is_empty() => {
                let cond = pat.is_match(&chat.msg);
                if cond {
                    println!(
                        "\x1b[91mMessage from {} matches '{}'\x1b[0m",
                        chat.src.name, pat
                    );
                }
                triggered[2] = Some(cond);
            }
            _ => {}
        }

        // None => filter not enabled
        // Some(false) => filter not tripped
        // Some(true) => tripped

        // returns false if any enabled filter was left untripped, otherwise returns true if any filter was tripped
        let (_, tripped) = triggered
            .into_iter()
            .fold((true, false), |acc, res| match (acc, res) {
                (_, Some(false)) => (false, false),
                ((true, _), Some(true)) => (true, true),
                _ => acc,
            });

        //println!("triggered: {:?}, tripped: {}", triggered, tripped);

        if tripped {
            Some(filter_action)
        } else {
            Some(RunResult::Ok)
        }
    }
}

// !mod
// !mod <msg>
static SUMMON_MOD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)(\s(.{1,50}))?").unwrap());

impl SummonMod {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<SummonModArgs> {
        let captures = SUMMON_MOD_REGEX.captures(&ctx.chat.msg)?;

        // check prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        let msg = captures.get(2).map(|m| m.as_str().trim().to_owned());

        Some(SummonModArgs { msg })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let SummonModArgs { msg } = self.can_run(ctx).await?;
        let chat = ctx.chat;

        // check if we have someone to ping
        let pingee_id = match &self.config.discord_id {
            Value::String(id) => id,
            _ => return None,
        };

        // send ping request
        Response::new(
            (chat.src.platform, &chat.src.name, &chat.src.id),
            Payload::PingRequest(&chat.src, pingee_id, msg.as_deref()),
        )
        .send(self.redis.clone())
        .await?;

        Some(RunResult::Ok)
    }
}

static GIVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s@?(.+)\s(\d+|all)\s*").unwrap());

impl Give {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<GiveArgs> {
        let chat = ctx.chat;
        let captures = GIVE_REGEX.captures(&chat.msg)?;
        // println!("captures: {:?}", captures);
        // check command prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        // get dest
        let to = captures[2].to_owned();

        // check if src != dest
        if chat.src.name == to {
            return None;
        }

        // parse and validate wager
        let amount = if &captures[2] == "all" {
            -1
        } else {
            captures[2].parse::<i32>().ok()?
        };

        Some(GiveArgs { amount, to })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        let args = self.can_run(ctx).await?;
        println!("{:?}", self.dump());

        let deduct_sql = match chat.src.platform {
            Platform::Youtube => include_str!("../sql/update/decr_points_youtube.sql"),
            Platform::Discord => include_str!("../sql/update/decr_points_discord.sql"),
            Platform::Twitch => include_str!("../sql/update/decr_points_twitch.sql"),
            Platform::Broadcast | Platform::Web => unreachable!(),
        };
        let deposit_sql = match chat.src.platform {
            Platform::Youtube => include_str!("../sql/update/incr_points_youtube_name.sql"),
            Platform::Broadcast => unreachable!(),
            _ => todo!(),
        };

        let (min, max) = match (&self.config.min_amount, &self.config.max_amount) {
            (Value::Number(min), Value::Number(max)) => (*min as i32, *max as i32),
            _ => return None,
        };

        // start transaction
        let mut client = self.db.get().await.unwrap();
        let client = client.build_transaction().start().await.ok()?;

        // TODO: query points for "!gamble all", currently "!gamble all" just means !gamble max_amt
        let amount = if args.amount == -1 {
            // all
            let points_sql = match chat.src.platform {
                Platform::Youtube => include_str!("../sql/select/youtube_id_lock.sql"),
                Platform::Broadcast => unreachable!(),
                _ => todo!(),
            };

            // query points
            let amount = client.query_one(points_sql, &[&chat.src.id]).await.unwrap();

            amount.get::<_, i32>(2_usize)
        } else {
            args.amount
        };

        if amount < min {
            return None;
        }

        // clamp amount
        let amount = amount.min(max);

        // try deducting from src
        let decremented = client
            .query(deduct_sql, &[&chat.src.id, &amount])
            .await
            .unwrap();

        // rollback on failure
        if decremented.is_empty() {
            println!(
                "\x1b[91mFailed to deduct {} point(s) from {}\x1b[0m",
                chat.src.name, args.amount
            );
            return None;
        }

        // try depositing into dest
        let incremented = client
            .query(deposit_sql, &[&args.to, &amount])
            .await
            .unwrap();

        // rollback on failure
        if incremented.is_empty() {
            println!(
                "\x1b[91mFailed to deposit {} point(s) into {}\x1b[0m",
                args.amount, args.to
            );
            return None;
        }

        // commit transaction
        client.commit().await.ok()?;

        let msg = format!("gave {} {} point(s)", args.to, args.amount);

        // send reply
        println!("{} {}", chat.src.name, msg);
        Response::new(
            (chat.src.platform, &chat.src.name, &chat.src.id),
            Payload::Message(&msg),
        )
        .send(self.redis.clone())
        .await?;

        Some(RunResult::Ok)
    }
}

impl Gamble {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<GambleArgs> {
        let chat = ctx.chat;
        util::one_arg_matches(&self.config.name, chat).map(|amount| GambleArgs { amount })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        let args = self.can_run(ctx).await?;
        println!("{:?}", self.dump());
        println!("{:?}", args);

        let deduct_sql = match chat.src.platform {
            Platform::Youtube => include_str!("../sql/update/decr_points_youtube.sql"),
            Platform::Discord => include_str!("../sql/update/decr_points_discord.sql"),
            Platform::Twitch => include_str!("../sql/update/decr_points_twitch.sql"),
            _ => unreachable!(),
        };
        let deposit_sql = match chat.src.platform {
            Platform::Youtube => include_str!("../sql/upsert/youtube_id.sql"),
            Platform::Discord => include_str!("../sql/upsert/discord_id.sql"),
            Platform::Twitch => include_str!("../sql/upsert/twitch_id.sql"),
            _ => unreachable!(),
        };

        let (min, max) = match (&self.config.min_amount, &self.config.max_amount) {
            (Value::Number(min), Value::Number(max)) => (*min as i32, *max as i32),
            _ => return None,
        };

        // start transaction
        let mut client = self.db.get().await.unwrap();
        let client = client.build_transaction().start().await.ok()?;

        let amount = if args.amount == -1 {
            // all
            let points_sql = match chat.src.platform {
                Platform::Youtube => include_str!("../sql/select/youtube_id_lock.sql"),
                Platform::Broadcast => unreachable!(),
                _ => todo!(),
            };

            // query points
            let amount = client.query_one(points_sql, &[&chat.src.id]).await.unwrap();

            amount.get::<_, i32>(2_usize)
        } else {
            args.amount
        };

        if amount < min {
            return None;
        }

        // clamp amount
        let amount = amount.min(max);

        // try to deduct wager
        let decremented = client
            .query(deduct_sql, &[&chat.src.id, &amount])
            .await
            .unwrap();
        if decremented.is_empty() {
            println!(
                "{} does not have enough points for wager of {}",
                chat.src.name, amount
            );
            return None;
        }

        // roll dice
        let dice_roll: i32 = Uniform::from(1..=100).sample(&mut rand::thread_rng());
        let winnings = if dice_roll > 90 {
            // triple wager, 1/10
            amount * 3
        } else if dice_roll > 50 {
            // double wager, 4/10
            amount * 2
        } else {
            // lose 5/10
            0
        };

        // deposit winnings if any, and reply
        if winnings > 0 {
            let incremented = client
                .query(deposit_sql, &[&chat.src.id, &chat.src.name, &winnings])
                .await
                .unwrap();

            // rollback on failure
            if incremented.is_empty() {
                println!(
                    "\x1b[91mFailed to deposit {} point(s) into {}\x1b[0m",
                    winnings, chat.src.name
                );
                return None;
            }

            client.commit().await.ok()?;

            let msg = format!(
                "rolled {}, won {} points ratJAM",
                dice_roll,
                winnings - amount
            );

            // send reply
            println!("{} {}", chat.src.name, msg);
            Response::new(
                (chat.src.platform, &chat.src.name, &chat.src.id),
                Payload::Message(&msg),
            )
            .send(self.redis.clone())
            .await?;
        } else {
            client.commit().await.ok()?;

            // send reply
            let msg = format!("rolled {}, lost {} points NOOOO", dice_roll, amount);
            println!("{} {}", chat.src.name, msg);
            Response::new(
                (chat.src.platform, &chat.src.name, &chat.src.id),
                Payload::Message(&msg),
            )
            .send(self.redis.clone())
            .await?;
        }

        Some(RunResult::Ok)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Heister {
    user: msg::User,
    amount: i32,
}

impl Heist {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<HeistArgs> {
        let chat = ctx.chat;
        util::one_arg_matches(&self.config.name, chat).map(|amount| HeistArgs { amount })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        let args = self.can_run(ctx).await?;
        //println!("{:?}", self.dump());

        // get heist duration
        let duration = match self.config.duration {
            Value::Number(t) => Duration::from_secs(t as u64),
            _ => return None,
        };

        // check if heist is currently running
        let starting_heist = acquire_lock(
            self.redis.clone(),
            &*HEIST_LOCK_ACTIVE,
            duration.as_secs() + 5,
        )
        .await?;

        // // don't use HLEN as the actual field doesn't get set till later, whereas HEIST_ACTIVE_KEY gets locked immediately

        let amount = if args.amount == -1 {
            // all
            let points_sql = match chat.src.platform {
                Platform::Youtube => include_str!("../sql/select/youtube_id_lock.sql"),
                Platform::Broadcast => unreachable!(),
                _ => todo!(),
            };

            // query points
            let client = self.db.get().await.unwrap();
            let amount = client.query_one(points_sql, &[&chat.src.id]).await.unwrap();

            amount.get::<_, i32>(2_usize)
        } else {
            args.amount
        };

        self.handle_heist(amount, chat).await?;

        if starting_heist {
            println!("\x1b[92mStarted a heist\x1b[0m");
            Response::new(
                (Platform::Broadcast, "", ""),
                Payload::Message(&format!("{} started a heist!", chat.src.name)),
            )
            .send(self.redis.clone())
            .await?;

            let redis = self.redis.clone();
            let db = self.db.clone();

            tokio::spawn(async move {
                // defer heist closeup till end of duration
                tokio::time::sleep(duration).await;
                Self::end_heist(redis, db).await.unwrap();
            });

            // // TODO: prevent new heisters from joining some seconds before ending the heist, i.e
        } else {
            println!("\x1b[93mJoined currently active heist\x1b[0m");
            Response::new(
                (Platform::Broadcast, "", ""),
                Payload::Message(&format!("{} joined the heist!", chat.src.name)),
            )
            .send(self.redis.clone())
            .await?;
        }

        Some(RunResult::Ok)
    }

    async fn handle_heist(&self, amount: i32, chat: &msg::Chat) -> Option<()> {
        let deduct_sql = match chat.src.platform {
            Platform::Youtube => include_str!("../sql/update/decr_points_youtube.sql"),
            Platform::Broadcast => unreachable!(),
            _ => todo!(),
        };

        let (min, max) = match (&self.config.min_amount, &self.config.max_amount) {
            (Value::Number(min), Value::Number(max)) => (*min as i32, *max as i32),
            _ => return None,
        };

        // start transaction
        let mut client = self.db.get().await.unwrap();
        let client = client.build_transaction().start().await.ok()?;

        let amount = if amount == -1 {
            // all
            let points_sql = match chat.src.platform {
                Platform::Youtube => include_str!("../sql/select/youtube_id_lock.sql"),
                Platform::Broadcast => unreachable!(),
                _ => todo!(),
            };

            // query points
            let amount = client.query_one(points_sql, &[&chat.src.id]).await.unwrap();

            amount.get::<_, i32>(2_usize)
        } else {
            amount
        };

        if amount < min {
            return None;
        }

        // clamp amount
        let amount = amount.min(max);

        // try to deduct wager
        let decremented = client
            .query(deduct_sql, &[&chat.src.id, &amount])
            .await
            .unwrap();
        if decremented.is_empty() {
            println!(
                "{} does not have enough points for wager of {}",
                chat.src.name, amount
            );
            return None;
        }

        let heister = Heister {
            user: chat.src.clone(),
            amount: amount * 2, //TODO: turn multiplier into config var
        };

        let serialised_heister =
            tokio::task::spawn_blocking(move || serde_json::to_string(&heister).unwrap())
                .await
                .unwrap();

        // join heist, exit if failed
        if !util::set_field(
            self.redis.clone(),
            &*HEIST_LOCK_MEMBERS,
            &chat.src.id,
            &serialised_heister,
            true,
        )
        .await?
        {
            return None;
        }

        // commit transaction
        client.commit().await.ok()
    }

    async fn end_heist(redis_pool: RedisPool, db: DbPool) -> Option<bool> {
        println!("\x1b[92mHeist over!\x1b[0m");

        // get heist members
        let mut redis = redis_pool.get().await.ok()?;
        let num_heisters = redis::cmd("HLEN")
            .arg(&*HEIST_LOCK_MEMBERS)
            .query_async::<redis::aio::Connection, usize>(&mut redis)
            .await
            .ok()?;

        // choose survivor count from [0,num_heisters]
        let num_survivors = Uniform::from(0..=num_heisters).sample(&mut rand::thread_rng());

        println!(
            "Out of {} heisters, {} survived",
            num_heisters, num_survivors
        );

        let mut survivor_list = "The heist is over! Survivors: ".to_owned();

        let msg = if num_survivors > 0 {
            let survivors = redis::cmd("HRANDFIELD")
                .arg(&[&*HEIST_LOCK_MEMBERS, &num_survivors.to_string()])
                .query_async::<redis::aio::Connection, Vec<String>>(&mut redis)
                .await
                .ok()?;
            println!("survivors: {:?}", survivors);

            // start transaction
            let client = db.get().await.unwrap();
            //let client = client.build_transaction().start().await.ok()?;
            //let cloop = &client;
            //{
            for (i, id) in survivors.iter().enumerate() {
                // get json stored at HEIST_MEMBERS_KEY[id]
                let survivor = redis::cmd("HGET")
                    .arg(&[&*HEIST_LOCK_MEMBERS, id])
                    .query_async::<redis::aio::Connection, String>(&mut redis)
                    .await
                    .ok()?;

                // deserialise into Heister
                let survivor = tokio::task::spawn_blocking(move || {
                    serde_json::from_str::<Heister>(&survivor).unwrap()
                })
                .await
                .unwrap();
                assert_eq!(id, &survivor.user.id);
                println!("survivor: {:?}", survivor);

                let deposit_sql = match survivor.user.platform {
                    Platform::Youtube => include_str!("../sql/upsert/youtube_id.sql"),
                    Platform::Discord => include_str!("../sql/upsert/discord_id.sql"),
                    Platform::Twitch => include_str!("../sql/upsert/twitch_id.sql"),
                    _ => unreachable!(),
                };

                // deposit points
                let incremented = client
                    .query(
                        deposit_sql,
                        &[&survivor.user.id, &survivor.user.name, &survivor.amount],
                    )
                    .await
                    .unwrap();

                // rollback on failure
                if incremented.is_empty() {
                    println!(
                        "\x1b[91mFailed to deposit {} point(s) into {}\x1b[0m",
                        survivor.amount, survivor.user.name
                    );
                    return None;
                }

                // add survivors' names and winnings to reply
                survivor_list.push_str(&survivor.user.name);
                survivor_list.push_str(&format!(" ({})", survivor.amount));
                if i < num_survivors - 1 {
                    survivor_list.push_str(", ");
                }
            }
            //}
            //client.commit().await.ok()?;

            &survivor_list
        } else {
            "The heist is over, there were no survivors :_monkaW:"
        };

        // send reply
        Response::new((Platform::Broadcast, "", ""), Payload::Message(msg))
            .send(redis_pool.clone())
            .await?;

        // release heist locks
        release_lock(redis_pool.clone(), &*HEIST_LOCK_MEMBERS).await?;
        release_lock(redis_pool.clone(), &*HEIST_LOCK_ACTIVE).await
    }
}

static BAN_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s@?(.+)\s*").unwrap());

impl Ban {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<BanArgs> {
        let chat = ctx.chat;
        let captures = BAN_REGEX.captures(&chat.msg)?;
        // check command prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        // bannee's name
        let name = captures[2].to_owned();

        Some(BanArgs { name })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        let args = self.can_run(ctx).await?;
        println!("{:?}", self.dump());
        println!("{:?}", args);

        let query_sql = match chat.src.platform {
            Platform::Youtube => include_str!("../sql/select/youtube_name.sql"),
            Platform::Broadcast => unreachable!(),
            _ => todo!(),
        };

        let mut client = self.db.get().await.unwrap();
        let client = client.build_transaction().start().await.ok()?;

        // get id of user
        let res = client.query_one(query_sql, &[&args.name]).await.ok()?;
        let id = res.try_get::<_, &str>(0).ok()?;

        let reason = format!("{} used Ban", chat.src.name);

        // send ban request
        Response::new(
            (chat.src.platform, &args.name, id),
            Payload::ModAction(ModAction::Ban, &reason),
        )
        .send(self.redis.clone())
        .await?;

        Some(RunResult::Ok)
    }
}

impl French {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<FrenchArgs> {
        let chat = ctx.chat;
        util::prefix_matches(&self.config.name, chat)
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        self.can_run(ctx).await?;
        println!("{:?}", self.dump());

        //tokio::spawn(async {
        let resp = reqwest::get("https://blague.xyz/api/joke/random")
            .await
            .unwrap()
            .json::<HashMap<String, serde_json::Value>>()
            .await
            .unwrap();

        if let serde_json::Value::Object(question_answer) = &resp["joke"] {
            let res = format!(
                "{} {}",
                question_answer["question"].as_str()?,
                question_answer["answer"].as_str()?
            );
            println!("{}", &res);

            Response::new(
                (chat.src.platform, &chat.src.name, &chat.src.id),
                Payload::Message(&res),
            )
            .send(self.redis.clone())
            .await?;
        }
        //});

        Some(RunResult::Ok)
    }
}

impl Help {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<HelpArgs> {
        let chat = ctx.chat;
        util::prefix_matches(&self.config.name, chat)
    }

    fn format_cmd(src: &msg::User, cmd: &Command) -> Option<String> {
        // ignore disable commands
        match cmd.get("enabled")? {
            Value::Bool(true) => {}
            _ => return None,
        }

        // ignore commands not available on user's platform
        match cmd.get("platforms") {
            Some(Value::Platforms(platforms))
                if platforms.is_empty() || platforms.contains(&src.platform) => {}
            _ => return None,
        }

        // ignore commands with no prefix (they're not callable)
        let prefix = match cmd.get("name")? {
            Value::String(prefix) if !prefix.is_empty() => prefix,
            _ => return None,
        };

        // ignore commands whose permission level > user's
        let perms = match cmd.get("perms") {
            Some(Value::Permissions(perms)) if src.perms < perms => return None,
            Some(Value::Permissions(perms)) if perms > Permissions::None => {
                format!(", {:?}", perms)
            }
            _ => "".into(),
        };

        // command's name
        let name = cmd.name();
        let desc = cmd.description();

        Some(format!(
            "{} ({}{})\n",
            prefix,
            if name.is_empty() { desc } else { name },
            perms
        ))
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        self.can_run(ctx).await?;

        let src = ctx.chat.src.clone();
        let platform = src.platform;
        let cmdlist_lock = ctx.cmdlist.clone();

        let msg = {
            tokio::task::spawn_blocking(move || {
                let commands = cmdlist_lock.commands.read();
                commands
                    .iter()
                    .filter_map(|cmd| Help::format_cmd(&src, cmd))
                    .fold(String::new(), |mut acc, res| {
                        acc.push_str(&res);
                        acc
                    })
            })
            .await
            .ok()?
        };

        Response::new((platform, "", ""), Payload::Message(&msg))
            .send(self.redis.clone())
            .await?;

        Some(RunResult::Ok)
    }
}

impl Text {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<TextArgs> {
        let chat = ctx.chat;
        util::prefix_matches(&self.config.name, chat)
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        self.can_run(ctx).await?;

        let msg = match &self.config.msg {
            Value::String(msg) => msg,
            _ => return None,
        };

        Response::new(
            (chat.src.platform, &chat.src.name, &chat.src.id),
            Payload::Message(msg),
        )
        .send(self.redis.clone())
        .await?;

        Some(RunResult::Ok)
    }
}

impl ToggleFilters {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<ToggleFiltersArgs> {
        let chat = ctx.chat;
        util::prefix_matches(&self.config.name, chat)
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        self.can_run(ctx).await?;
        println!("{:?}", self.dump());

        let cmdlist_lock = ctx.cmdlist.clone();

        tokio::task::spawn_blocking(move || {
            // acquire write lock on cmdlist
            let mut filters = cmdlist_lock.filters.write();

            // toggle filters
            for filter in &mut *filters {
                if let Some(Value::Bool(x)) = filter.get("enabled") {
                    filter.set("enabled", Value::Bool(!x));
                    println!(
                        "\x1b[93mFilter {:?} {}\x1b[0m",
                        filter.name(),
                        if !x { "enabled" } else { "disabled" }
                    );
                }
            }
        })
        .await
        .ok()?;

        Some(RunResult::Ok)
    }
}

//static ONE_ARG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)(?:\s(\S+))?\s*").unwrap());
static ONE_SUBCMD_1_ARG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)(?:\s(\S+)(?:\s(\d+))?)?\s*").unwrap());

// repackage schema with type and timestamp for versioning
static CONFIG_SCHEMA: Lazy<(&str, &str, Vec<DumpedSchema>)> =
    Lazy::new(|| ("schema", _build_timestamp(), Command::list_config()));
static CONFIG_SCHEMA_JSON: Lazy<String> =
    Lazy::new(|| serde_json::to_string::<(&str, &str, Vec<_>)>(&CONFIG_SCHEMA).unwrap());

impl Dump {
    /// Parse subcommend args for !dump
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<DumpArgs> {
        let chat = ctx.chat;
        let captures = ONE_SUBCMD_1_ARG_REGEX.captures(&chat.msg)?;
        //println!("{:?}", captures);

        // check command prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        // parse subcommand and args if any
        let what = Self::parse_args_helper(&captures);
        Some(DumpArgs { what })
    }

    fn parse_args_helper(captures: &regex::Captures) -> Option<DumpSetWhat> {
        if let Some(arg) = captures.get(2).map(|m| m.as_str()) {
            if arg == "c" {
                if let Some(arg) = captures.get(3).map(|m| m.as_str()) {
                    if let Ok(i) = arg.parse::<usize>() {
                        return Some(DumpSetWhat::CmdIndex(i));
                    }
                } else {
                    return Some(DumpSetWhat::Commands);
                }
            } else if arg == "f" {
                if let Some(arg) = captures.get(3).map(|m| m.as_str()) {
                    if let Ok(i) = arg.parse::<usize>() {
                        return Some(DumpSetWhat::FilterIndex(i));
                    }
                } else {
                    return Some(DumpSetWhat::Filters);
                }
            } else if arg == "t" {
                if let Some(arg) = captures.get(3).map(|m| m.as_str()) {
                    if let Ok(i) = arg.parse::<usize>() {
                        return Some(DumpSetWhat::TimerIndex(i));
                    }
                } else {
                    return Some(DumpSetWhat::Timers);
                }
            } else if arg == "d" {
                return Some(DumpSetWhat::Disk);
            }
        } else {
            return Some(DumpSetWhat::Schema);
        }
        None
    }

    async fn reply(&self, dest: &msg::User, msg: impl AsRef<str>) -> Option<()> {
        Response::new(
            (dest.platform, &dest.name, &dest.id),
            Payload::Message(msg.as_ref()),
        )
        .send(self.redis.clone())
        .await
    }

    fn dump_cmds(dumpee: &[Command]) -> Option<String> {
        let dumps: Vec<DumpedCommand> = dumpee.iter().map(Command::dump).collect();

        // repackage dumps with dump type and timestamp (versioning)
        let dumps = ("c", _build_timestamp(), dumps);

        let msg = serde_json::to_string(&dumps).ok()?;
        println!("{}", msg);
        Some(msg)
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let chat = ctx.chat;
        let args = self.can_run(ctx).await?;
        println!("{:?}", args);

        let cmdlist_lock = ctx.cmdlist.clone();

        match args.what {
            Some(DumpSetWhat::Commands) => {
                println!("\x1b[93mDumping all cmds\x1b[0m");
                let msg = tokio::task::spawn_blocking(move || {
                    let commands = cmdlist_lock.commands.read();
                    Self::dump_cmds(&commands).unwrap()
                })
                .await
                .ok()?;
                self.reply(&chat.src, msg).await?;
            }
            Some(DumpSetWhat::Filters) => {
                println!("\x1b[93mDumping all filters\x1b[0m");
                let msg = tokio::task::spawn_blocking(move || {
                    let filters = cmdlist_lock.filters.read();
                    Self::dump_cmds(&filters).unwrap()
                })
                .await
                .ok()?;
                self.reply(&chat.src, msg).await?;
            }
            Some(DumpSetWhat::Timers) => {
                println!("\x1b[93mDumping all timers\x1b[0m");
                let msg = tokio::task::spawn_blocking(move || {
                    let timers = cmdlist_lock.timers.read();
                    Self::dump_cmds(&timers).unwrap()
                })
                .await
                .ok()?;
                self.reply(&chat.src, msg).await?;
            }
            Some(DumpSetWhat::CmdIndex(i)) => {
                println!("\x1b[93mDumping cmd at index {} \x1b[0m", i);
                let msg = tokio::task::spawn_blocking(move || {
                    let commands = cmdlist_lock.commands.read();
                    if i >= commands.len() {
                        return None;
                    }
                    Self::dump_cmds(std::slice::from_ref(&commands[i]))
                })
                .await
                .ok()??;
                self.reply(&chat.src, msg).await?;
            }
            Some(DumpSetWhat::FilterIndex(i)) => {
                println!("\x1b[93mDumping filter at index {} \x1b[0m", i);
                let msg = tokio::task::spawn_blocking(move || {
                    let filters = cmdlist_lock.filters.read();
                    if i >= filters.len() {
                        return None;
                    }
                    Self::dump_cmds(std::slice::from_ref(&filters[i]))
                })
                .await
                .ok()??;
                self.reply(&chat.src, msg).await?;
            }
            Some(DumpSetWhat::TimerIndex(i)) => {
                println!("\x1b[93mDumping timers at index {} \x1b[0m", i);
                let msg = tokio::task::spawn_blocking(move || {
                    let timers = cmdlist_lock.timers.read();
                    if i >= timers.len() {
                        return None;
                    }
                    Self::dump_cmds(std::slice::from_ref(&timers[i]))
                })
                .await
                .ok()??;
                self.reply(&chat.src, msg).await?;
            }
            Some(DumpSetWhat::Schema) => {
                println!("\x1b[93mDumping schema\x1b[0m");
                println!("{}", CONFIG_SCHEMA_JSON.as_str());
                self.reply(&chat.src, CONFIG_SCHEMA_JSON.as_str()).await?;
            }
            Some(DumpSetWhat::Disk) => {
                println!("\x1b[93mDumping config to disk\x1b[0m");
                let [cmds_file, filters_file, timers_file] = util::open_config_files()?;

                tokio::task::spawn_blocking(move || {
                    // dump cmds
                    let commands = cmdlist_lock.commands.read();
                    let dumps: Vec<DumpedCommand> = commands.iter().map(Command::dump).collect();
                    serde_json::to_writer_pretty(cmds_file, &dumps).unwrap();
                    // dump filters
                    let filters = cmdlist_lock.filters.read();
                    let dumps: Vec<DumpedCommand> = filters.iter().map(Command::dump).collect();
                    serde_json::to_writer_pretty(filters_file, &dumps).unwrap();
                    // dump timers
                    let timers = cmdlist_lock.timers.read();
                    let dumps: Vec<DumpedCommand> = timers.iter().map(Command::dump).collect();
                    serde_json::to_writer_pretty(timers_file, &dumps).unwrap();
                    println!("\x1b[92mSuccessfully dumped\x1b[0m");
                })
                .await
                .ok()?;

                //self.reply(&chat.src, "config dumped to disk 💾").await?;
            }
            None => {}
        }

        Some(RunResult::Ok)
    }
}

static SET_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)(?:\s(\S+)(?:\s(\d+))?)?(?:\s(.+))?$").unwrap());

impl Set {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<SetArgs> {
        let chat = ctx.chat;
        let captures = SET_REGEX.captures(&chat.msg)?;
        //println!("set: {:?}", captures);

        // check command prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        // parse subcommand and args if any
        let what = Dump::parse_args_helper(&captures);

        // json
        let json = match captures.get(4).map(|m| m.as_str()) {
            Some(json) => json.to_owned(),
            _ if matches!(what, Some(DumpSetWhat::Disk)) => "".into(), // ignore json if loading from disk
            _ => return None,
        };

        // can't set schema
        if matches!(what, Some(DumpSetWhat::Schema) | None) {
            return None;
        }

        Some(SetArgs { what, json })
    }

    pub fn deserialise_commands(
        de_list: Vec<OwnedDumpedCmd>,
        redis: RedisPool,
        db: DbPool,
    ) -> Vec<Command> {
        de_list
            .into_iter() // consume de_list
            .map(|(cmd_type, name, keys)| {
                // create a new cmd of type cmd_type
                let mut new_cmd =
                    Command::new(&cmd_type, &name, redis.clone(), db.clone()).unwrap();
                for (key, value) in keys {
                    new_cmd.set(key, value); // try to set key
                }
                if !matches!(new_cmd.get("enabled"), Some(Value::Bool(true))) {
                    print!("\x1b[90m");
                }
                if name.is_empty() {
                    println!("Imported {}", cmd_type);
                } else {
                    println!("Imported {}({})", cmd_type, name);
                }
                print!("\x1b[0m");
                new_cmd
            })
            .collect::<Vec<Command>>()
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let args = self.can_run(ctx).await?;
        println!("{:?}", args);

        let notify_type = if !matches!(args.what, Some(DumpSetWhat::Disk)) {
            let json = args.json;
            // deserialise json
            let de_list = serde_json::from_str::<Vec<OwnedDumpedCmd>>(json.as_str()).ok()?;
            // collect now to avoid panicking while holding the write lock if new_cmd.set fails
            let new_commands =
                Self::deserialise_commands(de_list, self.redis.clone(), self.db.clone());
            // acquire write lock

            let cmdlist_lock = ctx.cmdlist.clone();

            // returns notify_type
            match args.what? {
                DumpSetWhat::Commands => {
                    tokio::task::spawn_blocking(move || {
                        let mut commands = cmdlist_lock.commands.write(); //.unwrap();
                        *commands = new_commands;
                        drop(commands);
                        println!("\x1b[32mCommands imported\x1b[0m");
                        NotifyType::Commands
                    })
                    .await
                    .ok()?
                }
                DumpSetWhat::Filters => {
                    tokio::task::spawn_blocking(move || {
                        let mut filters = cmdlist_lock.filters.write(); //.unwrap();
                        *filters = new_commands;
                        drop(filters);
                        println!("\x1b[32mFilters imported\x1b[0m");
                        NotifyType::Filters
                    })
                    .await
                    .ok()?
                }
                DumpSetWhat::Timers => {
                    // abort previous Reminder tasks
                    {
                        let old_timer_handles = cmdlist_lock.timer_handles.read();
                        for handle in &*old_timer_handles {
                            handle.abort();
                        }
                    }

                    // init timers
                    let handles = Timer::start(&new_commands, ctx.cmdlist.is_main_instance)
                        .await
                        .unwrap();

                    // acquire lock and set config
                    tokio::task::spawn_blocking(move || {
                        let mut timers = cmdlist_lock.timers.write(); //.unwrap();
                        *timers = new_commands;
                        drop(timers);
                        let mut timer_handles = cmdlist_lock.timer_handles.write(); //.unwrap();
                        *timer_handles = handles;
                        drop(timer_handles);
                        NotifyType::Timers
                    })
                    .await
                    .ok()?
                }
                _ => return None,
            }
        } else {
            let old_timer_handles = ctx.cmdlist.timer_handles.read().clone();

            // Set new config
            Self::set_config(
                ctx.cmdlist.is_main_instance,
                &old_timer_handles,
                ctx.cmdlist.clone(),
                self.redis.clone(),
                self.db.clone(),
            )
            .await?;

            NotifyType::Config
        };

        Response::new(
            (Platform::Broadcast, "", ""),
            Payload::Notify(ctx.chat.src.platform, notify_type),
        )
        .send(self.redis.clone())
        .await?;

        Some(RunResult::Ok)
    }

    /// Set config to new_cmdlist
    pub async fn set_config(
        is_main_instance: bool,
        timer_handles: &[TimerHandle],
        new_cmdlist: CommandList,
        redis: RedisPool,
        db: DbPool,
    ) -> Option<()> {
        // Load from disk
        let new_cfg =
            tokio::task::spawn_blocking(move || util::load_config(redis, db, is_main_instance))
                .await
                .ok()??; //lmao wut??

        // abort previous Reminder tasks
        for handle in timer_handles {
            handle.abort();
        }

        let handles = Timer::start(&new_cfg.timers, is_main_instance)
            .await
            .unwrap();

        tokio::task::spawn_blocking(move || {
            // set new config
            let mut commands = new_cmdlist.commands.write(); //.unwrap();
            *commands = new_cfg.commands;
            drop(commands);
            let mut filters = new_cmdlist.filters.write(); //.unwrap();
            *filters = new_cfg.filters;
            drop(filters);
            let mut timers = new_cmdlist.timers.write(); //.unwrap();
            *timers = new_cfg.timers;
            drop(timers);
            let mut timer_handles = new_cmdlist.timer_handles.write(); //.unwrap();
            *timer_handles = handles;
            drop(timer_handles);
        })
        .await
        .ok()?;

        Some(())
    }
}

impl Points {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<PointsArgs> {
        util::prefix_matches(&self.config.name, ctx.chat)
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        self.can_run(ctx).await?;

        let src = &ctx.chat.src;

        let client = self.db.get().await.unwrap();

        let sql = match src.platform {
            Platform::Youtube => include_str!("../sql/select/points_ytid.sql"),
            Platform::Discord => include_str!("../sql/select/points_discid.sql"),
            Platform::Twitch => include_str!("../sql/select/points_twid.sql"),
            _ => unreachable!(),
        };

        // query view
        let row = client.query_one(sql, &[&src.id]).await.unwrap();

        let youtube_points = row.try_get::<_, i32>(3).ok();
        let discord_points = row.try_get::<_, i32>(4).ok();
        let twitch_points = row.try_get::<_, i32>(5).ok();

        let mut msg = "points: ".to_owned();

        if let Some(points) = youtube_points {
            msg.push_str(&format!("{} (youtube), ", points));
        } else {
            msg.push_str("- (youtube), ");
        }

        if let Some(points) = discord_points {
            msg.push_str(&format!("{} (discord), ", points));
        } else {
            msg.push_str("- (discord), ");
        }

        if let Some(points) = twitch_points {
            msg.push_str(&format!("{} (twitch), ", points));
        } else {
            msg.push_str("- (twitch), ");
        }

        msg.truncate(msg.chars().count() - 2);

        Response::new(
            (src.platform, &src.name, &src.id),
            Payload::Message(msg.as_ref()),
        )
        .send(self.redis.clone())
        .await?;

        Some(RunResult::Ok)
    }
}

static TRANSFER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\S+)\s(\d+|all)\sfrom\s(\S+)\sto\s(\S+)\s*$").unwrap());

impl Transfer {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<TransferArgs> {
        let chat = ctx.chat;
        let captures = TRANSFER_REGEX.captures(&chat.msg)?;
        //println!("transfer: {:?}", captures);

        // check command prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        // amount to transfer
        let amount = if &captures[2] == "all" {
            -1
        } else {
            captures[2].parse::<i32>().unwrap()
        };

        let from = Platform::from_str(&captures[3]).unwrap();
        let to = Platform::from_str(&captures[4]).unwrap();

        // can't transfer to the same platform
        if to == from {
            return None;
        }

        Some(TransferArgs { amount, from, to })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let args = self.can_run(ctx).await?;
        println!("{:?}", args);

        let src = &ctx.chat.src;

        // check if user has both 'to' and 'from' platforms linked
        let sql = match src.platform {
            Platform::Youtube => include_str!("../sql/select/points_ytid.sql"),
            Platform::Discord => include_str!("../sql/select/points_discid.sql"),
            Platform::Twitch => include_str!("../sql/select/points_twid.sql"),
            _ => unreachable!(),
        };

        let (min, max) = match (&self.config.min_amount, &self.config.max_amount) {
            (Value::Number(min), Value::Number(max)) => (*min as i32, *max as i32),
            _ => return None,
        };

        let mut client = self.db.get().await.unwrap();

        // query view
        let row = client.query_one(sql, &[&src.id]).await.unwrap();

        let get_id = |p: Platform| match p {
            Platform::Youtube => row.try_get::<_, String>(0).ok(),
            Platform::Discord => row.try_get::<_, String>(1).ok(),
            Platform::Twitch => row.try_get::<_, String>(2).ok(),
            _ => None,
        };

        // check if 'to' platform is linked
        let to_id = get_id(args.to).unwrap();

        // check if 'from' platform is linked
        let from_id = get_id(args.from).unwrap();

        // get points in 'from' platform
        let from_points = match args.from {
            Platform::Youtube => row.try_get::<_, i32>(3),
            Platform::Discord => row.try_get::<_, i32>(4),
            Platform::Twitch => row.try_get::<_, i32>(5),
            _ => return None,
        }
        .ok()?;

        let amount = if args.amount == -1 {
            from_points // all
        } else {
            args.amount.min(from_points)
        };

        if amount < min {
            return None;
        }

        // clamp amount
        let amount = amount.min(max);

        // basically a Give but the target is self
        let deduct_sql = match args.from {
            Platform::Youtube => include_str!("../sql/update/decr_points_youtube.sql"),
            Platform::Discord => include_str!("../sql/update/decr_points_discord.sql"),
            Platform::Twitch => include_str!("../sql/update/decr_points_twitch.sql"),
            Platform::Broadcast | Platform::Web => unreachable!(),
        };
        let deposit_sql = match args.to {
            Platform::Youtube => include_str!("../sql/update/incr_points_youtube_id.sql"),
            Platform::Discord => include_str!("../sql/update/incr_points_discord_id.sql"),
            Platform::Twitch => include_str!("../sql/update/incr_points_twitch_id.sql"),
            Platform::Broadcast | Platform::Web => unreachable!(),
        };

        // update db
        let client = client.build_transaction().start().await.ok()?;
        let _ = client
            .query_one(deduct_sql, &[&from_id, &amount])
            .await
            .unwrap();
        let _ = client
            .query_one(deposit_sql, &[&to_id, &amount])
            .await
            .unwrap();
        client.commit().await.unwrap();

        let msg = format!(
            "transfered {} points(s) from {:?} to {:?}!",
            amount, args.from, args.to
        );

        Response::new((src.platform, &src.name, &src.id), Payload::Message(&msg))
            .send(self.redis.clone())
            .await?;

        Some(RunResult::Ok)
    }
}

impl Timer {
    const fn parse_arguments(&self, _: &Context<'_>) -> Option<TimerArgs> {
        Some(TimerArgs {})
    }

    // Chat msg counter for Timer
    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        // don't count messages for Timers with no msg_count trigger set
        if !matches!(&self.config.msg_count, Value::Number(1..)) {
            return None;
        }

        self.can_run(ctx).await?;

        let mut redis = self.redis.get().await.unwrap();

        let count_key = format!("{}_{}", &*TIMER_LOCK_COUNT, self.name);

        // atomically increment count
        redis::cmd("INCR")
            .arg(&[&count_key])
            .query_async::<redis::aio::Connection, u64>(&mut redis)
            .await
            .ok()?;

        Some(RunResult::Ok)
    }

    /// handle spawing Timer task
    async fn init(&self) -> Option<TimerHandle> {
        if !matches!(self.config.enabled, Value::Bool(true)) {
            return None;
        }

        // name's used as a key so it can't be empty
        if self.name.is_empty() {
            return None;
        }

        // ignore empty timers
        let msg = if let Value::String(msg) = &self.config.msg {
            if msg.is_empty() {
                return None;
            }
            msg.clone()
        } else {
            return None;
        };

        let platforms = if let Value::Platforms(platforms) = &self.config.platforms {
            if platforms.is_empty() {
                vec![Platform::Youtube, Platform::Discord, Platform::Twitch]
            } else {
                platforms.to_vec()
            }
        } else {
            return None;
        };

        let interval = if let Value::Number(time) = self.config.interval {
            time as u64
        } else {
            return None;
        };

        let jitter = if let Value::Number(jitter) = self.config.jitter {
            jitter as u64
        } else {
            0
        };

        let trigger_count = if let Value::Number(count) = self.config.msg_count {
            count as u64
        } else {
            0
        };

        let redis = self.redis.clone();
        let timer_name = self.name.clone();
        let jitter_dist = Uniform::from(0..=jitter);
        let count_key = format!("{}_{}", &*TIMER_LOCK_COUNT, self.name);

        println!(
            "\x1b[93mSpawning Timer {:?} with interval: {}s, max jitter: {}s\x1b[0m",
            timer_name, interval, jitter
        );

        let h = tokio::spawn(async move {
            loop {
                // sleep with random jitter
                let jitter = jitter_dist.sample(&mut rand::thread_rng());
                tokio::time::sleep(Duration::from_secs(interval.saturating_add(jitter))).await;
                // println!(
                //     "\x1b[93mChecking to see if Timer {} can run\x1b[0m",
                //     timer_name
                // );

                // check msg count
                if trigger_count > 0 {
                    // get and clear count
                    let mut conn = redis.get().await.unwrap();
                    let count = if let Ok(count) = redis::cmd("SET")
                        .arg(&[&count_key, "0", "GET"])
                        .query_async::<redis::aio::Connection, u64>(&mut conn)
                        .await
                    {
                        count
                    } else {
                        0
                    };
                    println!(
                        "\x1b[93mTimer {} msg count: {}, trigger count: {}\x1b[0m",
                        timer_name, count, trigger_count
                    );

                    // check if enough msgs have been received
                    if count < trigger_count {
                        continue;
                    }
                }

                println!(
                    "\x1b[93mIn Timer: {}, interval: {}, jitter: {}\x1b[0m",
                    timer_name, interval, jitter
                );
                for platform in &platforms {
                    println!(
                        "\x1b[93mSending Timer {}'s msg to {:?}\x1b[0m",
                        timer_name, platform
                    );
                    Response::new((*platform, "", ""), Payload::Message(&msg))
                        .send(redis.clone())
                        .await;
                }
            }
        });

        Some(Arc::new(h))
    }

    /// Used for (re)starting Timers after a config change
    ///
    /// Previous Timer tasks should be aborted before calling this
    pub async fn start(new_timers: &[Command], is_main_instance: bool) -> Option<Vec<TimerHandle>> {
        // only main instance'll handle timers
        if !is_main_instance {
            return None;
        }

        // start new tasks
        let mut handles: Vec<Option<TimerHandle>> = vec![];
        for cmd in new_timers {
            if let Command::Timer(reminder) = cmd {
                handles.push(Timer::init(reminder).await);
            }
        }

        // flatten handles, discarding None
        Some(handles.into_iter().flatten().collect::<Vec<TimerHandle>>())
    }
}

// !stream <platform> (<vid>|stop)
static STREAM_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s(\S+)\s(\S+)\s*").unwrap());
pub static YOUTUBE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"youtube\.com/watch\?v=(\S+)$").unwrap());

impl Stream {
    fn parse_arguments(&self, ctx: &Context<'_>) -> Option<StreamArgs> {
        let chat = ctx.chat;
        let captures = STREAM_REGEX.captures(&chat.msg)?;
        //println!("transfer: {:?}", captures);

        // check command prefix
        match &self.config.name {
            Value::String(pat) if pat.as_str() == &captures[1] => {}
            _ => return None,
        }

        // parse args
        let platform = Platform::from_str(&captures[2])?;
        let vid = captures[3].to_owned();

        Some(StreamArgs { platform, vid })
    }

    async fn run(&self, ctx: &Context<'_>) -> Option<RunResult> {
        let StreamArgs { platform, vid } = self.can_run(ctx).await?;

        let signal = if !vid.starts_with("stop") {
            StreamSignal::Start(&vid)
        } else {
            StreamSignal::Stop
        };

        println!(
            "Stream platform: {:?}, vid: {:?}, signal: {:?}",
            platform, vid, signal
        );

        // send stream signal
        Stream::signal_stream(self.redis.clone(), platform, signal).await?;

        Some(RunResult::Ok)
    }

    pub async fn signal_stream(
        redis: RedisPool,
        platform: Platform,
        signal: StreamSignal<'_>,
    ) -> Option<()> {
        Response::new((platform, "", ""), Payload::Stream(signal))
            .send(redis)
            .await
    }
}
