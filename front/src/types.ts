export enum Constraint {
  None,
  NonEmpty,
  Positive,
  Negative,
}

export type TCnstrRangeClosed = {
  RangeClosed: { start: number; end: number };
};

export type TCnstRangeHalfOpen = {
  RangeHalfOpen: { start: number; end: number };
};

export type TCnstrTimeout = {
  Timeout: [number, number] | [null, number] | [number, null];
};

export type TConstraint =
  | keyof typeof Constraint
  | TCnstrRangeClosed
  | TCnstRangeHalfOpen
  | TCnstrTimeout;

export type TModAction =
  | "None"
  | "Warn"
  | "Remove"
  | { Timeout: number }
  | "Kick"
  | "Ban";

export const ModActions = ["None", "Warn", "Remove", "Timeout", "Kick", "Ban"];

export enum TPlatform {
  Youtube = 1 << 0,
  Twitch = 1 << 1,
  Discord = 1 << 2,
  Web = 1 << 3,
}

export const ChatPlatforms: TPlatform =
  TPlatform.Youtube | TPlatform.Discord | TPlatform.Twitch;

export enum TPerms {
  Viewer = 1 << 0,
  Member = 1 << 1,
  Mod = 1 << 2,
  Admin = 1 << 3,
  Owner = 1 << 4,
}

export type Enum<E> = Record<keyof E, number | string> & {
  [k: number]: string;
};
export type TEnum = { [k: string | number]: string | number };

export function enum_keys<T extends TEnum>(v: T): string[] {
  return Object.keys(v).reduce((arr: (T | keyof T)[], key: keyof T) => {
    if (!arr.includes(key)) {
      arr.push(v[key]);
    }
    return arr;
  }, []) as string[];
}

export const Platforms = enum_keys(TPlatform) as (keyof typeof TPlatform)[];
//export const ModActions = enum_keys(TModAction) as (keyof typeof TModAction)[];
export const Permissions = enum_keys(TPerms) as (keyof typeof TPerms)[];

export type TBoolValue = { Bool: boolean };
export type TNumberValue = { Number: number | string };
export type TStringValue = { String: string };
export type TRegexValue = { Regex: string };
export type TPlatformValue = { Platforms: TPlatform };
export type TPermsValue = { Permissions: TPerms };
export type TModActionValue = { ModAction: TModAction };
export type TValue =
  | TBoolValue
  | TNumberValue
  | TStringValue
  | TRegexValue
  | TPlatformValue
  | TPermsValue
  | TModActionValue;

export type TMaybeValidValue = TValue & { valid: boolean };

export type TFns<U> = {
  bool: (v: TBoolValue) => U;
  number: (v: TNumberValue) => U;
  string: (v: TStringValue) => U;
  regex: (v: TRegexValue) => U;
  platform: (v: TPlatformValue) => U;
  perms: (v: TPermsValue) => U;
  modaction: (v: TModActionValue) => U;
  default: (v: TValue) => U; //default value
};

export type TKeyCmd = [string, TValue];
export type TCmdConfig = [string, string, TKeyCmd[]];

export type TConfig = {
  readonly type: string;
  name: string;
  fields: { [k: string]: TMaybeValidValue };
};

export enum TConfigType {
  Command = "commands",
  Filter = "filters",
  Timer = "timers",
}

export type TConfigTypeKey = keyof typeof TConfigType;
export const ConfigTypeValues = Object.values(TConfigType);

export type TConfigSetDump = {
  [k in TConfigType]: TCmdConfig[];
};

export type TConfigSet = {
  [k in TConfigType]: TConfig[];
};

export type TKeySchema = [string, string, TValue, TConstraint];
export type TCmdSchema = [string, string, TConfigTypeKey, TKeySchema[]];

export type TSchema = {
  [cmd_type: string]: {
    desc: string;
    configType: TConfigTypeKey;
    fields: {
      [k: string]: {
        desc: string;
        def_value: TValue;
        constraint: TConstraint;
      };
    };
  };
};

export type TConfigDumpPayload = {
  ConfigDump: TConfigSetDump;
};

export type TMessagePayload = {
  Message: {
    id: string;
    name: string;
    msg: string;
  };
};

export type TStreamEventPayload = {
  StreamEvent: { Detected: string } | { Started: string } | { Stopped: string };
};

