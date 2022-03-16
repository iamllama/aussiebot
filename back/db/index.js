const { Pool } = require('pg')
const { DB_USER, DB_HOST, DB_NAME, DB_PASS, DB_PORT } = require("../../env.json");

const pool = new Pool({
  user: DB_USER,
  host: DB_HOST,
  database: DB_NAME,
  password: DB_PASS,
  port: DB_PORT,
})

module.exports = {
  query: (text, params) => pool.query(text, params),
  getClient: () => pool.connect()
};
/*
const { Sequelize } = require('sequelize');
const sequelize = new Sequelize('aussiebot', 'postgres', 'changeme', {
  host: '192.168.1.65',
  dialect: 'postgres'
});

module.exports = { sequelize }*/