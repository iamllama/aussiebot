import {
  Checkbox,
  IconButton,
  TextField,
  Typography,
  Select,
  MenuItem,
  FormControl,
  InputLabel,
  Tooltip,
  OutlinedInput,
  ListItemText,
  SelectChangeEvent,
  Button,
  TableRow,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
} from "@mui/material";
import { FactCheck } from "@mui/icons-material";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  TConstraint,
  TSchema,
  TBoolValue,
  TNumberValue,
  TStringValue,
  TRegexValue,
  TPlatformValue,
  TPermsValue,
  TModActionValue,
  TValue,
  TConfig,
  TFns,
  ModActions,
} from "./types";
import { TPlatform, TModAction, TPerms, Permissions, Platforms } from "./types";
import {
  into_maybe_valid_value,
  map_value,
  bitsToBitList,
  verify_string,
  verify_regex,
  verify_number,
  verify_value,
  verify_modaction,
} from "./util";

// delay before committing if no further changes made (in ms)
const EDIT_COMMIT_DELAY = 500;

function desc_constraint(constraint: TConstraint) {
  if (typeof constraint === "string")
    switch (constraint) {
      case "Positive":
        return "Must be positive";
      case "Negative":
        return "Must be negative";
      case "NonEmpty":
        return "Must be filled";
      default:
        return "";
    }

  if ("RangeClosed" in constraint) {
    const { start, end } = constraint.RangeClosed;
    if (start && end) {
      return `Must be between ${start} and ${end} (inclusive)`;
    } else if (start) {
      return `Must be at least ${start}`;
    } else if (end) {
      return `Must be at most ${end}`;
    }
  }

  if ("RangeHalfOpen" in constraint) {
    const { start, end } = constraint.RangeHalfOpen;
    if (start && end) {
      return `Must be between ${start} and ${end} (non-inclusive)`;
    } else if (start) {
      return `Must be at least ${start}`;
    } else if (end) {
      return `Must be at most ${end}`;
    }
  }

  if ("Timeout" in constraint) {
    const [min, max] = constraint.Timeout;
    if (min && max) {
      return `Timeout must be between ${min} and ${max}`;
    } else if (min) {
      return `Timeout must be at least ${min}`;
    } else if (max) {
      return `Timeout must be at most ${max}`;
    }
  }
  return "";
}

const fromSV = (v: TStringValue): string => v.String;
const toSV = (String: string): TStringValue => ({ String });
const fromBV = (v: TBoolValue): boolean => v.Bool;
const toBV = (Bool: boolean): TBoolValue => ({ Bool });
const fromRV = (v: TRegexValue): string => v.Regex;
const toRV = (Regex: string): TRegexValue => ({ Regex });
const fromNV = (v: TNumberValue): number | string => v.Number;
const toNV = (Number: number | string): TNumberValue => ({ Number });
const fromPlatV = (v: TPlatformValue): TPlatform => v.Platforms;
const toPlatV = (Platforms: TPlatform): TPlatformValue => ({ Platforms });
const fromPermV = (v: TPermsValue): TPerms => v.Permissions;
const toPermV = (Permissions: TPerms): TPermsValue => ({ Permissions });
const fromMV = (v: TModActionValue): TModAction => v.ModAction;
const toMV = (ModAction: TModAction): TModActionValue => ({ ModAction });

interface ConfigProps {
  schema: TSchema;
  config: TConfig;
  index: number;
  onChange: (c: TConfig) => void;
}

