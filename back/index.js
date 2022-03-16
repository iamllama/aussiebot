const db = require('./db')
const Redis = require("ioredis");
const { redisOpt, upstreamChannel, downstreamChannel, botType, msgType, pubMsgIsValid } = require("../util");
const pub = new Redis(redisOpt);
const sub = new Redis(redisOpt);
sub.subscribe(upstreamChannel);

sub.on("message", (chn, msg) => {
  switch (chn) {
    case upstreamChannel:
      processMsg(msg);
      break;
    default:
      break;
  }
});

const processMsg = async (origMsg) => {
  let msg = null;
  try {
    msg = JSON.parse(origMsg);
  } catch (e) {
    debug(e);
    return;
  }

  if (!pubMsgIsValid(msg)) return;

  console.log(msg);

  const instanceId = msg[0], bot_type = msg[1], msg_type = msg[2], src_id = msg[3];

  switch (msg_type) {
    case msgType.STARTED:
      console.log(instanceId, "started up");
      break;
    case msgType.POINTS:
      const key = getKey(bot_type, src_id);
      const res = await getUser(db, key, ["points"]);
      if (res.length > 1) {
        console.error("non unique id", key, res);
        informErr(instanceId, src_id, "Non-unique id");
        break;
      } else if (res.length == 0) {
        await addUser(db, bot_type, src_id);
        //informErr(instanceId, src_id, "You don't have enough points");
        //  re-emit the event in case the starting amount isn't 0
        pub.publish(upstreamChannel, origMsg);
        break;
      }
      const user = res[0];
      pub.publish(downstreamChannel, JSON.stringify([instanceId, msgType.POINTS, src_id, user.points]));
      break;
    case msgType.GIVE:
      const target_name = msg[4], amount = msg[5];
      const src_key = getKey(bot_type, src_id);
      const target_key = getKey(bot_type, target_name);
      const [srcres, targetres] = await Promise.all([getUser(db, src_key), getUser(db, target_key)]);
      if (srcres.length != 1 || targetres.length != 1) {
        console.error("GIVE: one or both missing", src_id, target_name);
        if (srcres.length == 0) {
          // add src user
          await addUser(db, bot_type, src_id);
        }
        informErr(instanceId, src_id, "Missing user(s)");
        break;
      }
      const src = srcres[0], target = targetres[0];
      console.log(src.points, target.points);
      if (src.id === target.id ||
        (src.ytid && src.ytid === target.ytid) ||
        (src.discordid && src.discordid === target.discordid) ||
        (src.twitchid && src.twitchid === target.twitchid)) {
        console.error("GIVE: same user", src_id, target_name);
        informErr(instanceId, src_id, "You can't give yourself points");
        break;
      } else if (src.points < amount) {
        console.error("GIVE: insuff poinrs", src_id, target_name, amount);
        informErr(instanceId, src_id, "You don't have enough points");
        break;
      }
      // start update transaction
      const cl = await db.getClient();
      try {
        await cl.query("BEGIN");
        await Promise.all([setUser(cl, src_key, { points: src.points - amount }), setUser(cl, target_key, { points: target.points + amount })]);
        await cl.query("COMMIT");
        pub.publish(downstreamChannel, JSON.stringify([instanceId, msgType.GIVE, src_id, target_name, amount]));
      } catch (e) {
        await cl.query("ROLLBACK");
        informErr(instanceId, src_id, "Failed to update points");
        console.error(e);
      }
      break;
    case msgType.COMMAND: {
      const name = msg[4];
      const res = await getCommand(db, name);
      if (res.length) {
        const text = res[0].msg;
        pub.publish(downstreamChannel, JSON.stringify([instanceId, msgType.COMMAND, src_id, text]));
      }
    }
      break;
    default:
      break;
  }

}

const informErr = (instanceId, id, msg) => pub.publish(downstreamChannel, JSON.stringify([instanceId, msgType.ERROR, id, msg]));

const getKey = (type, id) => {
  const key = {};
  switch (type) {
    case botType.YOUTUBE:
      if (typeof id !== "string") {
        key.ytid = id[0];
      } else {
        key.ytname = id;
      }
      break;
    case botType.DISCORD:
      key.discordid = id;
      break;
    case botType.TWITCH:
      key.twitchid = id;
      break;
  }
  console.log("getKey", key);
  return key;
}

const getUser = (db, key, col) => {
  const query = [];
  const val = [];
  for (const col in key) {
    val.push(key[col]);
    query.push(`${col} = $${val.length}`);
  }
  const ret = (!col || !col.length) ? "*" : col.join(", ");
  const q = `SELECT ${ret} FROM users WHERE ${query.join(", ")}`;
  console.log("getUser", q, val);
  return db.query(q, val).then(res => res.rows);
}

const addUser = (db, type, id, col) => {
  const query = ["INSERT INTO users"];
  const val = [];
  switch (type) {
    case botType.YOUTUBE:
      query.push("(ytid, ytname) VALUES ($1, $2)");
      val.push(...id);
      break;
    case botType.DISCORD:
      query.push("(discordid) VALUES ($1)");
      val.push(id);
      break;
    case botType.TWITCH:
      query.push("(twitchid) VALUES ($1)");
      val.push(id);
      break;
  }
  query.push((!col || !col.length) ? "" : `RETURNING ${col.join(", ")}`);
  const q = query.join(" ");
  console.log("addUser", q, val);
  return db.query(q, val);
}

const setUser = (db, key, rec, col) => {
  const keys = [];
  const query = [];
  const val = [];
  for (const col in key) {
    val.push(key[col]);
    keys.push(`${col} = $${val.length}`);
  }
  for (const col in rec) {
    val.push(rec[col]);
    query.push(`${col} = $${val.length}`);
  }
  const ret = (!col || !col.length) ? "" : `RETURNING ${col.join(", ")}`;
  const q = `UPDATE users SET ${query.join(", ")} WHERE ${keys.join(" AND ")} ${ret}`;
  console.log("setUser", q, val);
  return db.query(q, val);
}

const getCommand = (db, name) => {
  const q = `SELECT msg FROM commands WHERE name = $1`;
  console.log("getCommand", q, name);
  return db.query(q, [name]).then(res => res.rows);
}