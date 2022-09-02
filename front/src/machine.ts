import {
  ActorRef,
  assign,
  createMachine,
  interpret,
  send,
  spawn,
  Sender,
  Receiver,
  actions,
} from "xstate";
import {
  configSetIsValid,
  dump_config,
  into_maybe_valid_value,
  parse_config_set,
  parse_schema,
  _debug,
  __DEV__,
} from "./util";
import {
  TSchema,
  TConfig,
  TCmdSchema,
  TConfigSet,
  TConfigSetDump,
  TConfigType,
  TMessage,
  TPayload,
  TConfigDumpPayload,
  TSchemaDump,
  TConfigCursor,
  ConfigTypeValues,
  TPlatform,
  TMessagePayload,
  TStreamEventPayload,
  TStreamSignalPayload,
  TChatPayload,
  TChatUser,
  TLogDumpPayload,
  TPlatformLog,
  TPlatformLogDump,
  TChat,
  TLog,
  ChatPlatforms,
  TChatMeta,
  TAuthMessage,
  TAuthResp,
  TAuthUsers,
  TAuthLogin,
  TModActionsDumpPayload,
  TModActionRow,
  TPlatformModActionsDump,
  TPlatformModActionsLog,
  TModActionsLog,
  TModAction,
  TModActionPayload,
  TModActionItem,
  ModActions,
  TAuthError,
  TAuthErrorType,
  TAuthSuccess,
} from "./types";
const { log } = actions;

const AUTH_LS_KEY = "auth";

const toMessage = (payload: TPayload): TMessage => ({
  platform: TPlatform.Web,
  channel: "aussiegg",
  payload,
});

const isConfigType = (t: string): t is TConfigType => {
  return (ConfigTypeValues as string[]).includes(t);
};

const isConfigDumpPayload = (payload: object): payload is TConfigDumpPayload =>
  "ConfigDump" in payload;
const isSchemaDump = (payload: object): payload is TSchemaDump =>
  "SchemaDump" in payload;
const isMessagePayload = (payload: object): payload is TMessagePayload =>
  "Message" in payload;
const isStreamSignalPayload = (
  payload: object
): payload is TStreamSignalPayload => "StreamSignal" in payload;
const isStreamEventPayload = (
  payload: object
): payload is TStreamEventPayload => "StreamEvent" in payload;
const isChatPayload = (payload: object): payload is TChatPayload =>
  "Chat" in payload;
const isLogDumpPayload = (payload: object): payload is TLogDumpPayload =>
  "LogDump" in payload;
const isModActionPayload = (payload: object): payload is TModActionPayload =>
  "ModAction" in payload;
const isModActionsDumpPayload = (
  payload: object
): payload is TModActionsDumpPayload => "ModActionsDump" in payload;

const isChatUser = (user: Record<string, unknown>): user is TChatUser =>
  "id" in user && "name" in user && "perms" in user;
const isChatMeta = (meta: Record<string, unknown>): meta is TChatMeta =>
  "Youtube" in meta ||
  "Discord1" in meta ||
  "Discord2" in meta ||
  "Discord3" in meta;
const isChat = (chat: object): chat is TChat =>
  "user" in chat &&
  "msg" in chat &&
  isChatUser((chat as { user: Record<string, unknown> }).user) &&
  (!("meta" in chat) ||
    isChatMeta((chat as { meta: Record<string, unknown> }).meta));

const isModAction = (
  action: string | Record<string, unknown>
): action is TModAction => {
  if (typeof action === "string") {
    return ModActions.includes(action);
  }
  return "Timeout" in action;
};

const isModActionRow = (row: unknown): row is TModActionRow => {
  return (
    Array.isArray(row) &&
    row.length === 5 &&
    typeof row[1] === "string" &&
    typeof row[2] === "string" &&
    typeof row[3] === "string" &&
    typeof row[4] === "number"
  );
};

