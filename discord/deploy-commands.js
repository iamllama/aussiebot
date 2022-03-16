const { SlashCommandBuilder } = require('@discordjs/builders');
const { REST } = require('@discordjs/rest');
const { Routes } = require('discord-api-types/v9');
const { clientId, guildId, token } = require('./config.json');

const commands = [
  new SlashCommandBuilder().setName('points').setDescription('CHeck your point balance'),
  new SlashCommandBuilder().setName('gamble').setDescription('Win or lose big').addNumberOption(option => option.setName('points').setDescription('Amount to wager')),
  new SlashCommandBuilder().setName('give').setDescription('Give points to someone').addUserOption(option => option.setName('target').setDescription('Select a user')).addNumberOption(option => option.setName('points').setDescription('Amount to give')),
  new SlashCommandBuilder().setName('link').setDescription('Link your yt/twitch points')
]
  .map(command => command.toJSON());

const rest = new REST({ version: '9' }).setToken(token);

rest.put(Routes.applicationGuildCommands(clientId, guildId), { body: commands })
  .then(() => console.log('Successfully registered application commands.'))
  .catch(console.error);