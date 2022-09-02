import Config from "./Config";
import { Logs, LogsDrawer, TLogCursor } from "./Logs";
import { Auth } from "./Auth";
import { mainService, emit, TMainContext } from "./machine";
import { useSelector } from "@xstate/react";
import {
  AppBar,
  Avatar,
  Box,
  Button,
  CircularProgress,
  Collapse,
  Dialog,
  Divider,
  Drawer,
  IconButton,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Slide,
  Toolbar,
  Typography,
  useMediaQuery,
  useScrollTrigger,
  Badge,
} from "@mui/material";
import { styled } from "@mui/material/styles";
import MenuIcon from "@mui/icons-material/Menu";
import DeleteIcon from "@mui/icons-material/Delete";
import AddCircleOutlineIcon from "@mui/icons-material/AddCircleOutline";
import ErrorOutlineIcon from "@mui/icons-material/ErrorOutline";
import React, { ReactElement, ReactNode, useCallback, useState } from "react";
import { TBoolValue, TConfigType, TPlatform, TSchema } from "./types";
import { ExpandLess, ExpandMore } from "@mui/icons-material";
import type { TConfigCursor } from "./types";
import { is_valid_config } from "./util";

interface LoadingProps {
  msg?: string;
}

const Loading = React.memo(function Loading(props: LoadingProps) {
  return (
    <div className="center">
      <Typography variant="h5">{props.msg ?? "Loading..."}</Typography>
      <CircularProgress className="mt10 mb10" />
    </div>
  );
});

// interface AddCommandProps {
//   // open: boolean;
//   // type: TConfigType;
//   // choices: string[];
//   // onSelect: (t: string) => void;
//   // onClose: () => void;
// }

const AddCommand = () => {
  const isAddCommand = useSelector(mainService, (state) =>
    state.matches("addCommand")
  );

  const context = mainService.state.context;
  const schema = context.schema;
  const currentAdd = context.currentAdd;
  const choices = context.currentAddChoices;

  const onClose = useCallback(() => emit("CONFIG_CANCEL_ADD"), []);

  return (
    <Dialog open={isAddCommand} onClose={onClose}>
      <div className="center m10" style={{ maxWidth: "300px" }}>
        <Typography variant="h6" className="p10">
          Add to {currentAdd}
        </Typography>
        <List style={{ maxHeight: "300px", overflow: "auto" }} disablePadding>
          {choices.map((choice) => (
            <ListItem key={choice} disablePadding>
              <ListItemButton
                onClick={() =>
                  emit({ type: "CONFIG_ADD_CMD", cmdName: choice })
                }
              >
                <ListItemText
                  primary={choice}
                  secondary={schema[choice as keyof TSchema].desc}
                />
              </ListItemButton>
            </ListItem>
          ))}
        </List>
      </div>
      <Button onClick={onClose}>Close</Button>
    </Dialog>
  );
};

interface ConfigChangedExtProps {
  open: boolean;
  onAccept: () => void;
  onReject: () => void;
}

const ConfigChangedExt = React.memo(function ConfigChangedExt(
  props: ConfigChangedExtProps
) {
  return (
    <Dialog open={props.open} aria-labelledby="" aria-describedby="">
      <div className="center m20">
        <div className="center-text mb20">
          <Typography variant="h6">
            Config changed externally! Discard changes and reload?
          </Typography>
        </div>
        <div className="center-row wrap">
          <Button onClick={props.onAccept}>Yes</Button>
          <Button onClick={props.onReject}>No</Button>
        </div>
      </div>
    </Dialog>
  );
});

interface ErrorProps {
  open: boolean;
  msg: string;
  onClose?: () => void;
}

const Error = (props: ErrorProps) => {
  return (
    <Dialog open={props.open} onClose={props.onClose}>
      <div className="center m20">
        <div className="center-row wrap-reverse">
          <Typography variant="h6" className="p10">
            Error
          </Typography>
          <ErrorOutlineIcon />
        </div>
        <Typography variant="body1" className="p10">
          {props.msg}
        </Typography>
      </div>
    </Dialog>
  );
};