const isPayload = (payload: string | object): payload is TPayload => {
  if (typeof payload === "string") {
    return [
      "DumpConfig",
      "DumpSchema",
      "ConfigChanged",
      "ConfigSaved",
    ].includes(payload);
  }

  if (isMessagePayload(payload)) {
    const msg = payload.Message;
    return "id" in msg && "name" in msg && "msg" in msg;
  }

  if (isConfigDumpPayload(payload)) {
    return Object.keys(payload.ConfigDump).reduce(
      (acc: boolean, type) => acc && isConfigType(type),
      true
    );
  }

  if (isSchemaDump(payload)) return true; //TODO

  if (isStreamSignalPayload(payload)) {
    const sig = payload.StreamSignal;
    return "Stop" in sig || "Start" in sig;
  }

  if (isStreamEventPayload(payload)) {
    const evt = payload.StreamEvent;
    return "Stopped" in evt || "Started" in evt || "Detected" in evt;
  }

  if (isChatPayload(payload)) {
    return isChat(payload.Chat);
  }

  if (isModActionPayload(payload)) {
    const action = payload.ModAction;
    return (
      Array.isArray(action) &&
      action.length === 3 &&
      isChatUser(action[0]) &&
      isModAction(action[1]) &&
      typeof action[2] === "string"
    );
  }

  if (isLogDumpPayload(payload)) {
    return Array.isArray(payload.LogDump); //TODO
  }

  if (isModActionsDumpPayload(payload)) {
    const dump = payload.ModActionsDump;
    return (
      Array.isArray(dump) &&
      dump.reduce(
        (prev, prow) =>
          prev &&
          Array.isArray(prow) &&
          prow.length === 2 &&
          prow[0] in TPlatform &&
          prow[1].reduce(
            (prev, row) => prev && isModActionRow(row),
            true as boolean
          ),
        true as boolean
      )
    );
  }

  return false;
};

const isMessage = (msg: Record<string, unknown>): msg is TMessage => {
  if (!("platform" in msg) /* || !TPlatforms[msg.platform]*/) return false;
  if (!msg.channel) return false;
  if (!msg.payload) return false;
  if (typeof msg.payload !== "object" && typeof msg.payload !== "string")
    return false;
  if (!isPayload(msg.payload)) return false;
  return true;
};

const isAuthSuccessMsg = (msg: object): msg is TAuthSuccess =>
  "AuthSuccess" in msg;
const isAuthErrorMsg = (msg: object): msg is TAuthError => "AuthError" in msg;
const isAuthUsersMsg = (msg: object): msg is TAuthUsers => "Users" in msg;

const isAuthResp = (msg: string | object): msg is TAuthResp => {
  if (typeof msg === "string")
    return [
      "InvalidUser",
      "CodeReady",
      "CodeExpired",
      "AuthFail",
      "AuthError",
    ].includes(msg);

  if (isAuthSuccessMsg(msg) && typeof msg.AuthSuccess === "string") return true;
  if (
    isAuthErrorMsg(msg) &&
    typeof msg.AuthError === "string" &&
    ["Ratelimited", "ServerError"].includes(msg.AuthError)
  )
    return true;
  if (isAuthUsersMsg(msg) && Array.isArray(msg.Users)) return true;

  return false;
};

const appendChatToPlatformLog = (
  _log: TPlatformLog,
  chat: TChat,
  timestamp?: number
): TPlatformLog => {
  const log: TPlatformLog = { ..._log };
  const ts = timestamp ?? Date.now();
  const ref = log[ts];
  if (log[ts]) {
    if (Array.isArray(ref)) {
      ref.push(chat);
    } else {
      log[ts] = [ref, chat];
    }
  } else {
    log[ts] = chat;
  }
  return log;
};

const appendChatToLog = (
  logs: TLog,
  platform: TPlatform,
  chat: TChat,
  timestamp?: number
): TLog => {
  const log = logs[platform] ?? [];
  // Log's useSelector can only detect new objects, not mutated ones
  return { ...logs, [platform]: appendChatToPlatformLog(log, chat, timestamp) };
};

// const pruneLog = (log: TPlatformLog, oldest: number): TPlatformLog => {
//   return Object.fromEntries(
//     Object.entries(log).filter(([time, _]) => parseInt(time) >= oldest)
//   );
// };

