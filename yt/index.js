const instanceId = process.env.ID ?? "aussieyt";
const liveId = process.env.VID ?? "";
const startTime = Date.now();

const { redisOpt, upstreamChannel, downstreamChannel, broadcastId, botType, msgType, pubMsgIsValid, heistResp } = require("../util");
const { postChat, ytInit, getLiveChatId } = require("./chat");
const { LiveChat } = require("youtube-chat");
const Redis = require("ioredis");

const pub = new Redis(redisOpt);
const sub = new Redis(redisOpt);
sub.subscribe(downstreamChannel);

const send = (...args) => pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, ...args]));
const wait = require('node:timers/promises').setTimeout;

const STREAMLABS_ID = "UCNL8jaJ9hId96P13QmQXNtA";

const processChat = (chat) => {
  const msg = chat?.message[0]?.text;
  const id = chat?.author?.channelId;
  const name = chat?.author?.name;
  const time = Date.parse(chat.timestamp ?? Date.now());
  if (time < startTime) return;
  if (id !== STREAMLABS_ID) {
    const source = [id, name];
    const cmd = /^!(\w+)/.exec(msg);
    if (cmd !== null) {
      // !-command
      processCommand(cmd[1], source, msg);
    } else {
      // normal chat msg
      if (chat.superchat) {
        send(msgType.CHAT, source, true);
        ytChatQueue.push(`@${name} ratJAM widepeepoHappy`);
      } else send(msgType.CHAT, source);
    }
  } else {
    // Streamlabs's bot
    const re = /^@(.+), you have (\d+) Points?/.exec(msg);
    if (re !== null) {
      const target = re[1], amount = re[2];
      send(msgType.SCRAPE_POINTS, target, amount);
    }
  }
}

const processCommand = async (cmd, source, msg) => {
  if (await rateLimited(source[0])) {
    console.log(source, "rate-limited");
    return;
  }
  switch (cmd) {
    case "points":
      send(msgType.POINTS, source);
      break;
    case "gamble": {
      const res = /^!gamble\s(\d+|all)\s*$/i.exec(msg);
      if (!res) break;
      try {
        let amount = 0;
        if (res[1] === "all") {
          amount = -1;
        } else {
          amount = parseInt(res[1]);
        }
        send(msgType.GAMBLE, source, amount);
      } catch (e) { console.error(e); }
      break;
    }
    case "heist": {
      const res = /^!heist\s(\d+|all)\s*$/i.exec(msg);
      if (!res) break;
      try {
        let amount = 0;
        if (res[1] === "all") {
          amount = -1;
        } else {
          amount = parseInt(res[1]);
        }
        send(msgType.HEIST, source, amount);
      } catch (e) { console.error(e); }
      break;
    }
    case "give": {
      const res = /^!give\s@?(.+)\s(\d+)$/i.exec(msg);
      if (!res) break;
      try {
        const target = res[1]
        const amount = parseInt(res[2]);
        send(msgType.GIVE, source, target, amount);
      } catch (e) { console.error(e); }
      break;
    }
    default:
      const res = /^!(\w+)\s*$/.exec(msg);
      if (!res) break;
      const cmd = res[1];
      send(msgType.COMMAND, source, cmd);
      break;
  }
};

sub.on("message", (chn, msg) => {
  switch (chn) {
    case downstreamChannel:
      processMsg(msg);
      break;
    default:
      break;
  }
});

const ytChatQueue = [];

