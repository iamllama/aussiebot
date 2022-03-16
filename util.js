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
  COMMAND: 3,
  POINTS: 4,
  GIVE: 5,
  GAMBLE: 6,
  HEIST: 7,
  LINK: 8
});

const pubMsgIsValid = (msg) => true; //TODO

module.exports = { redisOpt, upstreamChannel, downstreamChannel, botType, msgType, pubMsgIsValid };