export type TStreamSignalPayload = {
  StreamSignal: { Start: string } | { Stop: string };
};

export type TChatUser = {
  id: string;
  name: string;
  perms: TPerms;
};

export type TChatMetaYt = { Youtube: string };

export type TChatMetaDiscord1 = {
  Discord1: [number, string];
};
export type TChatMetaDiscord2 = {
  Discord2: [number, string, [string, string][], string[]];
};
export type TChatMetaDiscord3 = {
  Discord3: [[string, string][], string[]];
};

export type TChatMeta =
  | TChatMetaYt
  | TChatMetaDiscord1
  | TChatMetaDiscord2
  | TChatMetaDiscord3;

export type TChat = {
  user: TChatUser;
  msg: string;
  meta?: TChatMeta;
};

export type TChatPayload = {
  Chat: TChat;
};

export type TDumpLogPayload = {
  DumpLog: TPlatform;
};

export type TPlatformLogDump = string[];

export type TLogDumpPayload = {
  LogDump: [TPlatform, TPlatformLogDump][];
};

export type TModActionItem = [TChatUser, TModAction, string]; // user, action, reason // TODO: TModAction
export type TModActionPayload = {
  ModAction: TModActionItem;
};
export type TModActionRow = [string | null, string, TModAction, string, number]; // name, id, action, reason, at
export type TPlatformModActionsDump = TModActionRow[];
export type TModActionsDumpPayload = {
  ModActionsDump: [TPlatform, TPlatformModActionsDump][];
};

export type TDumpArgsPayload = {
  DumpArgs: TPlatform;
};

export type TArgKind =
  | "String"
  | { Integer: { min?: number; max?: number } }
  | "Bool"
  | "User"
  | "Platform";
export type TArg = {
  kind: TArgKind;
  optional: boolean;
  name: string;
  desc: string;
};
export type TArgsDumpItem = [string, string, TPerms, TArg[]]; // prefix, desc, perms, args
export type TArgsDumpPayload = {
  ArgsDump: TArgsDumpItem[];
};

export type TSchemaDump = { SchemaDump: TCmdSchema[] };

export type TPayload =
  | "DumpConfig"
  | "DumpSchema"
  | "DumpModActions"
  | "ConfigChanged"
  | "ConfigSaved"
  | TConfigDumpPayload
  | TSchemaDump
  | TMessagePayload
  | TStreamSignalPayload
  | TStreamEventPayload
  | TChatPayload
  | TDumpLogPayload
  | TLogDumpPayload
  | TModActionPayload
  | TModActionsDumpPayload
  | TDumpArgsPayload
  | TArgsDumpPayload;

export type TMessage = {
  platform: TPlatform;
  channel: string;
  payload: TPayload;
};

export type TConfigCursor = {
  type: TConfigType;
  index: number;
};

export type TPlatformLog = {
  [timestamp: number]: TChat | TChat[];
};

export type TLog = {
  [platform in TPlatform]: TPlatformLog;
};

export type TPlatformModActionsLog = TModActionRow[];

export type TModActionsLog = {
  [platform in TPlatform]: TPlatformModActionsLog;
};

/*
    ListUsers,
    RequestCode(Arc<String>),
    Login(Arc<String>, Arc<String>),
*/
export type TAuthListUsers = "ListUsers";
export type TAuthRequestCode = { RequestCode: string };
export type TAuthLogin = { Login: [string, string] };

export type TAuthMessage = TAuthListUsers | TAuthRequestCode | TAuthLogin;

/*
    Users(Vec<String>),
    InvalidUser,
    CodeReady,
    AuthSuccess(Arc<String>),
    AuthFail,
    AuthError,
*/

export type TAuthUsers = { Users: string[] };
export type TAuthInvalidUser = "InvalidUser";
export type TAuthCodeReady = "CodeReady";
export type TAuthCodeExpired = "CodeExpired";

export type TAuthSuccess = { AuthSuccess: string };
export type TAuthFail = "AuthFail";

export type TAuthErrorType = "Ratelimited" | "ServerError";
export type TAuthError = { AuthError: TAuthErrorType };

export type TAuthResp =
  | TAuthUsers
  | TAuthInvalidUser
  | TAuthCodeReady
  | TAuthCodeExpired
  | TAuthSuccess
  | TAuthFail
  | TAuthError;