const processMsg = (origMsg) => {
  let msg = null;
  try {
    msg = JSON.parse(origMsg);
  } catch (e) {
    console.log(e);
    return;
  }

  if (!pubMsgIsValid(msg) || (msg[0] !== instanceId && msg[0] !== broadcastId)) return;

  switch (msg[1]) {
    case msgType.POINTS: {
      const [bot_type, src_name] = msg[2], points = msg[3];
      const text = `${src_name}, you have ${points} point(s)`;
      ytChatQueue.push(text);
      console.log(text);
      break;
    }
    case msgType.GIVE: {
      const [_, src_name] = msg[2], target_name = msg[3], amount = msg[4];
      const text = `${src_name} gave ${target_name} ${amount} point(s)`;
      ytChatQueue.push(text);
      console.log(text);
      break;
    }
    case msgType.GAMBLE: {
      const [_, src_name] = msg[2], roll = msg[3], delta = msg[4], points = msg[5];
      const text = `${src_name} rolled ${roll}, ${delta > 0 ? "won" : "lost"} ${delta > 0 ? delta : -delta} point(s), and now has ${points} point(s)`;
      ytChatQueue.push(text);
      console.log(text);
      break;
    }
    case msgType.COMMAND: {
      const [_, src_name] = msg[2], text = msg[3];
      const res = `@${src_name} ` + text;
      const parts = res.match(/.{1,180}(\s|$)/g); //split along word boundary, unless at end
      for (const part of parts) {
        //rl.prompt();
        const text = part.trim()
        ytChatQueue.push(text);
        console.log(text);
      }
      break;
    }
    case msgType.HEIST: {
      switch (msg[2]) { //heistResp
        case heistResp.STARTED: {
          const name = msg[5] ?? msg[3][1], amount = msg[4];
          const text = `Ahoy! ${name} started a heist with ${amount} points 🏴‍☠️`;
          ytChatQueue.push(text);
          console.log(text);
          break;
        }
        case heistResp.JOINED: {
          const name = msg[5] ?? msg[3][1], amount = msg[4];
          const text = `Ahoy! ${name} added ${amount} points to the booty 🏴‍☠️`;
          ytChatQueue.push(text);
          console.log(text);
          break;
        }
        case heistResp.ENDED:
          const winner_list = msg[3];
          let text = "blah blah poisonous fog walk the plank davy jones' locker 🏴‍☠️ heist results: ";
          if (winner_list && winner_list.length) {
            const [name, amount] = winner_list.pop();
            text += `${name} (${amount})`;
            if (winner_list.length) text = winner_list.reduce((acc, [name, amount]) => acc + `, ${name} (${amount})`, text);
          } else text += "no one survived ☠";
          ytChatQueue.push(text);
          console.log(text);
          break;
        default:
          break;
      }
      break;
    }
    case msgType.ERROR: {
      console.log(msg[3]);
      break;
    }
    default:
      break;
  }

}

const rateliimitDelay = 10; // seconds

const rateLimited = async (id) => {
  const key = `${instanceId}$rate$${id}`;
  const check = await pub.get(key);
  if (check === null) {
    await pub.set(key, "", "ex", rateliimitDelay);
    return false;
  } else {
    // rate-limited
    return true;
  }
}

const YT_CHAT_INTERVAL = 2000; //interval between posting yt msgs, in ms

(async () => {

  const liveChat = new LiveChat({ liveId })
    .on("start", (liveId) => {
      console.log("chat started");
    })
    .on("end", (reason) => {
      console.log("chat stopped");
    })
    .on("chat", processChat)
    .on("error", console.error);

  const ok = await liveChat.start();

  if (!ok) {
    console.error("failed to start");
    return;
  }

  const youtube = await ytInit();
  const liveChatId = await getLiveChatId(youtube, liveId);

  console.log("liveChatId", liveChatId);

  const processChatQueue = async () => {
    if (!ytChatQueue.length) return;
    const msg = ytChatQueue.shift();
    if (msg) return postChat(youtube, liveChatId, `🤖 ${msg}`);
  };

  // clear queue before starting
  while (ytChatQueue.length) ytChatQueue.pop();
  ytChatQueue.push("aussiebot online");

  while (true) {
    await processChatQueue();
    await wait(YT_CHAT_INTERVAL);
  }

  //setInterval(processChatQueue, ytChatInterval);

})();