const Config = (props: ConfigProps) => {
  const cmd = props.config;
  const schema = props.schema;
  const index = props.index;
  const cmd_desc = schema[cmd.type].desc;

  return (
    <div style={{ flexDirection: "column" }}>
      <div style={{ padding: "10px 0 10px 10px" }}>
        <Typography variant="h6">{cmd_desc}</Typography>
      </div>
      <div style={{ padding: "10px 0 10px 10px" }}>
        <Name
          key={`${cmd.type}${index}`} // without this it doesn't rerender monkaW
          name={cmd.name}
          onUpdate={(name) =>
            props.onChange({
              ...cmd,
              name,
            })
          }
        />
      </div>
      <Fields>
        {Object.keys(cmd.fields).map((field) => {
          const value = cmd.fields[field];
          const constraint = schema[cmd.type].fields[field].constraint;
          const helperText = desc_constraint(constraint);
          const label = schema[cmd.type].fields[field].desc;
          return (
            <Field
              key={`${cmd.type}${index}${field}`}
              valid={value.valid}
              {...{ field, value, helperText, label, constraint }}
              onUpdate={(v) => {
                props.onChange({
                  ...cmd,
                  fields: {
                    ...cmd.fields,
                    [field]: into_maybe_valid_value(v, constraint), // we alr check constrsaints in complex fields, so change v to TMaybeValidValue
                  },
                });
              }}
            />
          );
        })}
      </Fields>
    </div>
  );
};

function commitCallback<T extends string, U>(
  toV: (a: T) => U,
  timer: React.MutableRefObject<NodeJS.Timeout | null>,
  setValue: (v: U) => void,
  onUpdate: (v: U) => void
) {
  return (e: React.ChangeEvent<HTMLInputElement>) => {
    if (timer?.current) clearTimeout(timer.current);
    const value = toV(e.target.value as T);
    setValue(value);
    timer.current = setTimeout(() => onUpdate(value), EDIT_COMMIT_DELAY);
  };
}

interface NameProps {
  name: string;
  onUpdate: (v: string) => void;
}

const Name = (props: NameProps) => {
  const [name, setName] = useState(props.name);
  const { onUpdate } = props;

  useEffect(() => {
    setName(props.name);
  }, [props.name]);

  const timer = useRef(null as NodeJS.Timeout | null);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const onChange = useCallback(
    commitCallback((a: string) => a, timer, setName, onUpdate),
    [setName, onUpdate]
  );

  return (
    <TextField
      label="Name (optional)"
      value={name}
      error={false} // enforce unique type-name pair?
      onChange={onChange}
    />
  );
};

interface FieldProps<T extends TValue> {
  label: string;
  field: string;
  value: T;
  valid?: boolean;
  helperText?: string;
  onUpdate: (v: T) => void;
  constraint: TConstraint;
}

interface FieldsProp {
  children: React.ReactNode;
}

const Fields = (props: FieldsProp) => {
  return (
    <TableContainer component={Paper} sx={{ maxWidth: 600 }}>
      <Table aria-label="settings">
        <TableBody>{props.children}</TableBody>
      </Table>
    </TableContainer>
  );
};

interface FieldBoxProp {
  label: string;
  children: React.ReactNode;
}

