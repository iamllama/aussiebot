'use strict';

const { google } = require('googleapis');
const path = require('path');
const { authenticate } = require('@google-cloud/local-auth');
const fetch = (...args) => import('node-fetch').then(({ default: fetch }) => fetch(...args));
const { YT_API_KEY } = require("../env.json");

const ytInit = async () => {
  // initialize the Youtube API library
  const youtube = google.youtube('v3');

  const auth = await authenticate({
    keyfilePath: path.join(__dirname, '../oauth2.keys.json'),
    scopes: ['https://www.googleapis.com/auth/youtube.force-ssl'],
  });

  google.options({ auth });

  return youtube;
};

const util = require("util");


async function getLiveChatId(youtube, id) {
  const res = await fetch(`https://www.googleapis.com/youtube/v3/videos?part=liveStreamingDetails&id=${id}&key=${YT_API_KEY}`);
  const data = await res.json();
  return data.items[0].liveStreamingDetails.activeLiveChatId;
}

async function postChat(youtube, liveChatId, messageText) {
  if (youtube === null) return;
  return youtube.liveChatMessages.insert({
    part: 'snippet',
    resource: {
      snippet: {
        type: "textMessageEvent",
        liveChatId,
        textMessageDetails: {
          messageText
        }
      }
    }
  });
}

module.exports = { ytInit, postChat, getLiveChatId };