const Main = () => {
  const isAddError = useSelector(mainService, (state) =>
    state.matches("configInvalid")
  );
  const isSaveTimeoutError = useSelector(mainService, (state) =>
    state.matches("configSaveFailed")
  );
  const isConfigChangedExt = useSelector(mainService, (state) =>
    state.matches("configChangedExternally")
  );
  const isDisconnected = useSelector(mainService, (state) =>
    state.matches("socketClosed")
  );

  return (
    <>
      <Auth />
      <AddCommand />
      <Error
        open={isAddError}
        msg="Invalid config, cannot save"
        onClose={() => emit("CONFIG_ERROR_CLOSE")}
      />
      <Error
        open={isSaveTimeoutError}
        msg="Timed out while waiiting for save acknowledgement"
        onClose={() => emit("CONFIG_ERROR_CLOSE")}
      />
      <Error
        open={isDisconnected}
        msg="Connection to server lost, reconnecting..."
      />
      <ConfigChangedExt
        open={isConfigChangedExt}
        onAccept={() => emit("CONFIG_CHANGE_RELOAD")}
        onReject={() => emit("CONFIG_CHANGE_IGNORE")}
      />
      <ResponsiveDrawer />
    </>
  );
};

//interface CWSProps {}

const ConfigWithState: React.FC = () => {
  // rerender on schema, config or cursor change
  const compareCtx = useCallback(
    (prevCtx: TMainContext, nextCtx: TMainContext) =>
      prevCtx.schema === nextCtx.schema &&
      prevCtx.config === nextCtx.config &&
      prevCtx.currentCursor === nextCtx.currentCursor,
    []
  );
  const context = useSelector(
    mainService,
    (state) => state.context,
    compareCtx
  );

  const { schema, config, currentCursor } = context;
  const { type, index: currentIndex } = currentCursor;
  const currentConfig = config[type];

  if (currentIndex < 0 || currentConfig.length <= currentIndex) {
    return (
      <>
        <Typography>Welcome, {context.user}</Typography>
        <Typography>
          Add/select a command from the menu to get started
        </Typography>
      </>
    );
  }

  if (!currentConfig) return null;

  const currentCmd = currentConfig[currentIndex];

  return (
    <Config
      config={currentCmd}
      schema={schema}
      onChange={(config) =>
        emit({
          type: "CONFIG_CHANGED",
          config,
          cursor: currentCursor,
        })
      }
      index={currentIndex}
    />
  );
};

//interface SettingsDrawerProps {}

const SettingsDrawer: React.FC = () => {
  // rerender on config or cursor change
  const compareCtx = useCallback(
    (prevCtx: TMainContext, nextCtx: TMainContext) =>
      prevCtx.config === nextCtx.config &&
      prevCtx.currentCursor === nextCtx.currentCursor,
    []
  );
  const context = useSelector(
    mainService,
    (state) => state.context,
    compareCtx
  );

  const {
    config,
    currentCursor: { type: currentType, index: currentIndex },
    configChanged,
    configValid,
  } = context;

  const [commandsOpen, setCommandsOpen] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [timersOpen, setTimersOpen] = useState(false);

  if (Object.keys(config).length === 0) return null;

  // TODO: fix this lcusterfuck
  const list = [
    {
      name: "Commands",
      type: "commands",
      config: config["commands"],
      isOpen: commandsOpen,
      isNotEmpty: config["commands"] && config["commands"].length !== 0,
      toggle: () => setCommandsOpen(!commandsOpen),
    },
    {
      name: "Filters",
      type: "filters",
      config: config["filters"],
      isOpen: filtersOpen,
      isNotEmpty: config["filters"] && config["filters"].length !== 0,
      toggle: () => setFiltersOpen(!filtersOpen),
    },
    {
      name: "Timers",
      type: "timers",
      config: config["timers"],
      isOpen: timersOpen,
      isNotEmpty: config["timers"] && config["timers"].length !== 0,
      toggle: () => setTimersOpen(!timersOpen),
    },
  ];

  return (
    <>
      <ListItem>
        <Button onClick={() => emit("CONFIG_REVERT")} disabled={!configChanged}>
          Discard changes
        </Button>
        <Button
          onClick={() => emit("CONFIG_SAVE")}
          disabled={!configChanged || !configValid}
        >
          Save changes
        </Button>
      </ListItem>
      <div style={{ overflow: "auto" }}>
        {list.map(({ name, type, config, isOpen, isNotEmpty, toggle }) => (
          <React.Fragment key={name}>
            <ListItemButton
              onClick={toggle}
              disableRipple={!isNotEmpty}
              selected={type === currentType && !isOpen}
            >
              <ListItemText primary={name} />
              {isNotEmpty &&
                (isOpen ? (
                  <ExpandLess />
                ) : (
                  <>
                    {config.length}
                    <ExpandMore />
                  </>
                ))}
            </ListItemButton>
            <Collapse in={!isNotEmpty || isOpen} timeout="auto">
              <List>
                {config.map((cmd, index) => {
                  const enabled = (cmd.fields?.enabled as TBoolValue)?.Bool;
                  return (
                    <ListItem
                      key={`${cmd.type}${index}`}
                      selected={index === currentIndex && type === currentType}
                      disablePadding
                      secondaryAction={
                        <IconButton
                          onClick={() =>
                            emit({
                              type: "CONFIG_DELETE",
                              cursor: { type, index } as TConfigCursor,
                            })
                          }
                        >
                          <DeleteIcon />
                        </IconButton>
                      }
                      sx={{ pl: 1 }}
                    >
                      <ListItemButton
                        onClick={() =>
                          emit({
                            type: "CONFIG_SELECT",
                            cursor: { type, index } as TConfigCursor,
                          })
                        }
                        sx={{
                          backgroundColor: !is_valid_config(cmd) // TODO: move into machine context
                            ? "Tomato"
                            : "",
                        }}
                      >
                        <ListItemText
                          primary={
                            <Typography sx={{ opacity: enabled ? "1" : "0.4" }}>
                              {cmd.type}
                            </Typography>
                          }
                          secondary={
                            <Typography
                              sx={{ opacity: enabled ? "1" : "0.4" }}
                              variant="caption"
                            >
                              {cmd.name}
                            </Typography>
                          }
                        />
                      </ListItemButton>
                    </ListItem>
                  );
                })}
                <ListItem key={`add${type}`} disablePadding>
                  <ListItemButton
                    style={{ justifyContent: "center" }}
                    onClick={() =>
                      emit({
                        type: "CONFIG_ADD",
                        cmdType: type as TConfigType,
                      })
                    }
                  >
                    <ListItemIcon>
                      <AddCircleOutlineIcon />
                    </ListItemIcon>
                  </ListItemButton>
                </ListItem>
              </List>
            </Collapse>
          </React.Fragment>
        ))}
      </div>
    </>
  );
};

