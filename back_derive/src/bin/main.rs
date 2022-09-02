use bitflags::bitflags;
use std::sync::Arc;

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum CmdType {
    Command,
    Filter,
    Timer,
}

#[allow(dead_code)]
type CmdSchema = (
    String,
    String,
    CmdType,
    Vec<(String, String, Value, Constraint)>,
);

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

pub type CmdDump = (String, String, Vec<(String, Value)>);
#[allow(dead_code)]
struct Arg {}
#[allow(dead_code)]
type ArgDump = (String, String, bool, Permissions, Vec<Arg>);

trait Command {
    fn chat() {}
    fn invoke() {}
    fn schema(platform: Platform) -> CmdSchema;
    fn dump(&self) -> CmdDump;
    fn new(name: impl Into<String>, kv: &mut [(String, Value)]) -> Option<Self>
    where
        Self: Sized;
}

trait Invokable {
    fn args_schema(&self, _platform: Platform) -> Option<ArgDump> {
        None
    }
    fn hidden(&self, _platform: Platform) -> bool {
        false
    }
}

trait CmdDesc {
    fn platform(&self) -> Platform;
    fn description(&self, _platform: Platform) -> Option<String> {
        None
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum Constraint {
    None,
    NonEmpty,
    Positive,
    Negative,
    RangeClosed(std::ops::RangeInclusive<i64>),
    RangeHalfOpen(std::ops::Range<i64>),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Value {
    None,
    Bool(bool),
    String(String),
    Number(i64),
    Platform(u32),
    Permissions(u32),
}

impl Default for Value {
    fn default() -> Self {
        Self::None
    }
}

impl Value {
    #[allow(dead_code)]
    fn verify(&self, constraint: Constraint) -> bool {
        println!("verify {:?}, constr: {:?}", self, constraint);
        if matches!(constraint, Constraint::None) {
            return true;
        }
        match self {
            Value::None => false,
            Value::Bool(_x) => unimplemented!(),
            Value::String(x) => match constraint {
                Constraint::NonEmpty => !x.is_empty(),
                _ => unimplemented!(),
            },
            Value::Number(x) => match constraint {
                Constraint::Positive => *x > 0,
                Constraint::Negative => *x < 0,
                Constraint::RangeClosed(range) => range.contains(x),
                Constraint::RangeHalfOpen(range) => range.contains(x),
                _ => unimplemented!(),
            },
            Value::Platform(_x) => unimplemented!(),
            Value::Permissions(_x) => unimplemented!(),
        }
    }
}

trait FromValue {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized;
}

impl<T: FromValue> FromValue for Arc<T> {
    fn from_value(value: Value) -> Option<Self> {
        T::from_value(value).map(Arc::new)
    }
}

impl FromValue for bool {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized,
    {
        match value {
            Value::Bool(x) => Some(x),
            _ => None,
        }
    }
}

impl FromValue for String {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized,
    {
        match value {
            Value::String(x) => Some(x),
            _ => None,
        }
    }
}

impl FromValue for u64 {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized,
    {
        match value {
            Value::Number(x) => Some(x.try_into().ok()?),
            _ => None,
        }
    }
}

impl FromValue for isize {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized,
    {
        match value {
            Value::Number(x) => Some(x.try_into().ok()?),
            _ => None,
        }
    }
}

impl FromValue for Platform {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized,
    {
        match value {
            Value::Platform(x) => Some(Platform::from_bits(x)?),
            _ => None,
        }
    }
}

impl FromValue for Permissions {
    fn from_value(value: Value) -> Option<Self>
    where
        Self: Sized,
    {
        match value {
            Value::Permissions(x) => Some(Permissions::from_bits(x)?),
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(x: bool) -> Self {
        Value::Bool(x)
    }
}

impl From<String> for Value {
    fn from(x: String) -> Self {
        Value::String(x)
    }
}

impl From<u64> for Value {
    fn from(x: u64) -> Self {
        Value::Number(x as i64)
    }
}

impl From<isize> for Value {
    fn from(x: isize) -> Self {
        Value::Number(x as i64)
    }
}

impl From<Platform> for Value {
    fn from(x: Platform) -> Self {
        Value::Platform(x.bits())
    }
}

impl From<Permissions> for Value {
    fn from(x: Permissions) -> Self {
        Value::Permissions(x.bits())
    }
}

impl<T: Into<Value>> From<Arc<T>> for Value {
    fn from(x: Arc<T>) -> Self {
        x.into()
    }
}

// #[command(timer, locks(lock, prev_msg, count))]
// #[derive(Debug, Invokable)]
// /// Give and receive points
// struct Give {
//     /// Command prefix
//     #[cmd(def("!give"), constr(non_empty))]
//     prefix: String,
//     /// Autocorrect prefix
//     autocorrect: bool,
//     /// Platforms
//     #[cmd(defl("Platform::CHAT"))]
//     platforms: Platform,
//     /// Permissions
//     #[cmd(defl("Permissions::NONE"))]
//     perms: Permissions,
//     /// Cooldown in seconds
//     #[cmd(def(120u64), constr(pos))]
//     cooldown: u64,
//     /// Min amount
//     #[cmd(def(10_u64), constr(pos))]
//     min_amount: u64,
//     /// Max amount
//     #[cmd(def(10_000_u64), constr(pos))]
//     max_amount: u64,
// }

fn main() {
    // let mut kv = vec![
    //     ("enabled".into(), true.into()),
    //     (
    //         "platforms".into(),
    //         (Platform::YOUTUBE | Platform::DISCORD).into(),
    //     ),
    // ];
    // let give = Give::new("yt", &mut kv).unwrap();
    // println!("{:?}", give);

    // let schema = Give::schema(Platform::YOUTUBE);
    // println!("schema: {:#?}", schema);

    // let dump = give.dump();
    // println!("dump: {:#?}", dump);
}
