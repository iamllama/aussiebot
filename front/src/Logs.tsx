import { devMenu, mainService, TMainContext } from "./machine";
import React, { ReactNode, useCallback } from "react";
import { useSelector } from "@xstate/react";
import {
  TableContainer,
  Paper,
  Table,
  TableHead,
  TableRow,
  TableCell,
  TableBody,
  ListItemButton,
  ListItemText,
  Link,
  ListSubheader,
  Tooltip,
  Typography,
} from "@mui/material";
import { TChat, TModActionRow, TPlatformLog, TPlatform } from "./types";
import {
  isChatMetaYt,
  isChatMetaDiscord1,
  isChatMetaDiscord2,
  isChatMetaDiscord3,
} from "./util";

interface ChatRowInnerProps {
  timestamp: number;
  children: ReactNode;
}

const ChatRowInner = (props: ChatRowInnerProps) => {
  return (
    <TableRow
      sx={{
        "&:last-child td, &:last-child th": { border: 0 },
        userSelect: "text",
      }}
    >
      <TableCell component="th" scope="row">
        {new Date(props.timestamp).toLocaleTimeString()}
      </TableCell>
      {props.children}
    </TableRow>
  );
};

interface ChatRowProps {
  timestamp: number;
  chat: TChat;
}

const TableCel = React.memo((props: { children: ReactNode }) => (
  <TableCell
    align="left"
    style={{
      maxWidth: "200px",
    }}
  >
    {props.children}
  </TableCell>
));

const ChatRow = (props: ChatRowProps) => {
  const {
    chat: { user, msg, meta },
    timestamp,
  } = props;

  if (meta) {
    if (isChatMetaDiscord1(meta)) {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const [_chanId, chanName] = meta.Discord1;
      if (chanName === "DMs" && !devMenu()) {
        return null;
      }
      return (
        <ChatRowInner {...{ timestamp }}>
          <TableCel>
            <Tooltip title={user.id}>
              <Typography variant="body1">
                {user.name} <sub>in {chanName}</sub>
              </Typography>
            </Tooltip>
          </TableCel>
          <TableCell align="left">{msg}</TableCell>
        </ChatRowInner>
      );
    }

    if (isChatMetaDiscord2(meta)) {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const [_chanId, chanName, attachments, stickers] = meta.Discord2;
      if (chanName === "DMs" && !devMenu()) {
        return null;
      }
      const att_links = attachments.map(([filename, url]) => (
        <Link
          key={url}
          href={url}
          sx={{ padding: "0 0 0 5px" }}
          underline="hover"
          target="_blank"
          rel="noreferrer"
        >
          {filename}
        </Link>
      ));

      return (
        <ChatRowInner {...{ timestamp }}>
          <TableCel>
            <Tooltip title={user.id}>
              <Typography variant="body1">
                {user.name} <sub>in {chanName}</sub>
              </Typography>
            </Tooltip>
          </TableCel>
          <TableCell align="left">
            {msg} {stickers.length ? `(stickers: ${stickers.join(", ")})` : ""}{" "}
            {att_links}
          </TableCell>
        </ChatRowInner>
      );
    }

    if (isChatMetaDiscord3(meta)) {
      const [attachments, stickers] = meta.Discord3;
      const att_links = attachments.map(([filename, url]) => (
        <Link
          key={url}
          href={url}
          sx={{ padding: "0 0 0 5px" }}
          underline="hover"
          target="_blank"
          rel="noreferrer"
        >
          {filename}
        </Link>
      ));

      return (
        <ChatRowInner {...{ timestamp }}>
          <TableCel>
            <Tooltip title={user.id}>
              <Typography variant="body1">{user.name}</Typography>
            </Tooltip>
          </TableCel>
          <TableCell align="left">
            {msg} {stickers.length ? `(stickers: ${stickers.join(", ")})` : ""}{" "}
            {att_links}
          </TableCell>
        </ChatRowInner>
      );
    }

    if (isChatMetaYt(meta)) {
      if (user.id.startsWith("UCtBkiI649CihbY3MyA-91kA") && !devMenu()) {
        return null;
      }
      const amount = meta.Youtube;
      return (
        <ChatRowInner {...{ timestamp }}>
          <TableCel>
            <Tooltip title={user.id}>
              <Typography variant="body1">
                {user.name} <sub>(💰 {amount})</sub>
              </Typography>
            </Tooltip>
          </TableCel>
          <TableCell align="left">{msg}</TableCell>
        </ChatRowInner>
      );
    }
  }

  if (user.id.startsWith("UCtBkiI649CihbY3MyA-91kA") && !devMenu()) {
    return null;
  }

  return (
    <ChatRowInner {...{ timestamp }}>
      <TableCel>
        <Tooltip title={user.id}>
          <Typography variant="body1">{user.name}</Typography>
        </Tooltip>
      </TableCel>
      <TableCell align="left">{msg}</TableCell>
    </ChatRowInner>
  );
};

export interface LogsProps {
  cursor: TLogCursor;
}

export const Logs: React.FC<LogsProps> = (props) => {
  //console.log("Logs", props.cursor);
  const { type, platform } = props.cursor;
  switch (type) {
    case "Chat":
      return <ChatLogs {...{ platform }} />;
    case "ModActions":
      return <ModActionLogs {...{ platform }} />;
    default:
      return <Typography>Select a log from the menu</Typography>;
  }
};

export interface ChatLogsProps {
  platform: TPlatform;
}

type TTimeChatPair = { timestamp: number; chat: TChat };