const parsePlatformLogDump = (dump: TPlatformLogDump) => {
  const res = dump.reduce((acc, json) => {
    const ts_chat_pair = JSON.parse(json);
    if (!Array.isArray(ts_chat_pair) || ts_chat_pair.length !== 2) return acc;
    const [time_str, chat] = ts_chat_pair;
    if (!isChat(chat)) return acc;
    try {
      const time = parseInt(time_str);
      return appendChatToPlatformLog(acc, chat, time);
    } catch (e) {
      console.error(e);
      return acc;
    }
  }, {} as TPlatformLog);
  return res;
};

const parseLogDump = (payload: TLogDumpPayload): TLog => {
  const res = Object.fromEntries(
    payload.LogDump.map(([platform, dump]) => [
      platform,
      parsePlatformLogDump(dump),
    ])
  );
  return res;
};

const appendModActionToLog = (
  logs: TModActionsLog,
  platform: TPlatform,
  item: TModActionItem,
  timestamp: number
): TModActionsLog => {
  const log = logs[platform] ?? [];
  const [user, action, reason] = item;
  return {
    ...logs,
    [platform]: [[user.name, user.id, action, reason, timestamp], ...log],
  };
};

const parsePlatformModActionsDump = (
  dump: TPlatformModActionsDump
): TPlatformModActionsLog => {
  return dump;
};

const parseModActionsDump = (
  payload: TModActionsDumpPayload
): TModActionsLog => {
  const res = Object.fromEntries(
    payload.ModActionsDump.map(([platform, dump]) => [
      platform,
      parsePlatformModActionsDump(dump),
    ])
  );
  return res;
};

const parseMessage = (msg: TMessage, send: Sender<TEvent>) => {
  const { payload } = msg;

  if (typeof payload === "string") {
    if (payload === "ConfigChanged") {
      send("CONFIG_CHANGE_NOTIF");
    }

    if (payload === "ConfigSaved") {
      send("CONFIG_SAVED");
    }

    return;
  }

  if (isMessagePayload(payload)) {
    debug("Chat message", payload.Message);
  }

  if (isConfigDumpPayload(payload)) {
    send({
      type: "CONFIG",
      configDump: payload.ConfigDump,
    });
  }

  if (isSchemaDump(payload)) {
    send({ type: "SCHEMA", schema: payload.SchemaDump });
  }

  if (isLogDumpPayload(payload)) {
    const log = parseLogDump(payload);
    send({ type: "STATS_LOG_DUMP", log });
  }

  if (isChatPayload(payload)) {
    send({ type: "STATS_CHAT", chat: payload.Chat, platform: msg.platform });
  }

  if (isModActionPayload(payload)) {
    send({
      type: "STATS_MODACTION",
      modAction: payload.ModAction,
      platform: msg.platform,
    });
  }

  if (isModActionsDumpPayload(payload)) {
    const modActions = parseModActionsDump(payload);
    send({ type: "STATS_MODACTIONS_DUMP", modActions });
  }
};

const parseAuthResp = (resp: TAuthResp, send: Sender<TEvent>) => {
  debug("parseAuthResp", resp);
  if (typeof resp === "string") {
    if (resp === "AuthFail") {
      send({ type: "AUTH_FAIL" });
    } else if (resp === "CodeReady") {
      send({ type: "AUTH_CODE_READY" });
    } else if (resp === "CodeExpired") {
      send({ type: "AUTH_CODE_EXPIRED" });
    } else if (resp === "InvalidUser") {
      //send({ type: "AUTH_SUCCESS" });
    } else {
      console.error("invalid auth resp", resp);
    }
  } else if (isAuthSuccessMsg(resp)) {
    send({ type: "AUTH_SUCCESS", user: resp.AuthSuccess });
  } else if (isAuthErrorMsg(resp)) {
    send({ type: "AUTH_ERROR", error: resp.AuthError });
  } else if (isAuthUsersMsg(resp)) {
    send({ type: "AUTH_LIST_USERS", users: resp.Users });
  }
};

type TSocketEvent =
  | { type: "WS_TX"; msg: TMessage | TAuthMessage }
  | { type: "WS_RECONNECT" }
  | { type: "WS_START_TIMER" }
  | { type: "WS_STOP_TIMER" };

