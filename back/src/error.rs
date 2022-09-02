use crate::pubsub::EOF as PubSubEOf;
use crate::{
    cmds::link::LinkError,
    cmds::OwnedValueError,
    db::give::GiveError,
    msg::{ArgMapError, PlatformError},
    ws::WsError,
};
use bb8::RunError as Bb8RunError;
use bb8_redis::redis::RedisError;
use futures_util::stream::ReuniteError;
use std::io::Error as IoError;
use std::num::TryFromIntError;
use std::{fmt::Display, net::AddrParseError, num::ParseIntError, time::SystemTimeError};
use tokio::sync::oneshot::error::RecvError as OneShotRecvError;
use tokio::{sync::mpsc::error::SendError, task::JoinError};
use tokio_tungstenite::tungstenite::http::header::ToStrError as HeaderToStrError;
use tokio_tungstenite::tungstenite::Error as TungsteniteError;
use url::ParseError as UrlParseError;

pub(crate) type Result<T> = std::result::Result<T, Error>;

// pub(crate) type Resuult<T> = std::result::Result<T, Erroor>;

// pub enum Erroor {
//     Redis(RedisError),
//     ParseInt(ParseIntError),
//     SerdeJson(serde_json::Error),
//     Postgres(tokio_postgres::Error),
//     SystemTime(SystemTimeError),
//     Join(JoinError),
//     HeaderToStr(HeaderToStrError),
//     AddrParse(AddrParseError),
//     UrlParse(UrlParseError),
// }

// impl From<RedisError> for Erroor {
//     fn from(err: RedisError) -> Self {
//         Erroor::Redis(err)
//     }
// }

// // impl<T> From<::std::result::Result<T, ArgMapError>> for Resuult<T> {
// //     fn from(_: ::std::result::Result<T, ArgMapError>) -> Self {
// //         todo!()
// //     }
// // }

// impl Display for Erroor {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Erroor::Redis(e) => e.fmt(f),
//             Erroor::ParseInt(e) => e.fmt(f),
//             Erroor::SerdeJson(e) => e.fmt(f),
//             Erroor::Postgres(e) => e.fmt(f),
//             Erroor::SystemTime(e) => e.fmt(f),
//             Erroor::Join(e) => e.fmt(f),
//             Erroor::HeaderToStr(e) => e.fmt(f),
//             Erroor::AddrParse(e) => e.fmt(f),
//             Erroor::UrlParse(e) => e.fmt(f),
//         }
//     }
// }

macro_rules! def_err {
  ( $( $err_name:ident ( $err_ty:ty )   ),+ $(,)? ) => {

    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Error {
      Generic(String),
      $( $err_name( $err_ty ) ),+
    }

    impl std::error::Error for Error {}

    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
              Error::Generic(s) => f.write_str(s),
              $(
                Error::$err_name(e) => e.fmt(f)
              ),+
            }
        }
    }

  $(
    impl From<$err_ty> for Error {
        fn from(err: $err_ty) -> Self {
            Error::$err_name(err)
        }
    }
  )+

  }
}

// channels are used in a lot of places, so define error here
#[derive(Debug)]
pub struct ChanSendError {
    pub(crate) msg: String,
}

impl std::fmt::Display for ChanSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("error sending on channel: {}", self.msg))
    }
}

#[derive(Debug)]
pub struct ChanRecvError {}

impl std::fmt::Display for ChanRecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("error receiving on channel")
    }
}

#[derive(Debug)]
pub struct StreamReuniteError {
    pub(crate) msg: String,
}

impl std::fmt::Display for StreamReuniteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("error reuniting streams: {}", self.msg))
    }
}

#[derive(Debug)]
pub struct Bb8Error {
    pub(crate) msg: String,
}

impl std::fmt::Display for Bb8Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("bb8 error: {}", self.msg))
    }
}

#[derive(Debug)]
pub struct Nop;

impl std::fmt::Display for Nop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Nop")
    }
}

def_err![
    Nop(Nop),
    Io(IoError),
    Redis(RedisError),
    Bb8(Bb8Error),
    Tungestenite(TungsteniteError),
    StreamReunite(StreamReuniteError),
    Ws(WsError),
    ParseInt(ParseIntError),
    SerdeJson(serde_json::Error),
    Postgres(tokio_postgres::Error),
    SystemTime(SystemTimeError),
    Join(JoinError),
    HeaderToStr(HeaderToStrError),
    AddrParse(AddrParseError),
    UrlParse(UrlParseError),
    ArgMap(ArgMapError),
    Platform(PlatformError),
    OwnedValue(OwnedValueError),
    ChanSend(ChanSendError),
    OneShotRecv(OneShotRecvError),
    GiveOp(GiveError),
    PubSubEOF(PubSubEOf),
    Link(LinkError),
    TryFromInt(TryFromIntError)
];

impl<T> From<SendError<T>> for Error {
    fn from(e: SendError<T>) -> Self {
        Error::ChanSend(ChanSendError { msg: e.to_string() })
    }
}

impl<T, U> From<ReuniteError<T, U>> for Error {
    fn from(e: ReuniteError<T, U>) -> Self {
        Error::StreamReunite(StreamReuniteError { msg: e.to_string() })
    }
}

impl<T: std::fmt::Debug> From<Bb8RunError<T>> for Error {
    fn from(e: Bb8RunError<T>) -> Self {
        Error::Bb8(Bb8Error {
            msg: format!("{:?}", e),
        })
    }
}

impl<'a> From<&'a str> for Error {
    fn from(e: &'a str) -> Self {
        Error::Generic(e.into())
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::Generic(e)
    }
}
