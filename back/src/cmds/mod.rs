pub(crate) mod filter;
pub(crate) mod give;
pub(crate) mod hours;
pub(crate) mod levenshtein;
pub(crate) mod link;
pub(crate) mod log;
pub(crate) mod memebank;
pub(crate) mod ping;
pub(crate) mod points;
pub(crate) mod quote;
pub(crate) mod reaction_role;
pub(crate) mod regex_filter;
pub(crate) mod russian_roulette;
pub(crate) mod stream;
pub(crate) mod streamlabs;
pub(crate) mod timer;
pub(crate) mod transfer;
pub(crate) mod util;

use crate::{
    cache, db,
    error::{self, Error},
    lock,
    msg::{self, Location, Permissions, Platform, Response, User},
};
use levenshtein_automata::{LevenshteinAutomatonBuilder, DFA};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, path::Path, sync::Arc};
use tokio::{fs, sync::mpsc};

/// cache lowercase versions of chat msg
#[derive(Debug, Clone)]
pub(crate) struct FilterCache {
    id: Arc<String>,
    name: Arc<String>,
    msg: Arc<String>,
}

type RespHandle = mpsc::Sender<(Location, Response)>;

#[derive(Debug)]
pub(crate) struct Context<'a> {
    pub(crate) platform: msg::Platform,
    pub(crate) location: msg::Location,
    pub(crate) user: &'a Arc<msg::User>,
    pub(crate) meta: &'a Option<msg::ChatMeta>,
    pub(crate) db: &'a db::Handle,
    pub(crate) cache: &'a cache::Handle,
    pub(crate) lock: &'a lock::Handle,
    pub(crate) resp: &'a RespHandle, // response channel
    pub(crate) filter_cache: RwLock<Option<FilterCache>>, // cached filtercontext
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CmdType {
    Command,
    Filter,
    Timer,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Constraint {
    None,
    NonEmpty,
    Positive,
    Negative,
    RangeClosed(std::ops::RangeInclusive<i64>),
    RangeHalfOpen(std::ops::Range<i64>),
}

trait VerifyConstraint {
    fn verify(&self, constraint: Constraint) -> bool {
        matches!(constraint, Constraint::None)
    }
}

impl<T: VerifyConstraint> VerifyConstraint for Arc<T> {
    fn verify(&self, _constraint: Constraint) -> bool {
        //self.verify(constraint)
        todo!()
    }
}

impl VerifyConstraint for String {
    fn verify(&self, constraint: Constraint) -> bool {
        match constraint {
            Constraint::None => true,
            Constraint::NonEmpty => !self.is_empty(),
            Constraint::RangeClosed(range) => range.contains(&(self.len() as i64)),
            Constraint::RangeHalfOpen(range) => range.contains(&(self.len() as i64)),
            _ => unreachable!(),
        }
    }
}

impl VerifyConstraint for Regex {
    fn verify(&self, constraint: Constraint) -> bool {
        match constraint {
            Constraint::None => true,
            Constraint::NonEmpty => !self.as_str().is_empty(),
            _ => unreachable!(),
        }
    }
}

impl VerifyConstraint for i64 {
    fn verify(&self, constraint: Constraint) -> bool {
        match constraint {
            Constraint::None => true,
            Constraint::Positive => *self >= 0,
            Constraint::Negative => *self < 0,
            Constraint::RangeClosed(range) => range.contains(self),
            Constraint::RangeHalfOpen(range) => range.contains(self),
            _ => unreachable!(),
        }
    }
}

impl VerifyConstraint for u64 {
    fn verify(&self, constraint: Constraint) -> bool {
        match constraint {
            Constraint::None => true,
            Constraint::Positive => true,
            Constraint::Negative => false,
            Constraint::RangeClosed(range) => range.contains(&(*self as i64)),
            Constraint::RangeHalfOpen(range) => range.contains(&(*self as i64)),
            _ => unreachable!(),
        }
    }
}

impl VerifyConstraint for bool {
    fn verify(&self, constraint: Constraint) -> bool {
        match constraint {
            Constraint::None => true,
            Constraint::Positive => *self,
            Constraint::Negative => !*self,
            _ => unreachable!(),
        }
    }
}

impl VerifyConstraint for Platform {}
impl VerifyConstraint for Permissions {}

impl VerifyConstraint for ModAction {
    fn verify(&self, constraint: Constraint) -> bool {
        match self {
            ModAction::Timeout(t) => match constraint {
                Constraint::None => true,
                Constraint::RangeClosed(range) => range.contains(&(*t as i64)),
                Constraint::RangeHalfOpen(range) => range.contains(&(*t as i64)),
                _ => unreachable!(),
            },
            _ => match constraint {
                Constraint::None | Constraint::RangeClosed(_) | Constraint::RangeHalfOpen(_) => {
                    true
                }
                _ => unreachable!(),
            },
        }
    }
}

impl Default for Constraint {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Value {
    None,
    String(String),
    Number(i64),
    Bool(bool),
    Permissions(u32),
    Platforms(u32),
    Regex(String),
    ModAction(ModAction),
}

impl Default for Value {
    fn default() -> Self {
        Value::None
    }
}

// impl Value {
//     fn verify(&self, constraint: Constraint) -> bool {
//         println!("verify {:?}, constr: {:?}", self, constraint);
//         if matches!(constraint, Constraint::None) {
//             return true;
//         }
//         match self {
//             Value::None => false,
//             Value::Bool(_x) => unimplemented!(),
//             Value::String(x) => match constraint {
//                 Constraint::NonEmpty => !x.is_empty(),
//                 _ => unimplemented!(),
//             },
//             Value::Number(x) => match constraint {
//                 Constraint::Positive => *x > 0,
//                 Constraint::Negative => *x < 0,
//                 Constraint::RangeClosed(range) => range.contains(x),
//                 Constraint::RangeHalfOpen(range) => range.contains(x),
//                 _ => unimplemented!(),
//             },
//             Value::Platforms(_x) => unimplemented!(),
//             Value::Permissions(_x) => unimplemented!(),
//             Value::Regex(x) => match constraint {
//                 Constraint::NonEmpty => !x.is_empty(),
//                 _ => unimplemented!(),
//             },
//             Value::ModAction(ModAction::Timeout(x)) => match constraint {
//                 Constraint::RangeClosed(range) => range.contains(&(*x as i64)),
//                 Constraint::RangeHalfOpen(range) => range.contains(&(*x as i64)),
//                 _ => unimplemented!(),
//             },
//             Value::ModAction(_) => match constraint {
//                 Constraint::RangeClosed(_) | Constraint::RangeHalfOpen(_) => true,
//                 _ => unimplemented!(),
//             },
//         }
//     }
// }

#[derive(Debug)]
pub struct OwnedValueError {
    expected: String,
    value: Value,
}

impl std::fmt::Display for OwnedValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "expected OwnedValue::{}, got {:?}",
            self.expected, self.value
        ))
    }
}