const FieldBox = (props: FieldBoxProp) => {
  return (
    <TableRow>
      <TableCell align="right">
        {/* <Tooltip arrow={false} title={props.label}> */}
        <Typography variant="body1">{props.label}:</Typography>
        {/* </Tooltip> */}
      </TableCell>
      <TableCell align="left">
        <div
          style={{
            display: "flex",
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          {props.children}
        </div>
      </TableCell>
    </TableRow>
  );
};

const Field = (props: FieldProps<TValue>) => {
  const { value } = props;

  const fn_list: TFns<JSX.Element> = useMemo(
    () => ({
      bool: (value) => BoolField({ ...props, value }),
      number: (value) => NumberField({ ...props, value }),
      string: (value) => StringField({ ...props, value }),
      regex: (value) => RegexField({ ...props, value }),
      platform: (value) => PlatformField({ ...props, value }),
      perms: (value) => PermissionsField({ ...props, value }),
      modaction: (value) => ModActionField({ ...props, value }),
      default: () => <div>Unreachable: unknown value</div>,
    }),
    [props]
  );

  return map_value(value, fn_list);
};

const BoolField = (props: FieldProps<TBoolValue>) => {
  const value = fromBV(props.value);
  const { onUpdate } = props;
  const onClick = useCallback(() => onUpdate(toBV(!value)), [value, onUpdate]);
  return (
    <FieldBox label={props.label}>
      <Button
        variant="contained"
        style={{ backgroundColor: value ? "green" : "red" }}
        sx={{ color: "text.primary" }}
        {...{ onClick }}
      >
        {value ? "ON" : "OFF"}
      </Button>
    </FieldBox>
  );
};

const toNumber = (nStr: string) => {
  const n = parseInt(nStr);
  // allow partial numbers
  const numOrStr = isNaN(n) || n.toString() !== nStr ? nStr : n; // parseInt truncates invalid numbers, so check string repr
  const nv = toNV(numOrStr);
  return nv;
};

const NumberField = (props: FieldProps<TNumberValue>) => {
  const [value, setValue] = useState(props.value);
  const onUpdate = props.onUpdate;

  useEffect(() => {
    setValue(props.value);
  }, [props.value]);

  const valid = verify_number(value, props.constraint);

  const timer = useRef(null as NodeJS.Timeout | null);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const onChange = useCallback(
    commitCallback(toNumber, timer, setValue, onUpdate),
    [setValue, onUpdate]
  );

  return (
    <FieldBox label={props.label}>
      <TextField
        value={fromNV(value)}
        error={!valid}
        label="Number"
        type="number"
        {...{ onChange }}
        helperText={valid ? "" : props.helperText || "Invalid number"}
      />
    </FieldBox>
  );
};

const StringField = (props: FieldProps<TStringValue>) => {
  const [value, setValue] = useState(props.value);
  const onUpdate = props.onUpdate;

  // update state if prop changes
  useEffect(() => {
    setValue(props.value);
  }, [props.value]);

  const valid = verify_string(value, props.constraint);

  const timer = useRef(null as NodeJS.Timeout | null);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const onChange = useCallback(
    commitCallback(toSV, timer, setValue, onUpdate),
    [setValue, onUpdate]
  );

  return (
    <FieldBox label={props.label}>
      <TextField
        value={fromSV(value)}
        error={!valid}
        onChange={onChange}
        helperText={valid ? "" : props.helperText}
      />
    </FieldBox>
  );
};

const RegexField = (props: FieldProps<TRegexValue>) => {
  const [value, setValue] = useState(props.value);
  const { onUpdate } = props;

  useEffect(() => {
    setValue(props.value);
  }, [props.value]);

  const regex = fromRV(value);
  const valid = verify_regex(value, props.constraint);

  const timer = useRef(null as NodeJS.Timeout | null);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const onChange = useCallback(
    commitCallback(toRV, timer, setValue, onUpdate),
    [setValue, onUpdate]
  );

  return (
    <FieldBox label={props.label}>
      <TextField
        label="Regex"
        value={regex}
        error={!valid}
        onChange={onChange}
        helperText={valid ? "" : props.helperText || "Invalid regex pattern"}
      />
      <Tooltip title="Check regex">
        <span>
          <IconButton
            aria-label="test-regex"
            onClick={() =>
              regex.length &&
              valid &&
              window.open(
                `https://regex101.com/?regex=${encodeURI(regex)}&options=g`
              )
            }
            disabled={!regex.length || !valid}
          >
            <FactCheck />
          </IconButton>
        </span>
      </Tooltip>
    </FieldBox>
  );
};

const ITEM_HEIGHT = 48;
const ITEM_PADDING_TOP = 8;
const MenuProps = {
  PaperProps: {
    style: {
      maxHeight: ITEM_HEIGHT * 5 + ITEM_PADDING_TOP,
      width: 200,
    },
  },
};

const PlatformField = (props: FieldProps<TPlatformValue>) => {
  const [value, setValue] = useState(props.value);

  useEffect(() => {
    setValue(props.value);
  }, [props.value]);

  const platformBits = fromPlatV(value);
  const platformBitList = bitsToBitList(platformBits, TPlatform);
  const onChange = (e: SelectChangeEvent<TPlatform[]>) => {
    const {
      target: { value },
    } = e;
    const list =
      typeof value === "string" ? value.split(",").map(parseInt) : value;
    const bits = list.reduce((b, x) => b | x, 0); // just sum list?\
    setValue(toPlatV(bits));
  };
  const valid = verify_value(value, props.constraint);
  const changed = value !== props.value;

  return (
    <FieldBox label={props.label}>
      <FormControl fullWidth>
        <InputLabel>
          Platform{platformBitList.length !== 1 ? "s" : ""}
        </InputLabel>
        <Select
          multiple
          value={platformBitList}
          onChange={onChange}
          error={!valid}
          onClose={() => changed && props.onUpdate(value)}
          input={<OutlinedInput label="Platforms" />}
          renderValue={(bl) => bl.map((p) => TPlatform[p]).join(", ")}
          MenuProps={MenuProps}
        >
          {Platforms.map((platform) => (
            <MenuItem key={platform} value={TPlatform[platform]}>
              <Checkbox checked={!!(platformBits & TPlatform[platform])} />
              <ListItemText primary={platform} />
            </MenuItem>
          ))}
        </Select>
      </FormControl>
    </FieldBox>
  );
};

const PermissionsField = (props: FieldProps<TPermsValue>) => {
  const perm = fromPermV(props.value);

  const onChange = (e: SelectChangeEvent) => {
    const p = TPerms[e.target.value as keyof typeof TPerms];
    props.onUpdate(toPermV(p));
  };

  return (
    <FieldBox label={props.label}>
      <FormControl fullWidth>
        <InputLabel>Permissions</InputLabel>
        <Select value={TPerms[perm]} label="Permissions" {...{ onChange }}>
          {Permissions.map((perm) => (
            <MenuItem key={perm} value={perm}>
              {perm}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
    </FieldBox>
  );
};

const ModActionField = (props: FieldProps<TModActionValue>) => {
  const [value, setValue] = useState(props.value);
  const onUpdate = props.onUpdate;

  useEffect(() => {
    setValue(props.value);
  }, [props.value]);

  const action = fromMV(value);
  const valid = verify_modaction(value, props.constraint);

  const timer = useRef(null as NodeJS.Timeout | null);
  const onChange = useCallback(
    function (v: TModAction) {
      if (timer?.current) clearTimeout(timer.current);
      const value = toMV(v);
      setValue(value);
      timer.current = setTimeout(() => onUpdate(value), EDIT_COMMIT_DELAY);
    },
    [setValue, onUpdate]
  );

  const isTimeout = typeof action !== "string" && "Timeout" in action;
  const Timeout = isTimeout ? action.Timeout : 300; // TODO: define default somewhere
  const actionValue = isTimeout ? "Timeout" : action;

  return (
    <FieldBox label={props.label}>
      <div style={{ display: "flex", flexDirection: "column" }}>
        <FormControl>
          <InputLabel>Action</InputLabel>
          <Select
            value={actionValue}
            label="Action"
            onChange={(e) =>
              e.target.value !== "Timeout"
                ? onChange(e.target.value as TModAction)
                : onChange({ Timeout })
            }
          >
            {ModActions.map((action) => (
              <MenuItem key={action} value={action}>
                {action}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        {isTimeout && (
          <>
            <div style={{ padding: "5px" }} />
            <TextField
              value={Timeout.toString()}
              label="Duration (s)"
              type="number"
              error={!valid}
              helperText={valid ? "" : props.helperText || "Invalid timeout"}
              onChange={(e) => onChange({ Timeout: parseInt(e.target.value) })}
            />
          </>
        )}
      </div>
    </FieldBox>
  );
};

export default Config;
