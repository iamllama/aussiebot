const instanceId = process.env.ID ?? "aussiedisc";

// Require the necessary discord.js classes
const { Client, Intents } = require('discord.js');
const { MessageActionRow, MessageButton } = require('discord.js');
const { token, clientId, guildId } = require('./config.json');
const { SlashCommandBuilder } = require('@discordjs/builders');
const { REST } = require('@discordjs/rest');
const { Routes } = require('discord-api-types/v9');

const Redis = require("ioredis");
const { redisOpt, upstreamChannel, downstreamChannel, broadcastId, botType, msgType, pubMsgIsValid } = require("../util");
const pub = new Redis(redisOpt);
const sub = new Redis(redisOpt);
sub.subscribe(downstreamChannel);

const send = (...args) => pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, ...args]));
const wait = require('node:timers/promises').setTimeout;

// Create a new client instance
const client = new Client({ intents: [Intents.FLAGS.GUILDS, Intents.FLAGS.GUILD_MESSAGES], partials: ["MESSAGE", "CHANNEL"] });

// When the client is ready, run this code (only once)
client.once('ready', () => {
  console.log('Ready!');
});

const callbacks = {};

client.on('interactionCreate', async interaction => {
  if (!interaction.isCommand()) return;

  const { commandName } = interaction;
  const { id } = interaction.user;

  if (callbacks[id]) {
    interaction.reply({ content: '⌛', ephemeral: true });
    return;
  }

  switch (commandName) {
    case "points":
      await interaction.deferReply();
      callbacks[id] = interaction;
      send(msgType.POINTS, id);
      break;
    case "give": {
      await interaction.deferReply();
      callbacks[id] = interaction;
      const target = interaction.options.getUser('target', true).id;
      const amount = interaction.options.getNumber("points", true);
      console.log(target, amount);
      send(msgType.GIVE, id, target, amount);
      break;
    }
    case "link": {
      send(msgType.LINK, id);
      break;
    }
    case "gamble": {
      const amount = interaction.options.getNumber("points", true);
      await interaction.deferReply();
      callbacks[id] = interaction;
      send(msgType.GAMBLE, id, amount);
      break;
    }
    case "heist": {
      const amount = interaction.options.getNumber("points", true);
      await interaction.deferReply();
      callbacks[id] = interaction;
      send(msgType.HEIST, id, amount, `${interaction.user.username}#${interaction.user.discriminator}`);
      break;
    }
    default:
      await interaction.deferReply();
      callbacks[id] = interaction;
      send(msgType.COMMAND, id, commandName);
      break;
  }
});

client.on('messageCreate', async message => {
  console.log("messageCreate");
  if (message.author.bot) return;
  const source = message.author.id;
  const text = message.content;
  if (text.startsWith('!')) processChatMsg(source, text);
  send(msgType.CHAT, source);
});

sub.on("message", (chn, msg) => {
  switch (chn) {
    case downstreamChannel:
      processMsg(msg);
      break;
    default:
      break;
  }
});

const processChatMsg = (source, text) => {
  const cmd = /^!(\w+)/.exec(text);
  if (!cmd) return;
  switch (cmd[1]) {
    case "give":
      const cmd = /^!give <@!(\d+)> (\d+)$/.exec(text);
      if (cmd === null) break;
      console.log(source, target, amount);
      send(msgType.GIVE, source, target, amount);
      break;
    default:
      break;
  }
}

const processMsg = (origMsg) => {
  let msg = null;
  try {
    msg = JSON.parse(origMsg);
  } catch (e) {
    console.log(e);
    return;
  }

  if (!pubMsgIsValid(msg) || (msg[0] !== instanceId && msg[0] !== broadcastId)) return;

  const id = msg[2];
  if (!callbacks[id] && msg[0] !== broadcastId) return;

  switch (msg[1]) {
    case msgType.POINTS: {
      const points = msg[3];
      callbacks[id].editReply({ content: `You have ${points} point(s)` });
      break;
    }
    case msgType.GIVE: {
      const target = msg[3], amount = msg[4];
      callbacks[id].editReply(`You gave <@!${target}> ${amount} point(s)`);
      break;
    }
    case msgType.ERROR: {
      const text = msg[3] ?? "Error";
      callbacks[id].editReply({ content: `⚠️ ${text}`, ephemeral: true });
      break;
    }
    case msgType.GAMBLE: {
      const roll = msg[3], delta = msg[4], points = msg[5];
      const content = `You rolled ${roll}, ${delta > 0 ? "won" : "lost"} ${delta > 0 ? delta : -delta} point(s), and now have ${points} point(s)`;
      callbacks[id].editReply({ content });
      break;
    }
    case msgType.HEIST: {
      //onst roll = msg[3], delta = msg[4], points = msg[5];
      //const content = `You rolled ${roll}, ${delta > 0 ? "won" : "lost"} ${delta > 0 ? delta : -delta} point(s), and now have ${points} point(s)`;
      //callbacks[id].editReply({ content: "ok" });
      console.log(msg);
      break;
    }
    default:
      break;
  }

  delete callbacks[id];
}

client.login(token);