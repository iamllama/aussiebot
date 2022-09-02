import React, { useCallback, useEffect, useState } from "react";
import { mainService, emit } from "./machine";
import { useSelector } from "@xstate/react";
import {
  Dialog,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  Typography,
  TextField,
  Button,
  IconButton,
  Tooltip,
} from "@mui/material";
import RefreshIcon from "@mui/icons-material/Refresh";

//export interface AuthProps {}

export const Auth: React.FC = (props) => {
  const inSelectUser = useSelector(mainService, (state) =>
    state.matches("auth.selectUser")
  );
  const inInputCode = useSelector(mainService, (state) =>
    state.matches("auth.inputCode")
  );
  const codeRequested = useSelector(mainService, (state) =>
    state.matches("auth.inputCode.codeRequest.pending")
  );
  const codeReady = useSelector(mainService, (state) =>
    state.matches("auth.inputCode.codeRequest.ready")
  );
  const authPending = useSelector(mainService, (state) =>
    state.matches("auth.inputCode.login.pending")
  );
  const authFail = useSelector(mainService, (state) =>
    state.matches("auth.inputCode.login.failed")
  );
  const ratelimited = useSelector(mainService, (state) =>
    state.matches("auth.ratelimited")
  );

  const context = mainService.state.context;
  const users = context.users;
  const user = context.user ?? "user";

  const onSubmit = useCallback(
    (code: string) => emit({ type: "AUTH_CODE_ENTERED", code }),
    []
  );
  const onReqCode = useCallback(
    () => emit({ type: "AUTH_CODE_REQUESTED" }),
    []
  );

  if (ratelimited) {
    return (
      <Dialog open={true}>
        <div
          className="center m10"
          style={{ maxWidth: "300px", textAlign: "center" }}
        >
          <Typography variant="h6" className="p10" gutterBottom>
            Login temporarily blocked 🛑
          </Typography>
          <Typography variant="subtitle1" className="p10">
            Please close all Aussiebot tabs and try again after the preset
            cooldown has ended
          </Typography>
        </div>
      </Dialog>
    );
  }

  return (
    <>
      <SelectUser open={inSelectUser} {...{ users }} />
      <InputCode
        open={inInputCode}
        {...{
          user,
          onSubmit,
          onReqCode,
          codeRequested,
          codeReady,
          authPending,
          authFail,
        }}
      />
    </>
  );
};

interface SelectUserProps {
  open: boolean;
  users: string[];
  onClose?: () => void;
}

const SelectUser: React.FC<SelectUserProps> = (props) => {
  return (
    <Dialog open={props.open} onClose={props.onClose}>
      <div className="center m10" style={{ maxWidth: "300px" }}>
        <Typography variant="h6" className="p10">
          Login as
        </Typography>
        <List style={{ maxHeight: "300px", overflow: "auto" }}>
          {props.users.map((user) => (
            <ListItem key={user}>
              <ListItemButton
                onClick={() => emit({ type: "AUTH_USER_SELECTED", user })}
              >
                <ListItemText primary={user} sx={{ textAlign: "center" }} />
              </ListItemButton>
            </ListItem>
          ))}
        </List>
      </div>
    </Dialog>
  );
};

interface InputCodeProps {
  open: boolean;
  user: string;
  onReqCode?: () => void;
  onSubmit: (code: string) => void;
  onClose?: () => void;
  codeRequested?: boolean;
  codeReady?: boolean;
  authFail?: boolean;
  authPending?: boolean;
}

const InputCode: React.FC<InputCodeProps> = (props) => {
  const [code, setCode] = useState("");
  const {
    open,
    onClose,
    onSubmit,
    onReqCode,
    user,
    authFail,
    authPending,
    codeRequested,
    codeReady,
  } = props;

  useEffect(() => {
    if (authFail) setCode("");
  }, [authFail]);

  return (
    <Dialog open={open} onClose={onClose}>
      <div className="center m10" style={{ maxWidth: "300px" }}>
        <Typography variant="h6" className="p10">
          {authFail
            ? "Invalid code, try again"
            : authPending
            ? "Attempting login"
            : codeReady
            ? "Code sent, check DMs"
            : `Hello, ${user}. Enter your code`}
        </Typography>
        <div
          style={{
            display: "flex",
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <TextField
            label="Code"
            variant="outlined"
            type="password"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            error={authFail}
            style={{ margin: "0 10px 0 0" }}
          />
          <Tooltip title="Request new code">
            <span>
              <IconButton
                aria-label="request new code"
                onClick={onReqCode}
                disabled={codeRequested || codeReady}
              >
                <RefreshIcon />
              </IconButton>
            </span>
          </Tooltip>
        </div>
        <Button
          disabled={code.length === 0}
          onClick={() => onSubmit(code.trim())}
          style={{ margin: "10px 10px 0 10px" }}
        >
          Login
        </Button>
      </div>
    </Dialog>
  );
};
