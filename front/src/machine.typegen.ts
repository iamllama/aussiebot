// This file was automatically generated. Edits will be overwritten

export interface Typegen0 {
  "@@xstate/typegen": true;
  internalEvents: {
    "": { type: "" };
    "xstate.after(10000)#main.saveConfig": {
      type: "xstate.after(10000)#main.saveConfig";
    };
    "xstate.init": { type: "xstate.init" };
    "xstate.stop": { type: "xstate.stop" };
  };
  invokeSrcNameMap: {};
  missingImplementations: {
    actions: never;
    services: never;
    guards: never;
    delays: never;
  };
  eventsCausingActions: {
    addCommand: "CONFIG_ADD_CMD";
    appendChatToLog: "STATS_CHAT";
    appendModActionToLog: "STATS_MODACTION";
    authListUsers: "" | "AUTH_CODE_EXPIRED" | "AUTH_ERROR" | "AUTH_FAIL";
    authReqCode: "AUTH_CODE_EXPIRED" | "AUTH_CODE_REQUESTED";
    checkConfigValid:
      | "CONFIG"
      | "CONFIG_CHANGED"
      | "SCHEMA"
      | "WS_CLOSE"
      | "done.state.main.reqSettings"
      | "xstate.stop";
    clearConfigChanged: "CONFIG_SAVED";
    deleteConfig: "CONFIG_DELETE";
    initSocket: "";
    loadAuth: "WS_OPEN";
    parseConfig:
      | "CONFIG"
      | "SCHEMA"
      | "WS_CLOSE"
      | "done.state.main.reqSettings"
      | "xstate.stop";
    parseSchema: "SCHEMA";
    prepareAddCmd: "CONFIG_ADD";
    reconnectSocket: "WS_CLOSE";
    requestConfig:
      | "CONFIG_CHANGE_NOTIF"
      | "CONFIG_CHANGE_RELOAD"
      | "done.state.main.auth";
    requestLog: "STATS_DUMP_LOG" | "done.state.main.auth";
    requestModActions: "done.state.main.auth";
    requestSchema: "done.state.main.auth";
    revertToPrevConfig: "CONFIG_REVERT";
    saveAuth:
      | "AUTH_SUCCESS"
      | "WS_CLOSE"
      | "done.state.main.auth"
      | "xstate.stop";
    savePrevConfig:
      | "CONFIG"
      | "SCHEMA"
      | "WS_CLOSE"
      | "done.state.main.reqSettings"
      | "xstate.stop";
    savePrevCursor: "CONFIG_SAVED" | "CONFIG_SELECT";
    selectConfig: "CONFIG_SELECT";
    sendConfigDump: "CONFIG_SAVE";
    setAuth: "AUTH_CODE_ENTERED";
    setAuthUser: "AUTH_USER_SELECTED";
    setAuthUsers: "AUTH_LIST_USERS";
    setConfigDump: "CONFIG";
    setCurrentAddChoices: "CONFIG_ADD";
    setLog: "STATS_LOG_DUMP";
    setModActions: "STATS_MODACTIONS_DUMP";
    startPingTimer: "WS_OPEN" | "xstate.stop";
    stopPingTimer: "WS_CLOSE";
    tryAuth: "" | "AUTH_CODE_ENTERED";
    updateConfig: "CONFIG_CHANGED";
  };
  eventsCausingServices: {};
  eventsCausingGuards: {
    authRatelimited: "AUTH_ERROR";
    authSuccess: "AUTH_SUCCESS";
    checkAddCommand: "";
    configNotChanged: "CONFIG_CHANGE_NOTIF";
    configValid: "CONFIG_SAVE";
    hasAuthUsers: "AUTH_CODE_EXPIRED" | "AUTH_FAIL";
    hasLoginData: "";
    hasWindow: "";
  };
  eventsCausingDelays: {};
  matchesStates:
    | "addCommand"
    | "auth"
    | "auth.getListUsers"
    | "auth.init"
    | "auth.inputCode"
    | "auth.inputCode.codeRequest"
    | "auth.inputCode.codeRequest.idle"
    | "auth.inputCode.codeRequest.pending"
    | "auth.inputCode.codeRequest.ready"
    | "auth.inputCode.login"
    | "auth.inputCode.login.failed"
    | "auth.inputCode.login.idle"
    | "auth.inputCode.login.pending"
    | "auth.ratelimited"
    | "auth.selectUser"
    | "auth.success"
    | "auth.tryAuth"
    | "configChangedExternally"
    | "configInvalid"
    | "configSaveFailed"
    | "init"
    | "preAddCommand"
    | "preinit"
    | "ready"
    | "reqConfig"
    | "reqSettings"
    | "reqSettings.config"
    | "reqSettings.config.pending"
    | "reqSettings.config.ready"
    | "reqSettings.schema"
    | "reqSettings.schema.pending"
    | "reqSettings.schema.ready"
    | "saveConfig"
    | "socketClosed"
    | {
        auth?:
          | "getListUsers"
          | "init"
          | "inputCode"
          | "ratelimited"
          | "selectUser"
          | "success"
          | "tryAuth"
          | {
              inputCode?:
                | "codeRequest"
                | "login"
                | {
                    codeRequest?: "idle" | "pending" | "ready";
                    login?: "failed" | "idle" | "pending";
                  };
            };
        reqSettings?:
          | "config"
          | "schema"
          | { config?: "pending" | "ready"; schema?: "pending" | "ready" };
      };
  tags: never;
}