impl std::error::Error for OwnedValueError {}

impl VerifyConstraint for Value {
    fn verify(&self, constraint: Constraint) -> bool {
        match (self, constraint) {
            (_, Constraint::None) => true,
            (Value::String(s), Constraint::NonEmpty) => !s.is_empty(),
            (Value::String(s), Constraint::RangeClosed(range)) => range.contains(&(s.len() as i64)),
            (Value::String(s), Constraint::RangeHalfOpen(range)) => {
                range.contains(&(s.len() as i64))
            }
            (Value::Regex(s), Constraint::NonEmpty) => !s.is_empty(),
            (Value::Number(n), Constraint::Positive) => *n >= 0,
            (Value::Number(n), Constraint::Negative) => *n < 0,
            (Value::Number(n), Constraint::RangeClosed(range)) => range.contains(n),
            (Value::Number(n), Constraint::RangeHalfOpen(range)) => range.contains(n),
            // ModAction::Timeout
            (Value::ModAction(ModAction::Timeout(t)), Constraint::RangeClosed(range)) => {
                range.contains(&(*t as i64))
            }
            (Value::ModAction(ModAction::Timeout(t)), Constraint::RangeHalfOpen(range)) => {
                range.contains(&(*t as i64))
            }
            (_, _) => true,
        }
    }
}

