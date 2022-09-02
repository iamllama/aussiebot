import {
  TBoolValue,
  TChat,
  TChatMetaDiscord1,
  TChatMetaDiscord2,
  TChatMetaDiscord3,
  TChatMetaYt,
  TCmdConfig,
  TCmdSchema,
  TConfig,
  TConfigSet,
  TConfigSetDump,
  TConstraint,
  TEnum,
  TFns,
  TMaybeValidValue,
  TModActionValue,
  TNumberValue,
  TPermsValue,
  TPlatformValue,
  TRegexValue,
  TSchema,
  TStringValue,
  TValue,
} from "./types";

export const __DEV__ = process.env.NODE_ENV === "development";

export const _debug = __DEV__ ? console.log : () => null;

const isBoolValue = (arg: object): arg is TBoolValue => "Bool" in arg;
const isNumberValue = (arg: object): arg is TNumberValue => "Number" in arg;
const isStringValue = (arg: object): arg is TStringValue => "String" in arg;
const isRegexValue = (arg: object): arg is TRegexValue => "Regex" in arg;
const isPlatformValue = (arg: object): arg is TPlatformValue =>
  "Platforms" in arg;
const isPermissionsValue = (arg: object): arg is TPermsValue =>
  "Permissions" in arg;
const isModActionValue = (arg: object): arg is TModActionValue =>
  "ModAction" in arg;

export const strip_maybe_value = ({
  valid,
  ...value
}: TMaybeValidValue): TValue => value;

export function parse_schema(dump: Readonly<TCmdSchema[]>): TSchema {
  const res = dump.reduce(
    (acc, [cmd_type, desc, configType, keys]) => ({
      ...acc,
      [cmd_type]: {
        desc,
        configType,
        fields: keys.reduce(
          (acc, [field, desc, def_value, constraint]) => ({
            ...acc,
            [field]: {
              desc,
              def_value,
              constraint,
            },
          }),
          {}
        ),
      },
    }),
    {} as TSchema
  );
  _debug("parse_schema", dump, res);
  return res;
}

export function parse_config(dump: TCmdConfig[], schema: TSchema): TConfig[] {
  if (!check_config_types(dump, schema)) return [];
  return dump.map(([type, name, keys]) => ({
    type,
    name,
    fields: Object.fromEntries(
      keys.map(([field, value]) => [
        field,
        into_maybe_valid_value(value, schema[type].fields[field].constraint),
      ])
    ),
  }));
}

export function parse_config_set(
  dump: TConfigSetDump,
  schema: TSchema
): TConfigSet {
  return {
    commands: parse_config(dump["commands"], schema),
    filters: parse_config(dump["filters"], schema),
    timers: parse_config(dump["timers"], schema),
  };
}

export function dump_config(config: Readonly<TConfig[]>): TCmdConfig[] {
  return config.map(({ type, name, fields }) => [
    type,
    name,
    // TMaybeValidValue to TValue
    Object.entries(fields).map(([field, mv_value]) => [
      field,
      strip_maybe_value(mv_value),
    ]),
  ]);
}

export const into_maybe_valid_value = (
  value: TValue,
  constraint: TConstraint
): TMaybeValidValue => ({
  ...value,
  valid: verify_value(value, constraint),
});

function same_type_values(v: TValue, def: TValue) {
  const fns: TFns<boolean> = {
    bool: () => isBoolValue(def),
    number: () => isNumberValue(def),
    string: () => isStringValue(def),
    regex: () => isRegexValue(def),
    platform: () => isPlatformValue(def),
    perms: () => isPermissionsValue(def),
    modaction: () => isModActionValue(def),
    default: () => false,
  };
  return map_value(v, fns);
}

function check_config_types(dump: TCmdConfig[], schema: TSchema) {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  for (const [type, name, keys] of dump) {
    for (const [field, value] of keys) {
      const defValue = schema[type].fields[field].def_value;
      if (!same_type_values(value, defValue)) {
        console.error(
          "invalid Value type, got: typeof",
          value,
          "expected: typeof",
          defValue
        );
        return false;
      }
    }
  }
  return true;
}

/** check if all fields are valid */
export function is_valid_config(config: Readonly<TConfig>): boolean {
  //config.reduce((valid, cmd) => chmodSync., true);
  return Object.keys(config.fields).reduce(
    (valid: boolean, field) => valid && config.fields[field].valid,
    true
  );
}

/** check if all fields of all commands are valid */
export function is_valid_configs(config: Readonly<TConfig[]>): boolean {
  return config.reduce(
    (valid: boolean, cmd) => valid && is_valid_config(cmd),
    true
  );
}

export const configSetIsValid = (config: TConfigSet) =>
  Object.values(config)
    .map(is_valid_configs)
    .reduce((acc, valid) => acc && valid, true);