const ChatLogs: React.FC<ChatLogsProps> = (props) => {
  // rerender on log change
  //console.log("Logs render");
  const compareCtx = useCallback(
    (prevCtx: TMainContext, nextCtx: TMainContext) =>
      prevCtx.log[props.platform] === nextCtx.log[props.platform],
    [props.platform]
  );
  const context = useSelector(
    mainService,
    (state) => state.context,
    compareCtx
  );

  const log = context.log[props.platform]; // state.context.log[props.platform];

  const platformName = TPlatform[props.platform];

  if (!log || !Object.keys(log).length)
    return (
      <strong>
        No messages currently logged{platformName && ` from ${platformName}`}
      </strong>
    );

  const logItems: TTimeChatPair[] = Object.keys(log)
    .flatMap((ts: string) => {
      const timestamp = parseInt(ts) as keyof TPlatformLog;
      const chat_s = log[timestamp];
      return !Array.isArray(chat_s)
        ? [{ timestamp, chat: chat_s }]
        : chat_s.map((chat) => ({
            timestamp,
            chat,
          }));
    })
    .sort((a, b) => b.timestamp - a.timestamp); // reverse sort

  return (
    <TableContainer component={Paper}>
      <Table stickyHeader aria-label="chat log">
        <TableHead>
          <TableRow>
            <TableCell>Time</TableCell>
            <TableCell align="left">Name</TableCell>
            <TableCell align="left">Message</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {logItems.map(({ timestamp, chat }, i) => (
            <ChatRow key={`${timestamp}${i}`} {...{ chat, timestamp }} />
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
};

interface ModActionRowProps {
  row: TModActionRow;
}

//  key={`mod${platformName}${i}`}
const ModActionRow: React.FC<ModActionRowProps> = (
  props: ModActionRowProps
) => {
  const [name, id, action, reason, timestamp] = props.row;
  const actionString =
    typeof action === "string"
      ? action
      : "Timeout" in action
      ? `Timeout (${action.Timeout}s)`
      : "INVALID";

  return (
    <TableRow
      sx={{
        "&:last-child td, &:last-child th": { border: 0 },
        userSelect: "text",
      }}
    >
      <TableCell component="th" scope="row">
        {new Date(timestamp * 1000).toLocaleString()}
      </TableCell>
      <TableCell>{name ?? "<not captured>"}</TableCell>
      <TableCell>{id}</TableCell>
      <TableCell>{actionString}</TableCell>
      <TableCell>{reason}</TableCell>
    </TableRow>
  );
};

const ModActionLogs: React.FC<ChatLogsProps> = (props) => {
  // rerender on modactions change
  const platform = props.platform;
  const compareCtx = useCallback(
    (prevCtx: TMainContext, nextCtx: TMainContext) =>
      prevCtx.modActions[platform] === nextCtx.modActions[platform],
    [platform]
  );
  const context = useSelector(
    mainService,
    (state) => state.context,
    compareCtx
  );

  const log = context.modActions[platform];
  const platformName = TPlatform[platform];

  if (!log || !Object.keys(log).length)
    return (
      <strong>
        No mod actions currently logged{platformName && ` for ${platformName}`}
      </strong>
    );

  return (
    <TableContainer component={Paper}>
      <Table stickyHeader aria-label="chat log">
        <TableHead>
          <TableRow>
            <TableCell>Time</TableCell>
            <TableCell>Name</TableCell>
            <TableCell>ID</TableCell>
            <TableCell>Action</TableCell>
            <TableCell>Reason</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {log.map((row, i) => {
            return <ModActionRow key={`${platform}${i}`} {...{ row }} />;
          })}
        </TableBody>
      </Table>
    </TableContainer>
  );
};

export type TLogCursor = { type: "Chat" | "ModActions"; platform: TPlatform };

export interface LogsDrawerProps {
  cursor: TLogCursor;
  onSelect: (p: TLogCursor) => void;
}

export const LogsDrawer: React.FC<LogsDrawerProps> = (props) => {
  // rerender on log/modactions change
  //console.log("LogsDrawer render", props.cursor);
  const compareCtx = useCallback(
    (prevCtx: TMainContext, nextCtx: TMainContext) =>
      prevCtx.log === nextCtx.log && prevCtx.modActions === nextCtx.modActions,
    []
  );
  const context = useSelector(
    mainService,
    (state) => state.context,
    compareCtx
  );

  const cursor = props.cursor;
  const logs = context.log;
  const modActions = context.modActions;

  return (
    <div style={{ flexGrow: "0" }}>
      <ListSubheader>Chat</ListSubheader>
      {Object.keys(logs).map((_platform) => {
        const platform = _platform as unknown as TPlatform;
        const log = logs[platform];
        const name = TPlatform[platform];
        return (
          <ListItemButton
            key={`chat${platform}`}
            selected={cursor.type === "Chat" && cursor.platform === platform}
            onClick={() => props.onSelect({ type: "Chat", platform })}
            disabled={Object.keys(log).length === 0}
          >
            <ListItemText primary={name} />
          </ListItemButton>
        );
      })}
      <ListSubheader>Mod actions</ListSubheader>
      {Object.keys(modActions).map((_platform) => {
        const platform = _platform as unknown as TPlatform;
        const log = modActions[platform as unknown as keyof typeof modActions];
        const name = TPlatform[platform];
        return (
          <ListItemButton
            key={`mod${platform}`}
            selected={
              cursor.type === "ModActions" && cursor.platform === platform
            }
            onClick={() => props.onSelect({ type: "ModActions", platform })}
            disabled={Object.keys(log).length === 0}
          >
            <ListItemText primary={name} />
          </ListItemButton>
        );
      })}
    </div>
  );
};
