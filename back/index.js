const random = require('random');
const Redis = require("ioredis");
const db = require('./db')
const fetch = (...args) => import('node-fetch').then(({ default: fetch }) => fetch(...args));
const { redisOpt, upstreamChannel, downstreamChannel, broadcastId, botType, msgType, pubMsgIsValid, HEIST_KEY, heistResp } = require("../util");

const rollDice = random.uniformInt(1, 100);

const pub = new Redis(redisOpt);
const sub = new Redis(redisOpt);
sub.subscribe(upstreamChannel);

const send = (...args) => pub.publish(downstreamChannel, JSON.stringify(args));
const informErr = (instanceId, id, msg) => send(instanceId, msgType.ERROR, id, msg);

const processMsg = async (origMsg) => {
  let msg = null;
  try {
    msg = JSON.parse(origMsg);
  } catch (e) {
    console.log(e);
    return;
  }

  if (!pubMsgIsValid(msg)) return;

  console.log(msg);

  const instanceId = msg[0], bot_type = msg[1], msg_type = msg[2];

  // msg types that don't need src user resolved
  switch (msg_type) {
    case msgType.STARTED:
      console.log(instanceId, "started up");
      return;
    case msgType.STOPPED:
      console.log(instanceId, "stopped");
      return;
    case msgType.SCRAPE_POINTS: {
      const target_name = msg[3], amount = msg[4];
      const target_key = getKey(bot_type, target_name);
      console.log("SCRAPE_POINTS", target_key, amount);
      await setUser(db, target_key, { points: amount });
      return;
    }
    default:
      break;
  }

  const src_id = msg[3];

  // TODO: refactor out
  // add user if not alr in db
  const key = getKey(bot_type, src_id);
  let res = await getUser(db, key);
  if (!res.length) {
    res = await addUser(db, bot_type, src_id);
  } else if (res.length > 1) {
    console.error("non unique id", key, res);
    informErr(instanceId, src_id, "Non-unique id");
    return;
  }
  const src = res[0];

  switch (msg_type) {
    case msgType.POINTS:
      send(instanceId, msgType.POINTS, src_id, src.points);
      break;
    case msgType.GIVE:
      const target_name = msg[4], amount = msg[5];
      const target_key = getKey(bot_type, target_name);
      let target_res = await getUser(db, target_key);
      if (!target_res.length) {
        if (bot_type == botType.YOUTUBE) {
          // can't add target if yt, missing ytid
          informErr(instanceId, src_id, "Missing user(s)");
        } else {
          // add target to db
          target_res = await addUser(db, bot_type, target_name); //target_name: discordid | twitchid
        }
      }
      const target = target_res[0];
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
        await Promise.all([setUser(cl, key, { points: src.points - amount }), setUser(cl, target_key, { points: target.points + amount })]);
        await cl.query("COMMIT");
        send(instanceId, msgType.GIVE, src_id, target_name, amount);
      } catch (e) {
        await cl.query("ROLLBACK");
        informErr(instanceId, src_id, "Failed to update points");
        console.error(e);
      }
      break;
    case msgType.GAMBLE: {
      const amount = msg[4] == -1 ? Math.min(10000, src.points) : msg[4]; // -1 means all
      if (10000 < amount || amount < 10) {
        informErr(instanceId, src_id, "Wager must be between 10 and 10000");
        break;
      } else if (src.points < amount) {
        informErr(instanceId, src_id, "Not enough points");
        break;
      }
      const roll = rollDice();
      let delta = 0;
      if (roll <= 50) {
        delta = -amount;
      } else if (roll >= 99) {
        delta = 2 * amount;
      } else {
        delta = amount;
      }
      const res = await setUser(db, key, { points: src.points + delta }, ["points"]);
      const src1 = res[0];
      send(instanceId, msgType.GAMBLE, src_id, roll, delta, src1.points);
      break;
    }
    case msgType.COMMAND: {
      const name = msg[4];
      if (name === "french") {
        const res = await getBlague(/*random.boolean()*/false);
        const { error, joke, vdm } = await res.json();
        if (!error) {
          const text = joke ? `${joke.question} ${joke.answer}` : vdm.content;
          send(instanceId, msgType.COMMAND, src_id, text);
        }
        break;
      }
      const res = await getCommand(db, name);
      if (res.length) {
        const text = res[0].msg;
        send(instanceId, msgType.COMMAND, src_id, text);
      }
      break;
    }
    case msgType.CHAT: {
      const delta = msg[4] ? 1000 : 2; //superchat or not
      const points = src.points + delta;
      console.log("CHAT", key, points, msg[4]);
      await setUser(db, key, { points });
      break;
    }
    case msgType.HEIST: {
      const amount = msg[4] == -1 ? Math.min(10000, src.points) : msg[4]; // -1 means all
      const name = msg[5] ?? src.ytname;
      console.log("HEIST", name, amount);
      if (10000 < amount || amount < 10) {
        informErr(instanceId, src_id, "Wager must be between 10 and 10000");
        break;
      } else if (src.points < amount) {
        informErr(instanceId, src_id, "Not enough points");
        break;
      }
      const inProgress = await pub.exists(HEIST_KEY);
      const set_value = JSON.stringify([name, amount * 2]);
      if (inProgress) {
        const notAlrJoined = await pub.hsetnx(HEIST_KEY, src.id, set_value);
        if (notAlrJoined) {
          await setUser(db, key, { points: src.points - amount });
          send(broadcastId, msgType.HEIST, heistResp.JOINED, src_id, amount, name);
        } else {
          informErr(instanceId, src_id, "Already joined heist");
        }
      } else {
        await pub.hset(HEIST_KEY, src.id, set_value);
        await setUser(db, key, { points: src.points - amount });
        send(broadcastId, msgType.HEIST, heistResp.STARTED, src_id, amount, name);
        setTimeout(() => processHeist(db), 100000);
      }
    }
    default:
      break;
  }

}

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
  query.push((!col || !col.length) ? "RETURNING *" : `RETURNING ${col.join(", ")}`);
  const q = query.join(" ");
  console.log("addUser", q, val);
  return db.query(q, val).then(res => res.rows);
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
  return db.query(q, val).then(res => res.rows);
}