macro_rules! impl_try_from_ownedvalue {
    ($($t:ident),+) => {
        $(impl TryFrom<Value> for $t {
            type Error = OwnedValueError;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value {
                  Value::$t(x) => Ok(x),
                    _ => Err(OwnedValueError {
                        expected: stringify!($t).into(),
                        value,
                    }),
                }
            }
        })+
    };
}

impl_try_from_ownedvalue!(String, ModAction);

impl TryFrom<Value> for i64 {
    type Error = OwnedValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(x) => Ok(x),
            _ => Err(OwnedValueError {
                expected: "Number".into(),
                value,
            }),
        }
    }
}

impl TryFrom<Value> for u64 {
    type Error = error::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(x) => Ok(x.try_into()?),
            _ => Err(OwnedValueError {
                expected: "Number".into(),
                value,
            }
            .into()),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = error::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(x) => Ok(x),
            _ => Err(OwnedValueError {
                expected: "Bool".into(),
                value,
            }
            .into()),
        }
    }
}

impl TryFrom<Value> for Regex {
    type Error = OwnedValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Regex(ref x) => Ok(Regex::new(x).unwrap()), //map_err(|_| ()),
            _ => Err(OwnedValueError {
                expected: "Regex".into(),
                value,
            }),
        }
    }
}

impl TryFrom<Value> for Platform {
    type Error = OwnedValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let err = |value| OwnedValueError {
            expected: "Platforms".into(),
            value,
        };

        match value {
            Value::Platforms(ref x) => Platform::from_bits(*x).ok_or_else(|| err(value)),
            _ => Err(err(value)),
        }
    }
}

impl TryFrom<Value> for Permissions {
    type Error = OwnedValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let err = |value| OwnedValueError {
            expected: "Permissions".into(),
            value,
        };

        match value {
            Value::Permissions(ref x) => Permissions::from_bits(*x).ok_or_else(|| err(value)),
            _ => Err(err(value)),
        }
    }
}

impl From<bool> for Value {
    fn from(x: bool) -> Self {
        Self::Bool(x)
    }
}

impl From<String> for Value {
    fn from(x: String) -> Self {
        Self::String(x)
    }
}

impl From<u64> for Value {
    fn from(x: u64) -> Self {
        Self::Number(x as i64)
    }
}

impl From<i64> for Value {
    fn from(x: i64) -> Self {
        Self::Number(x)
    }
}

impl From<isize> for Value {
    fn from(x: isize) -> Self {
        Self::Number(x as i64)
    }
}

impl From<Platform> for Value {
    fn from(x: Platform) -> Self {
        Self::Platforms(x.bits())
    }
}

impl From<Permissions> for Value {
    fn from(x: Permissions) -> Self {
        Self::Permissions(x.bits())
    }
}

impl From<Regex> for Value {
    fn from(x: Regex) -> Self {
        Self::Regex(x.as_str().to_owned())
    }
}

impl From<ModAction> for Value {
    fn from(x: ModAction) -> Self {
        Self::ModAction(x)
    }
}

impl<T: Into<Value>> From<Arc<T>> for Value {
    fn from(x: Arc<T>) -> Self {
        x.into()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Copy, Serialize, Deserialize)]
pub enum ModAction {
    None,
    Warn,
    Remove,
    Timeout(u32),
    Kick,
    Ban,
}

impl Display for ModAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModAction::None => write!(f, "None"),
            ModAction::Warn => write!(f, "Warn"),
            ModAction::Remove => write!(f, "Remove"),
            ModAction::Timeout(t) => write!(f, "Timeout ({}s)", t),
            ModAction::Kick => write!(f, "Kick"),
            ModAction::Ban => write!(f, "Ban"),
        }
    }
}

#[derive(Debug)]
pub enum RunRes {
    Ok,
    Noop, // for implicit cmds like Chat
    Filtered(ModAction),
    Autocorrect(String),
    Disabled,
    Ratelimited { global: bool },
    InsufficientPerms,
    InvalidArgs,
}

type KeySchema = (String, String, Value, Constraint); // (key, desc, default value (doubles as type, constraint)

