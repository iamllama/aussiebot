const instanceId = "aussiedisc";

// Require the necessary discord.js classes
const { Client, Intents } = require('discord.js');
const { MessageActionRow, MessageButton } = require('discord.js');
const { token, clientId, guildId } = require('./config.json');
const { SlashCommandBuilder } = require('@discordjs/builders');
const { REST } = require('@discordjs/rest');
const { Routes } = require('discord-api-types/v9');

const Redis = require("ioredis");
const { redisOpt, upstreamChannel, downstreamChannel, botType, msgType, pubMsgIsValid } = require("../util");
const pub = new Redis(redisOpt);
const sub = new Redis(redisOpt);
sub.subscribe(downstreamChannel);

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
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, msgType.POINTS, id]));
      break;
    case "give": {
      await interaction.deferReply();
      callbacks[id] = interaction;
      const target = interaction.options.getUser('target', true).id;
      const amount = interaction.options.getNumber("points", true);
      console.log(target, amount);
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, msgType.GIVE, id, target, amount]));
      break;
    }
    case "link": {
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, msgType.LINK, id]));
      break;
    }
    case "gamble":
      const amount = interaction.options.getNumber("points", true);
      await interaction.deferReply();
      callbacks[id] = interaction;
      await wait(2000);
      callbacks[id].editReply(`blah`);
      delete callbacks[id];
      break;
    default:
      await interaction.deferReply();
      callbacks[id] = interaction;
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, msgType.COMMAND, id, commandName]));
      break;
  }
});

client.on('messageCreate', async message => {
  console.log("messageCreate");
  if (message.author.bot) return;
  //let fullMessage = (message.partial) ? await message.fetch() : message;
  //if (fullMessage.author.bot) return;
  const source = message.author.id;
  const text = message.content;
  if (text.startsWith('!')) processChatMsg(source, text);
  pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, msgType.CHAT, source]));
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
      pub.publish(upstreamChannel, JSON.stringify([instanceId, botType.DISCORD, msgType.GIVE, source, target, amount]));
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
    debug(e);
    return;
  }

  if (!pubMsgIsValid(msg) || msg[0] != instanceId) return;

  const id = msg[2];
  if (!callbacks[id]) return;

  switch (msg[1]) {
    case msgType.POINTS: {
      const points = msg[3];
      callbacks[id].editReply({ content: `You have ${points} point(s)` });
      delete callbacks[id];
      break;
    }
    case msgType.GIVE: {
      const target = msg[3], amount = msg[4];
      callbacks[id].editReply(`You gave @${target} ${amount} point(s)`);
      delete callbacks[id];
      break;
    }
    case msgType.ERROR: {
      const text = msg[3] ?? "Error";
      callbacks[id].editReply({ content: `⚠️ ${text}`, ephemeral: true });
      delete callbacks[id];
      break;
    }
    default:
      break;
  }

}

client.login(token);