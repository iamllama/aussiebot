use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use crate::{
    cmds::{self, UnlockedCmdListInner},
    msg, DbPool, RedisPool,
};
use once_cell::sync::Lazy;
use regex::Regex;

// TODO: async trait on RedisPool? i.e self.redis.acquire_lock(key,time)
/// Acquire a distributed lock `key`
pub async fn acquire_lock(redis: RedisPool, key: impl AsRef<str>, time: u64) -> Option<bool> {
    let mut redis = redis.get().await.ok()?;
    let locked = redis::cmd("SET")
        .arg(&[key.as_ref(), "1", "NX", "EX", &time.to_string()])
        .query_async::<redis::aio::Connection, bool>(&mut redis)
        .await
        .ok()?;
    println!("acquired lock: {} ({})", locked, key.as_ref());
    Some(locked)
}

/// Release a distributed lock `key`
pub async fn release_lock(redis: RedisPool, key: impl AsRef<str>) -> Option<bool> {
    let mut redis = redis.get().await.ok()?;
    let unlocked = redis::cmd("DEL")
        .arg(key.as_ref())
        .query_async::<redis::aio::Connection, bool>(&mut redis)
        .await
        .ok()?;
    println!("released lock: {} ({})", unlocked, key.as_ref());
    Some(unlocked)
}

// TODO: async trait on RedisPool? i.e self.redis.acquire_lock(key,time)
/// Acquire a distributed lock `key`
pub async fn set_field(
    redis: RedisPool,
    key: impl AsRef<str>,
    field: impl AsRef<str>,
    value: impl AsRef<str>,
    exclusive: bool,
) -> Option<bool> {
    let mut redis = redis.get().await.ok()?;
    let set = redis::cmd(if exclusive { "HSETNX" } else { "HSET" })
        .arg(&[key.as_ref(), field.as_ref(), value.as_ref()])
        .query_async::<redis::aio::Connection, bool>(&mut redis)
        .await
        .ok()?;
    println!(
        "set {}[{}] = {}: {}",
        key.as_ref(),
        field.as_ref(),
        value.as_ref(),
        set
    );
    Some(set)
}

/// Check if prefix matches
pub fn prefix_matches<T: Default>(name: &cmds::Value, chat: &msg::Chat) -> Option<T> {
    if let cmds::Value::String(prefix) = name {
        if chat.msg.starts_with(prefix.as_ref()) {
            return Some(T::default());
        }
    }
    None
}

static ONE_NUM_ARG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\S+)\s(\d+|all)\s*").unwrap());

/// Check if prefix matches
/// -1 means all
pub fn one_arg_matches(name: &cmds::Value, chat: &msg::Chat) -> Option<i32> {
    let captures = ONE_NUM_ARG_REGEX.captures(&chat.msg)?;
    //println!("{:?}", captures);
    // check command prefix
    match name {
        cmds::Value::String(pat) if pat.as_str() == &captures[1] => {}
        _ => return None,
    }
    // parse int arg
    if &captures[2] == "all" {
        return Some(-1);
    }

    captures[2].parse::<i32>().ok()
}

/// custom serde for Value::Regex
pub mod serde_regex {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(regex: &regex::Regex, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(regex.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<regex::Regex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        regex::Regex::new(&s).map_err(serde::de::Error::custom)
    }
}

/// custom serde for Value::String
pub mod serde_arc_string {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(s: &std::sync::Arc<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(s.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<std::sync::Arc<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(std::sync::Arc::new(s))
    }
}

pub fn load_config(
    redis: RedisPool,
    db: DbPool,
    is_main_instance: bool,
) -> Option<UnlockedCmdListInner> {
    println!("\x1b[93mLoading config...\x1b[0m");
    let config_dir = dotenv::var("CONFIG_DIR").ok()?;
    let config_dir = Path::new(&config_dir);
    // open cmds file
    let cmds_file = File::open(Path::join(config_dir, "cmds.json")).ok()?;
    // deserialise json
    let de_list = serde_json::from_reader::<_, Vec<cmds::OwnedDumpedCmd>>(cmds_file).ok()?;
    let commands = cmds::Set::deserialise_commands(de_list, redis.clone(), db.clone());
    // open filters file
    let filters_file = File::open(Path::join(config_dir, "filters.json")).ok()?;
    // deserialise json
    let de_list = serde_json::from_reader::<_, Vec<cmds::OwnedDumpedCmd>>(filters_file).ok()?;
    let filters = cmds::Set::deserialise_commands(de_list, redis.clone(), db.clone());

    // open filters file
    let timers_file = File::open(Path::join(config_dir, "timers.json")).ok()?;
    // deserialise json
    let de_list = serde_json::from_reader::<_, Vec<cmds::OwnedDumpedCmd>>(timers_file).ok()?;
    let timers = cmds::Set::deserialise_commands(de_list, redis, db);

    println!("\x1b[92mConfig loaded\x1b[0m");

    Some(UnlockedCmdListInner {
        commands,
        filters,
        timers,
        is_main_instance,
        timer_handles: vec![],
    })
}

pub fn open_config_files() -> Option<[File; 3]> {
    let config_dir = dotenv::var("CONFIG_DIR").ok()?;
    let config_dir = Path::new(&config_dir);
    // open cmds file
    let cmds_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(Path::join(config_dir, "cmds.json"))
        .ok()?;
    let filters_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(Path::join(config_dir, "filters.json"))
        .ok()?;
    let timers_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(Path::join(config_dir, "timers.json"))
        .ok()?;

    Some([cmds_file, filters_file, timers_file])
}