/// (cmd, desc, keys)
type CmdSchema = (String, String, CmdType, Vec<KeySchema>);
pub type SchemaDump = Vec<CmdSchema>;
/// (cmd type, cmd name, (config key-value pairs))
pub type CmdDump = (String, String, Vec<(String, Value)>);

/// wrapper to impl Debug for DFA
pub(crate) struct DFAWrapper(DFA);

impl std::fmt::Debug for DFAWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DFA").finish()
    }
}

static DFA_BUILDER: Lazy<LevenshteinAutomatonBuilder> =
    Lazy::new(|| LevenshteinAutomatonBuilder::new(2, true));

trait Commandable {
    fn schema(platform: Platform) -> CmdSchema;
    fn args_schema(&self, _platform: Platform) -> Option<ArgDump> {
        None
    }
    fn dump(&self) -> CmdDump;
    fn new(name: impl Into<String>, kv: &mut [(String, Value)]) -> Option<Self>
    where
        Self: Sized;
}

trait CmdDesc {
    fn platform(&self) -> Platform;

    fn description(&self, _platform: Platform) -> Option<String> {
        None
    }
}

macro_rules! impl_cmddesc {
($($cmd:ty),+) => {
    $(
      impl CmdDesc for $cmd {
        #[inline]
        fn platform(&self) -> Platform {
          self.platforms
        }
      }
    )+
};
}

use crate::cmds::levenshtein::Levenshtein;
use filter::Filter;
use give::Give;
use hours::Hours;
use link::Link;
use log::Log;
use memebank::MemeBank;
use ping::Ping;
use points::Points;
use quote::Quote;
use reaction_role::ReactionRole;
use regex_filter::RegexFilter;
use russian_roulette::RussianRoulette;
use stream::Stream;
use streamlabs::Streamlabs;
use timer::Timer;
use transfer::Transfer;

impl_cmddesc![
    Filter,
    Give,
    Hours,
    Levenshtein,
    Link,
    Log,
    Points,
    Quote,
    RegexFilter,
    Timer,
    Transfer
];