const getCommand = (db, name) => {
  const q = `SELECT msg FROM commands WHERE name = $1`;
  console.log("getCommand", q, name);
  return db.query(q, [name]).then(res => res.rows);
}

const getBlague = (vdm) => fetch(`https://blague.xyz/api/${vdm ? "vdm" : "joke"}/random`, { method: "Get" });

const processHeist = async (db) => {
  const pirates = await pub.hgetall(HEIST_KEY);
  console.log(pirates);
  const cl = await db.getClient();
  try {
    const winners = Object.keys(pirates).filter(_ => rollDice() > 50);
    const results = await Promise.all(winners.map(id => getUser(cl, { id })));
    await cl.query("BEGIN");
    const winner_list = [];
    const users = results
      .filter(res => res && res.length == 1)
      .map(res => {
        const user = res[0];
        const id = user.id;
        const name_amount = JSON.parse(pirates[id]);
        winner_list.push(name_amount);
        return setUser(cl, { id }, { points: user.points + name_amount[1] });
      });
    await Promise.all(users);
    await cl.query("COMMIT");
    await pub.del(HEIST_KEY);
    send(broadcastId, msgType.HEIST, heistResp.ENDED, winner_list);
  } catch (e) {
    await cl.query("ROLLBACK");
    //informErr(instanceId, src_id, "Failed to update points");
    console.error(e);
  }
}

(async () => {
  await pub.del(HEIST_KEY);
  sub.on("message", (chn, msg) => {
    switch (chn) {
      case upstreamChannel:
        processMsg(msg);
        break;
      default:
        break;
    }
  });
})();

