'use strict';

const { google } = require('googleapis');
const path = require('path');
const { authenticate } = require('@google-cloud/local-auth');


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
  const res = await youtube.liveBroadcasts.list({ part: "id,snippet,contentDetails,status", id })
  console.log(util.inspect(res, { showHidden: false, depth: null, colors: true }))
  return res.data.items[0].snippet.liveChatId;
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