export function verify_value(value: TValue, constraint: TConstraint): boolean {
  const def = (value: TValue) => verify_def(value, constraint);
  const fn_list: TFns<boolean> = {
    bool: (value) => verify_bool(value, constraint),
    number: (value) => verify_number(value, constraint),
    string: (value) => verify_string(value, constraint),
    regex: (value) => verify_regex(value, constraint),
    platform: def,
    perms: def,
    modaction: (value) => verify_modaction(value, constraint),
    default: () => false,
  };

  return map_value(value, fn_list);
}

function verify_bool(value: TBoolValue, constraint: TConstraint) {
  switch (constraint) {
    case "None":
      return true;
    case "Positive":
      return value.Bool === true;
    case "Negative":
      return value.Bool === false;
    default:
      return false;
  }
}

export function verify_number(value: TNumberValue, constraint: TConstraint) {
  const num = value.Number;

  if (typeof num !== "number") {
    return false;
  }

  // parseInt returns NaN if not a num
  if (isNaN(num)) {
    return false;
  }

  if (typeof constraint === "string")
    switch (constraint) {
      case "None":
        return true;
      case "Positive":
        return num >= 0;
      case "Negative":
        return num < 0;
      default:
        return false;
    }

  if ("RangeClosed" in constraint) {
    const { start, end } = constraint.RangeClosed;
    const left = start === null || start <= num;
    const right = end === null || num <= end;
    return left && right;
  } else if ("RangeHalfOpen" in constraint) {
    const { start, end } = constraint.RangeHalfOpen;
    const left = start === null || start <= num;
    const right = end === null || num < end;
    return left && right;
  }

  return false;
}

export function verify_string(value: TStringValue, constraint: TConstraint) {
  const len = value.String.length;

  if (typeof constraint === "string")
    switch (constraint) {
      case "None":
        return true;
      case "NonEmpty":
        return len > 0;
      default:
        return false;
    }

  if ("RangeClosed" in constraint) {
    const { start, end } = constraint.RangeClosed;
    const left = start === null || start <= len;
    const right = end === null || len <= end;
    return left && right;
  } else if ("RangeHalfOpen" in constraint) {
    const { start, end } = constraint.RangeHalfOpen;
    const left = start === null || start <= len;
    const right = end === null || len < end;
    return left && right;
  }

  return false;
}

export function verify_regex(value: TRegexValue, constraint: TConstraint) {
  // check if regex is valid
  const pat = value.Regex;
  try {
    if (pat.startsWith("(?i)")) {
      new RegExp(pat.substring(4));
    } else {
      new RegExp(pat);
    }
  } catch (e) {
    return false;
  }

  switch (constraint) {
    case "None":
      return true;
    case "NonEmpty":
      return value.Regex.length > 0;
    default:
      return false;
  }
}

export function verify_modaction(
  value: TModActionValue,
  constraint: TConstraint
) {
  const action = value.ModAction;

  if (typeof action === "string") {
    return true;
  }

  if (typeof constraint === "string")
    switch (constraint) {
      case "None":
        return true;
      default:
        return false;
    }

  if ("Timeout" in action) {
    const t = action.Timeout;

    if ("RangeClosed" in constraint) {
      const { start, end } = constraint.RangeClosed;
      const left = start === null || start <= t;
      const right = end === null || t <= end;
      return left && right;
    } else if ("RangeHalfOpen" in constraint) {
      const { start, end } = constraint.RangeHalfOpen;
      const left = start === null || start <= t;
      const right = end === null || t < end;
      return left && right;
    }
  }

  return false;
}

/** default constraint check for value types with no constraints */
function verify_def(value: TValue, constraint: TConstraint) {
  switch (constraint) {
    case "None":
      return true;
    default:
      return false;
  }
}

export function map_value<T>(value: TValue, fns: TFns<T>) {
  if (isBoolValue(value)) {
    return fns.bool(value);
  }
  if (isNumberValue(value)) {
    return fns.number(value);
  }
  if (isStringValue(value)) {
    return fns.string(value);
  }
  if (isRegexValue(value)) {
    return fns.regex(value);
  }
  if (isPlatformValue(value)) {
    return fns.platform(value);
  }
  if (isPermissionsValue(value)) {
    return fns.perms(value);
  }
  if (isModActionValue(value)) {
    return fns.modaction(value);
  }
  // return default otherwise (unreachable unless a case was missing)
  return fns.default(value);
}

export function bitsToBitList<T extends TEnum>(b: number, enm: T): number[] {
  const list = [];
  for (const p in enm) {
    const v = enm[p];
    if (typeof v === "number" && b & v) {
      list.push(v);
    }
  }
  return list;
}

export const chatHasMeta = (chat: TChat): boolean => "meta" in chat;
export const isChatMetaYt = (meta: object): meta is TChatMetaYt =>
  "Youtube" in meta;
export const isChatMetaDiscord1 = (meta: object): meta is TChatMetaDiscord1 =>
  "Discord1" in meta;
export const isChatMetaDiscord2 = (meta: object): meta is TChatMetaDiscord2 =>
  "Discord2" in meta;
export const isChatMetaDiscord3 = (meta: object): meta is TChatMetaDiscord3 =>
  "Discord3" in meta;