const initSocket = (
  callback: Sender<TEvent>,
  onReceive: Receiver<TSocketEvent>
) => {
  const _initSocket = (): WebSocket => {
    const socket = new WebSocket(
      true ? "wss://abapi.siid.sh" : "ws://192.168.1.97:3001"
    );
    // Listen for messages
    socket.addEventListener("message", function (event) {
      try {
        const obj = JSON.parse(event.data);
        debug("RECEIVED", obj);
        if (isAuthResp(obj)) {
          parseAuthResp(obj, callback);
        } else if (isMessage(obj)) {
          parseMessage(obj, callback);
        } else {
          debug("invalid msg", obj);
        }
      } catch (e) {}
    });
    // Connection opened
    socket.addEventListener("open", function (event) {
      console.log("socket opened");
      callback({ type: "WS_OPEN" });
    });

    socket.addEventListener("error", function (event) {
      console.error("socket error", event);
      this.close();
    });

    socket.addEventListener("close", function (event) {
      console.error("socket closed");
      callback({ type: "WS_CLOSE" });
    });

    return socket;
  };

  let socket = _initSocket();
  let timer: NodeJS.Timer | null = null;
  const PING_INTERVAL_MS = 30000;

  onReceive((event: TSocketEvent) => {
    switch (event.type) {
      case "WS_TX":
        if (socket.readyState === WebSocket.OPEN) {
          debug("SENDING", event.msg);
          socket.send(JSON.stringify(event.msg));
        }
        break;
      case "WS_RECONNECT":
        if (socket.readyState !== WebSocket.OPEN) socket = _initSocket();
        break;
      case "WS_START_TIMER":
        timer = setInterval(() => socket.send("💓"), PING_INTERVAL_MS);
        debug("startPingTimer", timer);
        break;
      case "WS_STOP_TIMER":
        if (!timer) break;
        clearInterval(timer);
        debug("stopPingTimer", timer);
        timer = null;
        break;
    }
  });
};

export type TMainContext = {
  login: TAuthLogin | null;
  user: string | null;
  users: string[];
  socket: WebSocket;
  configDump: TConfigSetDump;
  schema: TSchema;
  config: TConfigSet;
  prevConfig: TConfigSet;
  configChanged: boolean;
  configValid: boolean;
  currentCursor: TConfigCursor;
  prevCursor: TConfigCursor;
  currentAdd: TConfigType;
  currentAddChoices: string[];
  socketRef: ActorRef<TSocketEvent, undefined>;
  log: TLog;
  modActions: TModActionsLog;
};

type TEvent =
  | { type: "WS_OPEN" }
  | { type: "WS_CLOSE" }
  | { type: "WS_RECV"; msg: TMessage }
  | { type: "AUTH_LIST_USERS"; users: string[] }
  | { type: "AUTH_USER_SELECTED"; user: string }
  | { type: "AUTH_CODE_REQUESTED" }
  | { type: "AUTH_CODE_READY" }
  | { type: "AUTH_CODE_ENTERED"; code: string }
  | { type: "AUTH_CODE_EXPIRED" }
  | { type: "AUTH_SUCCESS"; user: string }
  | { type: "AUTH_FAIL" }
  | { type: "AUTH_ERROR"; error: TAuthErrorType }
  | { type: "SCHEMA"; schema: TCmdSchema[] }
  | { type: "CONFIG"; configDump: TConfigSetDump }
  | { type: "CONFIG_CHANGED"; config: TConfig; cursor: TConfigCursor }
  | { type: "CONFIG_CHANGE_NOTIF" }
  | { type: "CONFIG_SELECT"; cursor: TConfigCursor }
  | { type: "CONFIG_DELETE"; cursor: TConfigCursor }
  | { type: "CONFIG_REVERT" }
  | { type: "CONFIG_ADD"; cmdType: TConfigType }
  | { type: "CONFIG_CANCEL_ADD" }
  | { type: "CONFIG_ADD_CMD"; cmdName: string }
  | { type: "CONFIG_SAVE" }
  | { type: "CONFIG_ERROR_CLOSE" }
  | { type: "CONFIG_SAVED" }
  | { type: "CONFIG_CHANGE_RELOAD" }
  | { type: "CONFIG_CHANGE_IGNORE" }
  | { type: "STATS_DUMP_LOG" }
  | { type: "STATS_LOG_DUMP"; log: TLog } // initial log dump
  | { type: "STATS_CHAT"; chat: TChat; platform: TPlatform } // subsequent chats while client is active (we supply the timestamp)
  | { type: "STATS_MODACTION"; modAction: TModActionItem; platform: TPlatform }
  | { type: "STATS_MODACTIONS_DUMP"; modActions: TModActionsLog }; // initial modactions dump

