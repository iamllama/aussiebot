const instanceId = process.env.ID ?? "aussieyt";
const liveId = process.env.VID ?? "";

const { postChat, ytInit, getLiveChatId } = require("./chat");
const { LiveChat } = require("youtube-chat");
const Redis = require("ioredis");
const { redisOpt, upstreamChannel, downstreamChannel, botType, msgType, pubMsgIsValid } = require("../util");
const pub = new Redis(redisOpt);
const sub = new Redis(redisOpt);
sub.subscribe(downstreamChannel);

// Or specify LiveID in Stream manually.
const liveChat = new LiveChat({ liveId })

const processChat = (chat) => {
  const msg = chat?.message[0]?.text;
  const cmd = /^!(\w+)/.exec(msg);
  const id = chat?.author?.channelId;
  const name = chat?.author?.name;
  const source = [id, name];
  if (cmd !== null) {
    // !-command
    console.log(cmd);
    const cmdName = cmd[1];
    const msg = chat?.message[0]?.text;
    processCommand(cmdName, source, msg);
  } else {
    // normal chat msg
    pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.CHAT, source]));
  }
  // Streamlabs's bot id
  if (id == "UCNL8jaJ9hId96P13QmQXNtA") {
    const re = /^@(.+), you have (\d+) Points?/.exec(msg);
    if (re !== null) {
      const target = re[1], amount = re[2];
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.SCRAPE_POINTS, target, amount]));
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
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.POINTS, source]));
      break;
    case "gamble": {
      const res = /^!gamble\s(\d+|all)\s*$/.exec(msg)
      if (!res) break;
      try {
        let amount = 0;
        if (res[1] === "all") {
          amount = 10000;
        } else {
          amount = parseInt(res[1]);
        }
        pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.GAMBLE, source, amount]));
      } catch (e) { console.error(e); }
      break;
    }
    case "heist": {
      const res = /^!heist\s(\d+)\s*$/.exec(msg);
      if (!res) break;
      try {
        const amount = parseInt(res[1]);
        pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.HEIST, source, amount]));
      } catch (e) { console.error(e); }
      break;
    }
    case "give": {
      const res = /^!give\s@?(.+)\s(\d+)$/.exec(msg);
      if (!res) break;
      try {
        const target = res[1]
        const amount = parseInt(res[2]);
        pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.GIVE, source, target, amount]));
      } catch (e) { console.error(e); }
      break;
    }
    default:
      const res = /^!(\w+)\s*$/.exec(msg);
      if (!res) break;
      const cmd = res[1];
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.COMMAND, source, cmd]));
      break;
  }
};


// Emit at start of observation chat.
// liveId: string
liveChat.on("start", (liveId) => {
  console.log("chat started");
  pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.STARTED, liveId]));
})

// Emit at end of observation chat.
// reason: string?
liveChat.on("end", (reason) => {
  console.log("chat stopped");
  pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.STOPPED, reason]));
})

// Emit at receive chat.
// chat: ChatItem
liveChat.on("chat", processChat);

// Emit when an error occurs
// err: Error or any
liveChat.on("error", console.error)

/*
const readline = require('readline'), rl = readline.createInterface(process.stdin, process.stdout);
const source = ["UCtBkiI649CihbY3MyA-91kA3", "a_llama3"];

rl.setPrompt('chat> ');
rl.prompt();

rl.on('line', function (line) {
  postChat(yt, "KicKGFVDdnlRYWkwYnFJZmZHWV91WVhxMmRNZxILaVJ0c0taY2dTVTA", line);
  const cmd = /^!(\w+)/.exec(line);
  if (cmd != null) {
    processCommand(cmd[1], source, line);
  } else {
    pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.YOUTUBE, msgType.CHAT, source]));
  }
  rl.prompt();
}).on('close', function () {
  console.log('Have a great day!');
  process.exit(0);
});*/

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
    debug(e);
    return;
  }

  if (!pubMsgIsValid(msg) || msg[0] != instanceId) return;

  switch (msg[1]) {
    case msgType.POINTS: {
      const [bot_type, src_name] = msg[2], points = msg[3];
      const text = `@${src_name} you have ${points} point(s)`;
      ytChatQueue.push(text);
      console.log(text);
      break;
    }
    case msgType.GIVE: {
      const [_, src_name] = msg[2], target_name = msg[3], amount = msg[4];
      const text = `@${src_name} gave @${target_name} ${amount} point(s)`;
      ytChatQueue.push(text);
      console.log(text);
      break;
    }
    case msgType.GAMBLE: {
      const [_, src_name] = msg[2], roll = msg[3], delta = msg[4], points = msg[5];
      const text = `@${src_name} rolled ${roll}, ${delta > 0 ? "won" : "lost"} ${delta > 0 ? delta : -delta} point(s), and now has ${points} point(s)`;
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

const ytChatInterval = 2000; //interval between posting yt msgs, in ms

(async () => {

  liveChat.liveId = "DqYE5mkrMlA";
  const youtube = await ytInit();
  const liveChatId = await getLiveChatId(youtube, liveId);
  const ok = liveChat.start();

  if (!ok) {
    console.error("failed to start");
    //return;
  }

  console.log("liveChatId", liveChatId);

  const processChatQueue = async () => {
    if (!ytChatQueue.length) return;
    const msg = ytChatQueue.shift();
    if (msg) {
      await postChat(youtube, liveChatId, msg);
    }
  };

  setInterval(processChatQueue, ytChatInterval);

})();