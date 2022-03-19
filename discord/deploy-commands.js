const { SlashCommandBuilder } = require('@discordjs/builders');
const { REST } = require('@discordjs/rest');
const { Routes } = require('discord-api-types/v9');
const { clientId, guildId, token } = require('./config.json');

const pointsOpt = (opt) => opt.setName('points').setDescription('Amount to wager').setRequired(true);
const userOpt = (opt) => opt.setName('target').setDescription('Select a user').setRequired(true);

const commands = [
  new SlashCommandBuilder().setName('points').setDescription('CHeck your point balance'),
  new SlashCommandBuilder().setName('gamble').setDescription('Win or lose big').addNumberOption(pointsOpt),
  new SlashCommandBuilder().setName('give').setDescription('Give points to someone').addUserOption(userOpt).addNumberOption(pointsOpt),
  new SlashCommandBuilder().setName('link').setDescription('Link your yt/twitch points'),
  new SlashCommandBuilder().setName('heist').setDescription('Start a heist 🏴‍☠️').addNumberOption(pointsOpt)
]
  .map(command => command.toJSON());

const rest = new REST({ version: '9' }).setToken(token);

rest.put(Routes.applicationGuildCommands(clientId, guildId), { body: commands })
  .then(() => console.log('Successfully registered application commands.'))
  .catch(console.error);