type TTab = "settings" | "logs";

interface NewDrawerProps {
  tab: TTab;
  setTab: (t: TTab) => void;
  logCursor: TLogCursor;
  setLogCursor: (p: TLogCursor) => void;
}

const StyledBadge = styled(Badge)(({ theme }) => ({
  "& .MuiBadge-badge": {
    backgroundColor: "#44b700",
    color: "#44b700",
    boxShadow: `0 0 0 2px ${theme.palette.background.paper}`,
    "&::after": {
      position: "absolute",
      top: 0,
      left: 0,
      width: "100%",
      height: "100%",
      borderRadius: "50%",
      animation: "ripple 2.4s infinite ease-in-out",
      border: "1px solid currentColor",
      content: '""',
    },
  },
  "@keyframes ripple": {
    "0%": {
      transform: "scale(.8)",
      opacity: 1,
    },
    "50%": {
      transform: "scale(2.4)",
      opacity: 0,
    },
    "100%": {
      transform: "scale(2.4)",
      opacity: 0,
    },
  },
}));

const NewDrawer: React.FC<NewDrawerProps> = (props) => {
  const { tab, setTab, logCursor, setLogCursor } = props;

  const inSettings = tab === "settings";
  const inLogs = tab === "logs";

  const drawerList = [
    {
      label: "Settings",
      selected: inSettings,
      handleClick: () => setTab("settings"), //() => emit({ type: "GOTO_SETTINGS" }),
    },
    {
      label: "Logs",
      selected: inLogs,
      handleClick: () => setTab("logs"), //() => emit({ type: "GOTO_STATS" }),
    },
  ];

  return (
    <>
      <Toolbar style={{ justifyContent: "space-evenly", alignItems: "center" }}>
        <StyledBadge
          overlap="circular"
          anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
          variant="dot"
        >
          <Avatar src="aussiebot.png" />
        </StyledBadge>
        <Typography variant="h5" onClick={() => emit("STATS_DUMP_LOG")}>
          Aussiebot
        </Typography>
      </Toolbar>
      <Divider />
      <List>
        {drawerList.map(({ label, selected, handleClick }, index) => (
          <ListItem key={index} disablePadding>
            <ListItemButton
              selected={selected}
              onClick={selected ? () => null : handleClick}
            >
              <ListItemText primary={label} />
            </ListItemButton>
          </ListItem>
        ))}
      </List>
      <Divider />
      {inSettings && <SettingsDrawer />}
      {inLogs && <LogsDrawer cursor={logCursor} onSelect={setLogCursor} />}
    </>
  );
};

