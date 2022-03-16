const { REDIS_HOST, REDIS_PORT } = require("./env.json");

const redisOpt = {
  host: REDIS_HOST,
  port: REDIS_PORT,
  enableAutoPipelining: true,
};

const upstreamChannel = "aussieup";
const downstreamChannel = "aussiedown";

const botType = Object.freeze({
  DISCORD: 1,
  YOUTUBE: 2,
  TWITCH: 3,
});

const msgType = Object.freeze({
  ERROR: 0,
  STARTED: 1,
  STOPPED: 2,
  CHAT: 3,
  COMMAND: 4,
  POINTS: 5,
  GIVE: 6,
  GAMBLE: 7,
  HEIST: 8,
  LINK: 9,
  SCRAPE_POINTS: 10
});

const pubMsgIsValid = (msg) => true; //TODO

module.exports = { redisOpt, upstreamChannel, downstreamChannel, botType, msgType, pubMsgIsValid };