/// prefix, desc, hidden (ephemeral), perms, arg
type ArgDump = (String, String, bool, Permissions, Vec<Arg>);
pub type ArgsDump = Vec<ArgDump>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arg {
    pub kind: ArgKind,
    pub optional: bool,
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArgKind {
    String,
    Integer {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    Bool,
    User,
    Platform,
    SubCommandGroup(Vec<Arg>), // Arg should only be of ArgKind::SubCommand
    SubCommand(Vec<Arg>),
    Autocomplete,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ArgValue {
    String(String),
    Integer(i64),
    Bool(bool),
    User(msg::User),
    Platform(msg::Platform),
    SubCommand(HashMap<String, ArgValue>),
}

impl From<User> for ArgValue {
    fn from(user: User) -> Self {
        Self::User(user)
    }
}

impl From<String> for ArgValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for ArgValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

trait Invokable {
    /// Arguments for invoking the command
    fn args(&self, _platform: Platform) -> Vec<Arg> {
        vec![]
    }

    /// Whether the response should be hidden or not
    fn hidden(&self, _platform: Platform) -> bool {
        false
    }
}

macro_rules! impl_invokable {
    ($($cmd:ty),+) => {
        $(
          impl Invokable for $cmd {}
        )+
    };
}

impl_invokable![
    Filter,
    Hours,
    Levenshtein,
    Log,
    Points,
    Quote,
    RegexFilter,
    Streamlabs,
    Timer
];

#[inline]
/// Removes first non-alphanum char (prefix assumed to be non-empty)
fn unbang_prefix(prefix: &str) -> &str {
    let has_bang = !prefix.chars().next().unwrap().is_alphanumeric();
    if has_bang {
        &prefix[1..] // strip bang if present
    } else {
        prefix
    }
}

fn check_invoke_prefix(prefix: &str, invoked_cmd: &str) -> Option<()> {
    if unbang_prefix(prefix) != invoked_cmd {
        return None;
    }
    Some(())
}

macro_rules! declare_cmds {
  ($($cmd:ident),* )  => {

    #[derive(Debug)]
    pub enum Command {
      $($cmd($cmd)),*,
    }

    pub(crate) fn schema(platform: Platform) -> SchemaDump {
      vec![$($cmd::schema(platform)),*]
    }

    impl Command {
      pub fn dump(&self) -> CmdDump {
        match self {
          $(Command::$cmd(c) => c.dump() ),*,
        }
      }

      pub fn name(&self) -> &str {
        match self {
          $(Command::$cmd(c) => &c.name ),*,
        }
      }

      pub fn new((cmd_type, name, mut values): CmdDump) -> Option<Self> {
        match cmd_type.as_str() {
          $(
            stringify!($cmd) => Some(Command::$cmd($cmd::new(name, &mut values).unwrap()))
          ),*,
          _ => None
        }
      }

      pub(crate) async fn chat(&self, ctx: &Context<'_>, chat: &msg::Chat) -> error::Result<RunRes> {
        match self {
          $(
            Self::$cmd(c) => c.chat(ctx, chat).await
          ),*
        }
      }

      pub(crate) async fn invoke(&self, ctx: &Context<'_>, invocation: &msg::Invocation) -> Option<RunRes> {
        match self {
          $(
            Self::$cmd(c) => c.invoke(ctx, invocation).await
          ),*
        }
      }

      pub(crate) fn args_schema(&self, platform: Platform) -> Option<ArgDump> {
        match self {
          $(
            Self::$cmd(c) => c.args_schema(platform)
          ),*
        }
      }
    }
  };
}

#[derive(Debug, Clone)]
pub struct CommandConfig {
    pub(crate) filters: Arc<Vec<Command>>,
    pub(crate) commands: Arc<Vec<Command>>,
    pub(crate) timers: Arc<Vec<Command>>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ConfigDump {
    pub(crate) filters: Vec<CmdDump>,
    pub(crate) commands: Vec<CmdDump>,
    pub(crate) timers: Vec<CmdDump>,
}

declare_cmds! {
  Points,
  Give,
  Filter,
  RegexFilter,
  Levenshtein,
  Streamlabs,
  Timer,
  Hours,
  Log,
  Link,
  Ping,
  Transfer,
  RussianRoulette,
  Quote,
  MemeBank,
  ReactionRole,
  Stream
}

#[derive(Debug)]
pub enum ConfigFile {
    Commands,
    Filters,
    Timers,
    Users,
}

pub fn config_path(cfg_type: ConfigFile) -> &'static str {
    match cfg_type {
        ConfigFile::Commands => "cmds.json",
        ConfigFile::Filters => "filters.json",
        ConfigFile::Timers => "timers.json",
        ConfigFile::Users => "users.json",
    }
}

#[tracing::instrument]
pub async fn load(cfg_type: ConfigFile) -> error::Result<Vec<Command>> {
    let contents =
        fs::read_to_string(Path::new(&*crate::CONFIG_DIR).join(config_path(cfg_type))).await?;

    // deserialise
    let inflated: Vec<CmdDump> = serde_json::from_str(&contents)?;

    let futures = inflated
        .into_iter()
        .map(|cmd_dump| tokio::task::spawn_blocking(|| Command::new(cmd_dump).unwrap()));
    let res = futures_util::future::join_all(futures).await;
    let res: Vec<Command> = res.into_iter().flat_map(|r| r.ok()).collect();

    Ok(res)
}

#[tracing::instrument]
async fn save(cmds: &[Command], cfg_type: ConfigFile) -> error::Result<()> {
    let dump: Vec<CmdDump> = cmds.iter().map(|c| c.dump()).collect();
    let dump = serde_json::to_string_pretty(&dump)?;
    fs::write(
        Path::new(&*crate::CONFIG_DIR).join(config_path(cfg_type)),
        dump,
    )
    .await
    .map_err(Error::Io)
}

pub async fn save_cmds(cmds: &[Command]) -> error::Result<()> {
    save(cmds, ConfigFile::Commands).await
}

pub async fn save_filters(cmds: &[Command]) -> error::Result<()> {
    save(cmds, ConfigFile::Filters).await
}

pub async fn save_timers(cmds: &[Command]) -> error::Result<()> {
    save(cmds, ConfigFile::Timers).await
}