interface MDDDrawerProps {
  children: ReactNode;
  mobileOpen: boolean;
  setMobileOpen: (s: boolean) => void;
}

const drawerWidth = 240;

const MDDrawer: React.FC<MDDDrawerProps> = (props) => {
  const isAtleastSm = useMediaQuery("(min-width: 500px)"); // atleast sm

  const drawerProps = isAtleastSm
    ? {}
    : {
        open: props.mobileOpen,
        onClose: () => props.setMobileOpen(!props.mobileOpen),
        ModalProps: { keepMounted: true },
      };

  return (
    <Drawer
      variant={isAtleastSm ? "permanent" : "temporary"}
      {...drawerProps}
      sx={{
        "& .MuiDrawer-paper": {
          boxSizing: "border-box",
          width: drawerWidth,
        },
      }}
    >
      {props.children}
    </Drawer>
  );
};

interface HideOnScrollProps {
  children?: ReactNode;
}

function HideOnScroll(props: HideOnScrollProps) {
  const { children } = props;
  // Note that you normally won't need to set the window ref as useScrollTrigger
  // will default to window.
  // This is only being set here because the demo is in an iframe.
  const trigger = useScrollTrigger();

  return (
    <Slide appear={false} direction="down" in={!trigger}>
      {children as ReactElement}
    </Slide>
  );
}

interface ResponsiveDrawerProps {
  children?: ReactNode;
  drawer?: ReactNode;
}

interface appBarLabelProps {
  tab: TTab;
  logCursor: TLogCursor;
}

function _appBarLabel(opt: appBarLabelProps): string {
  const { tab, logCursor } = opt;
  switch (tab) {
    case "settings":
      return "Settings";
    case "logs":
      switch (logCursor.type) {
        case "Chat":
          return `Chat logs (${TPlatform[logCursor.platform]})`;
        case "ModActions":
          return `Mod action logs (${TPlatform[logCursor.platform]})`;
      }
  }
  return "";
}

function ResponsiveDrawer(props: ResponsiveDrawerProps) {
  const isLoading = useSelector(mainService, (state) =>
    ["init", "auth", "reqSettings"].some(state.matches)
  );
  const inSaveConfig = useSelector(mainService, (state) =>
    state.matches("saveConfig")
  );

  const [tab, setTab] = useState("settings" as TTab);
  const [logCursor, setLogCursor] = useState({
    type: "Chat",
    platform: TPlatform.Discord,
  } as TLogCursor);
  const [mobileOpen, setMobileOpen] = useState(false);

  const inSettings = tab === "settings";
  const inLogs = tab === "logs";

  const appBarLabel = _appBarLabel({ tab, logCursor });

  const handleDrawerToggle = () => {
    setMobileOpen(!mobileOpen);
  };

  return (
    <Box sx={{ display: "flex" }}>
      <HideOnScroll>
        <AppBar
          position="fixed"
          sx={{
            width: { sm: `calc(100% - ${drawerWidth}px)` },
            ml: { sm: `${drawerWidth}px` },
          }}
        >
          <Toolbar>
            <IconButton
              color="inherit"
              aria-label="open drawer"
              edge="start"
              onClick={handleDrawerToggle}
              sx={{ mr: 2, display: { sm: "none" } }}
            >
              <MenuIcon />
            </IconButton>
            <Typography variant="h5" noWrap component="div">
              {appBarLabel}
            </Typography>
          </Toolbar>
        </AppBar>
      </HideOnScroll>
      <Box
        component="nav"
        sx={{ width: { sm: drawerWidth }, flexShrink: { sm: 0 } }}
        aria-label="menu"
      >
        <MDDrawer {...{ mobileOpen, setMobileOpen }}>
          <NewDrawer
            {...{
              tab,
              setTab,
              logCursor,
              setLogCursor,
            }}
          />
        </MDDrawer>
      </Box>
      <Box
        component="main"
        sx={{
          flexGrow: 1,
          p: 3,
          width: { sm: `calc(100% - ${drawerWidth}px)` },
        }}
      >
        <Toolbar />
        <div>
          {inSettings &&
            (!isLoading && !inSaveConfig ? <ConfigWithState /> : <Loading />)}
          {inLogs && <Logs cursor={logCursor} />}
        </div>
      </Box>
    </Box>
  );
}

export default Main;