const mainMachine = createMachine(
  {
    id: "main",
    strict: true,
    tsTypes: {} as import("./machine.typegen").Typegen0,
    context: {
      login: null,
      user: null,
      users: [] as string[],
      configDump: {} as TConfigSetDump,
      schema: {} as TSchema,
      config: {} as TConfigSet,
      prevConfig: {} as TConfigSet,
      configChanged: false,
      configValid: false,
      currentCursor: { type: TConfigType.Command, index: -1 },
      prevCursor: { type: TConfigType.Command, index: -1 },
      currentAdd: TConfigType.Command,
      currentAddChoices: [] as string[],
      log: {},
      modActions: {},
    } as TMainContext,
    schema: {
      context: {} as TMainContext,
      events: {} as TEvent,
      services: {}, // as TServices,
    },
    on: {
      WS_CLOSE: "socketClosed",
      STATS_DUMP_LOG: { actions: "requestLog" },
      STATS_LOG_DUMP: { actions: "setLog" },
      STATS_MODACTIONS_DUMP: { actions: "setModActions" },
      STATS_CHAT: { actions: "appendChatToLog" },
      STATS_MODACTION: { actions: "appendModActionToLog" },
    },
    initial: "preinit",
    states: {
      preinit: {
        always: [
          {
            target: "init",
            cond: "hasWindow", // stop machine during ssr
          },
        ],
      },
      init: {
        entry: "initSocket",
        on: {
          WS_OPEN: { target: "auth", actions: "startPingTimer" },
          WS_CLOSE: "socketClosed",
        },
      },
      auth: {
        entry: "loadAuth",
        exit: ["saveAuth", log("exiting auth")],
        initial: "init",
        on: {
          AUTH_SUCCESS: { target: ".success", cond: "authSuccess" },
          AUTH_ERROR: [
            { target: ".ratelimited", cond: "authRatelimited" },
            { target: ".getListUsers" },
          ],
        },
        onDone: "reqSettings",
        states: {
          init: {
            always: [
              {
                target: "tryAuth",
                cond: "hasLoginData",
              },
              {
                target: "getListUsers",
              },
            ],
          },
          tryAuth: {
            entry: "tryAuth",
            on: {
              AUTH_FAIL: [
                { target: "selectUser", cond: "hasAuthUsers" },
                { target: "getListUsers" },
              ],
              AUTH_CODE_EXPIRED: [
                { target: "selectUser", cond: "hasAuthUsers" },
                { target: "getListUsers" },
              ],
            },
          },
          getListUsers: {
            entry: "authListUsers",
            on: {
              AUTH_LIST_USERS: {
                target: "selectUser",
                actions: "setAuthUsers",
              },
            },
          },
          selectUser: {
            on: {
              AUTH_USER_SELECTED: {
                target: "inputCode",
                actions: "setAuthUser",
              },
            },
          },
          inputCode: {
            type: "parallel",
            states: {
              codeRequest: {
                initial: "idle",
                on: {
                  AUTH_CODE_REQUESTED: {
                    target: ".pending",
                    actions: "authReqCode",
                  },
                  AUTH_CODE_READY: ".ready",
                },
                states: {
                  idle: {},
                  pending: {},
                  ready: {},
                },
              },
              login: {
                initial: "idle",
                on: {
                  AUTH_CODE_ENTERED: {
                    target: ".pending",
                    actions: ["setAuth", "tryAuth"],
                  },
                  AUTH_FAIL: ".failed",
                  AUTH_CODE_EXPIRED: {
                    target: ".idle",
                    actions: "authReqCode",
                  },
                },
                states: {
                  idle: {},
                  pending: {},
                  failed: {},
                },
              },
            },
          },
          ratelimited: {},
          success: {
            type: "final",
          },
        },
      },
      reqSettings: {
        type: "parallel",
        onDone: "ready",
        entry: [
          "requestSchema",
          "requestConfig",
          "requestLog",
          "requestModActions",
        ],
        exit: ["parseConfig", "checkConfigValid", "savePrevConfig"],
        states: {
          schema: {
            initial: "pending",
            states: {
              pending: {
                on: {
                  SCHEMA: {
                    target: "ready",
                    actions: "parseSchema",
                  },
                },
              },
              ready: {
                type: "final",
              },
            },
          },
          config: {
            initial: "pending",
            states: {
              pending: {
                on: {
                  CONFIG: {
                    target: "ready",
                    actions: ["setConfigDump"],
                  },
                },
              },
              ready: {
                type: "final",
              },
            },
          },
        },
      },
      reqConfig: {
        entry: ["requestConfig"],
        on: {
          CONFIG: {
            target: "ready",
            actions: [
              "setConfigDump",
              "parseConfig",
              "checkConfigValid",
              "savePrevConfig",
            ],
          },
        },
      },
      socketClosed: {
        entry: ["stopPingTimer", "reconnectSocket"],
        exit: "startPingTimer",
        on: {
          WS_OPEN: "auth",
        },
      },
      ready: {
        on: {
          CONFIG_CHANGED: {
            actions: ["updateConfig", "checkConfigValid"],
          },
          CONFIG_SELECT: {
            actions: ["selectConfig", "savePrevCursor"],
          },
          CONFIG_DELETE: {
            actions: "deleteConfig",
          },
          CONFIG_REVERT: {
            actions: "revertToPrevConfig",
          },
          CONFIG_ADD: {
            target: "preAddCommand",
            actions: ["prepareAddCmd", "setCurrentAddChoices"],
          },
          CONFIG_SAVE: [
            { target: "saveConfig", cond: "configValid" },
            { target: "configInvalid" },
          ],
          CONFIG_CHANGE_NOTIF: [
            { target: "reqConfig", cond: "configNotChanged" },
            { target: "configChangedExternally" },
          ],
        },
      },
      preAddCommand: {
        always: [
          {
            target: "addCommand",
            cond: "checkAddCommand",
          },
          { target: "ready" },
        ],
      },
      addCommand: {
        on: {
          CONFIG_CANCEL_ADD: "ready",
          CONFIG_ADD_CMD: {
            target: "ready",
            actions: "addCommand",
          },
        },
      },
      saveConfig: {
        // send to server and wait for resp or timeout
        entry: ["sendConfigDump"],
        after: {
          10000: "configSaveFailed",
        },
        on: {
          CONFIG_SAVED: {
            target: "ready",
            actions: ["clearConfigChanged", "savePrevCursor"],
          }, //TODO: currentCursor might be invalidated
          CONFIG_CHANGE_NOTIF: {}, // TODO: this means someone else's save went thru instead of ours, show an alert
        },
      },
      // show alerts
      configInvalid: {
        on: {
          CONFIG_ERROR_CLOSE: "ready",
        },
      },
      configSaveFailed: {
        on: {
          CONFIG_ERROR_CLOSE: "ready",
        },
      },
      configChangedExternally: {
        on: {
          CONFIG_CHANGE_RELOAD: "reqConfig",
          CONFIG_CHANGE_IGNORE: "ready",
        },
      },
    },
  },
  {
    actions: {
      //log: (ctx, e) => console.log("log", ctx, e),
      loadAuth: assign((ctx, e) => {
        const auth = window.localStorage.getItem(AUTH_LS_KEY);
        if (auth !== null) {
          try {
            const Login = JSON.parse(auth);
            if (!Array.isArray(Login) || Login.length !== 2) return {};
            return { login: { Login } as TAuthLogin, user: Login[0] };
          } catch (e) {
            console.error(e);
            return {};
          }
        }
        return {};
      }),
      saveAuth: (ctx, e) =>
        ctx.login &&
        window.localStorage.setItem(
          AUTH_LS_KEY,
          JSON.stringify(ctx.login.Login)
        ),
      tryAuth: send((ctx) => ({ type: "WS_TX", msg: ctx.login }), {
        to: (ctx) => ctx.socketRef,
      }),
      setAuth: assign({
        login: (ctx, e) => ({ Login: [ctx.user, e.code] } as TAuthLogin),
      }),
      authListUsers: send(
        {
          type: "WS_TX",
          msg: "ListUsers",
        },
        { to: (ctx) => ctx.socketRef }
      ),
      setAuthUsers: assign({
        users: (ctx, e) => e.users.sort() as string[],
      }),
      setAuthUser: assign({
        user: (ctx, e) => e.user,
      }),
      authReqCode: send(
        (ctx) => ({
          type: "WS_TX",
          msg: { RequestCode: ctx.user },
        }),
        { to: (ctx) => ctx.socketRef }
      ),
      initSocket: assign({
        socketRef: (ctx, e) => spawn(initSocket),
      }),
      startPingTimer: send(
        { type: "WS_START_TIMER" },
        { to: (ctx) => ctx.socketRef }
      ),
      stopPingTimer: send(
        { type: "WS_STOP_TIMER" },
        { to: (ctx) => ctx.socketRef }
      ),
      reconnectSocket: send(
        { type: "WS_RECONNECT" },
        { to: (ctx) => ctx.socketRef }
      ),
      requestSchema: send(
        { type: "WS_TX", msg: toMessage("DumpSchema") },
        { to: (ctx) => ctx.socketRef }
      ),
      requestConfig: send(
        { type: "WS_TX", msg: toMessage("DumpConfig") },
        { to: (ctx) => ctx.socketRef }
      ),
      requestLog: send(
        {
          type: "WS_TX",
          msg: toMessage({ DumpLog: ChatPlatforms }),
        },
        { to: (ctx) => ctx.socketRef }
      ),
      requestModActions: send(
        { type: "WS_TX", msg: toMessage("DumpModActions") },
        { to: (ctx) => ctx.socketRef }
      ),
      setLog: assign({ log: (_, e) => e.log }),
      appendChatToLog: assign({
        log: (ctx, e) =>
          appendChatToLog(
            ctx.log,
            e.platform,
            e.chat,
            Math.floor(Date.now()) // js's timestamp is in msecs
          ),
      }),
      appendModActionToLog: assign({
        modActions: (ctx, e) =>
          appendModActionToLog(
            ctx.modActions,
            e.platform,
            e.modAction,
            Math.floor(Date.now() / 1000) // js's timestamp is in msecs
          ),
      }),
      setModActions: assign({ modActions: (_, e) => e.modActions }),
      setConfigDump: assign({ configDump: (_, e) => e.configDump }),
      parseSchema: assign({ schema: (_, e) => parse_schema(e.schema) }),
      parseConfig: assign({
        config: (ctx, e) => parse_config_set(ctx.configDump, ctx.schema),
        configChanged: (ctx, e) => false,
        //currentCursor: (ctx, e) => ({ type: TConfigType.Command, index: 0 }), //TODO
      }),
      checkConfigValid: assign({
        configValid: (ctx, e) => configSetIsValid(ctx.config),
      }),
      savePrevConfig: assign({
        prevConfig: (ctx, e) => JSON.parse(JSON.stringify(ctx.config)),
      }),
      savePrevCursor: assign({
        prevCursor: (ctx, e) =>
          ctx.configChanged // after a change, don't store cursors till a save
            ? ctx.prevCursor
            : JSON.parse(JSON.stringify(ctx.currentCursor)),
      }),
      revertToPrevConfig: assign({
        config: (ctx, e) => {
          const config = JSON.parse(JSON.stringify(ctx.prevConfig));
          debug("REVERTING TO", config);
          return config;
        },
        currentCursor: (ctx, e) => ctx.prevCursor,
        configChanged: (ctx, e) => false,
      }),
      sendConfigDump: send(
        (ctx, e) => {
          const ConfigDump = {
            commands: dump_config(ctx.config.commands),
            filters: dump_config(ctx.config.filters),
            timers: dump_config(ctx.config.timers),
          };
          return {
            type: "WS_TX",
            msg: toMessage({ ConfigDump }),
          };
        },
        { to: (ctx) => ctx.socketRef }
      ),
      clearConfigChanged: assign({
        configChanged: (ctx, e) => false,
      }),
      updateConfig: assign({
        config: (ctx, e) => {
          const { type, index } = e.cursor;
          return {
            ...ctx.config,
            [type]: ctx.config[type].map((v, i) =>
              i !== index ? v : e.config
            ),
          };
        },
        configChanged: (ctx, e) => true,
      }),
      selectConfig: assign({
        currentCursor: (ctx, e) => {
          const { type, index } = e.cursor;
          const newIndex = Math.min(
            Math.max(index, 0),
            ctx.config[type].length - 1
          );
          return { type, index: newIndex };
        },
      }),
      deleteConfig: assign((ctx, e) => {
        const { type, index: delIndex } = e.cursor;

        const configList = ctx.config[type];
        const validDelIdx = 0 <= delIndex && delIndex < configList.length;
        const config = validDelIdx
          ? {
              ...ctx.config,
              [type]: configList.filter((_, i) => i !== delIndex),
            }
          : ctx.config;
        const configChanged = validDelIdx;

        const { type: currType, index: currIndex } = ctx.currentCursor;
        if (type === currType) {
          // might have to change current index
          const newIndex =
            currIndex === delIndex
              ? Math.max(delIndex - 1, 0) // switch to prev
              : currIndex > delIndex
              ? currIndex - 1 // shift down
              : currIndex;
          return {
            config,
            configChanged,
            currentCursor: { type, index: newIndex },
          };
        } else {
          return { config, configChanged };
        }
      }),
      prepareAddCmd: assign({
        currentAdd: (Ctx, e) => e.cmdType,
      }),
      setCurrentAddChoices: assign({
        currentAddChoices: (ctx, e) =>
          Object.keys(ctx.schema).filter(
            (name) => TConfigType[ctx.schema[name].configType] === e.cmdType
          ),
      }),
      addCommand: assign((ctx, e) => {
        const cmd_tmpl = ctx.schema[e.cmdName];
        const new_cmd = {
          type: e.cmdName,
          name: "",
          fields: Object.fromEntries(
            Object.entries(cmd_tmpl.fields).map(
              ([field, { def_value, constraint }]) => [
                field,
                into_maybe_valid_value(def_value, constraint),
              ]
            )
          ),
        } as TConfig;
        const config = {
          ...ctx.config,
          [ctx.currentAdd]: [...ctx.config[ctx.currentAdd], new_cmd],
        };
        return {
          config,
          configChanged: true,
          currentCursor: {
            type: ctx.currentAdd,
            index: config[ctx.currentAdd].length - 1,
          },
        };
      }),
    },
    guards: {
      authRatelimited: (ctx, e) => e.error === "Ratelimited",
      authSuccess: (ctx, e) => ctx.user === e.user,
      //authServerError: (ctx, e) => e.error === "ServerError",
      hasWindow: (ctx, e) => typeof window !== "undefined",
      hasLoginData: (ctx, e) => ctx.login !== null,
      hasAuthUsers: (ctx, e) => ctx.users.length > 0,
      checkAddCommand: (ctx, e) => ctx.currentAddChoices.length > 0,
      configValid: (ctx, e) => ctx.configValid,
      configNotChanged: (ctx, e) => !ctx.configChanged,
    },
  }
);

export const mainService = interpret(mainMachine, {
  devTools: false,
})
  .onEvent((event) => _debug("event", event))
  .onTransition((state) => {
    console.log("stateΔ", state.value, state.context);
  })
  .start();

export const emit = mainService.send;

export const devMenu = () => mainService.state.context.user === "🦙";
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const debug: any = (...data: any[]) =>
  __DEV__ || devMenu() ? console.log(...data) : () => null;
