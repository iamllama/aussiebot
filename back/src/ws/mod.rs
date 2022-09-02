use crate::{
    auth::{self, AuthMsg, AuthResp},
    error,
    msg::Location,
};
use futures_util::{pin_mut, stream::SplitStream, SinkExt, StreamExt, TryStreamExt};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{Request, Response},
        http::{HeaderMap, HeaderValue, StatusCode},
        Message,
    },
    WebSocketStream,
};
use url::Url;

pub type Msg = (Option<Vec<(Arc<String>, SocketAddr)>>, Arc<String>);
type PeerMap = HashMap<SocketAddr, mpsc::Sender<Arc<String>>>;

const HEARTBEAT_PING: &str = "💓";
const HEARTBEAT_PONG: &str = "👀";

/// WS server handles demuxing. It has to keep track of which peer SocketAddr corresponds to which ws_out_tx channel
/// msg_in_tx is just cloned and shared across all peers as a fan-in channel
///
///  redis -------\
///  ws peer 1 rx \| (fanin)                                      (demux)  / peer 1 tx
///  ws peer 2 rx  |---------> msg_in_tx -> msg task -> ws_in_rx -------->|  peer 2 tx
///  ws peer 3 rx /                                                        \ peer 3 tx
///
#[derive(Clone)]
pub struct Server {
    msg_in_tx: mpsc::Sender<(Location, String)>, // <- ws
    clients: Arc<RwLock<PeerMap>>,               // map sockets to channels
    disconnect_tx: mpsc::Sender<SocketAddr>,     // receive disconnect events
    auth: auth::Handle,
}

#[derive(Debug)]
pub enum WsError {
    Parse(&'static str),
    MissingHeader(&'static str),
    CorsMissingOrigin,
    CorsInvalidOrigin { origin: String },
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

// TODO: state machine for handling auth
impl Server {
    #[tracing::instrument(skip_all)]
    async fn fanout(mut ws_in_rx: mpsc::Receiver<Msg>, clients: Arc<RwLock<PeerMap>>) {
        while let Some((dest_addrs, msg)) = ws_in_rx.recv().await {
            if let Some(addrs) = dest_addrs {
                match addrs[..] {
                    // slice pattern for a single elem
                    [ref addr] => {
                        let (_username, addr) = addr;
                        let client = clients.read().get(addr).cloned();
                        if let Some(tx) = client {
                            let _ = tx.send(msg).await;
                        }
                    }
                    _ => {
                        // filter and send
                        let clients: PeerMap = clients.read().clone();
                        Self::send_mult(
                            msg,
                            addrs
                                .iter()
                                .filter_map(|(_username, addr)| clients.get(addr)),
                        )
                        .await
                    }
                };
            } else {
                let clients: PeerMap = clients.read().clone();
                Self::send_mult(msg, clients.iter().map(|(_, tx)| tx)).await;
            }
        }
    }

    pub fn new(
        msg_in_tx: mpsc::Sender<(Location, String)>, /* <- ws */
        ws_in_rx: mpsc::Receiver<Msg>,               /* -> ws */
        auth: auth::Handle,
    ) -> Self {
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let (disconnect_tx, disconnect_rx) = mpsc::channel::<SocketAddr>(32);

        // spawn task to handle disconnects
        tokio::spawn(Self::disconnect(clients.clone(), disconnect_rx));

        // fan out ws_in_rx to all clients
        tokio::spawn(Self::fanout(ws_in_rx, clients.clone()));

        Self {
            clients,
            disconnect_tx,
            msg_in_tx,
            auth,
        }
    }

    async fn disconnect(
        clients: Arc<RwLock<PeerMap>>,
        mut disconnect_rx: mpsc::Receiver<SocketAddr>,
    ) {
        while let Some(addr) = disconnect_rx.recv().await {
            clients.write().remove(&addr);
            tracing::debug!("removed {} from clients", addr);
        }
    }

    async fn send_mult<'a, M, I>(msg: M, clients: I)
    where
        M: 'a + Clone,
        I: Iterator<Item = &'a mpsc::Sender<M>>,
    {
        tracing::debug!(
            "\x1b[33mSending to approx {:?} ws peers\x1b[0m",
            clients.size_hint()
        );
        for tx in clients {
            let _ = tx.send(msg.clone()).await;
        }
    }

    #[tracing::instrument(skip_all)]
    async fn auth(
        ws_stream: WebSocketStream<TcpStream>,
        auth: &auth::Handle,
        peer_ip: String,
    ) -> error::Result<Option<(Arc<String>, WebSocketStream<TcpStream>)>> {
        let (mut ws_sink, mut ws_source) = ws_stream.split();

        while let Some(Ok(msg)) = ws_source.next().await {
            let msg = if msg.is_text() || msg.is_binary() {
                if let Ok(msg) = msg.into_text() {
                    msg
                } else {
                    continue;
                }
            } else if msg.is_close() {
                return Ok(None);
            } else {
                continue;
            };

            if msg == HEARTBEAT_PING {
                ws_sink.send(HEARTBEAT_PONG.into()).await?;
                continue;
            }

            let msg = tokio::task::spawn_blocking(move || {
                let de = serde_json::from_str::<AuthMsg>(&msg);
                (msg, de)
            })
            .await;

            let msg = match msg {
                Ok((_, Ok(msg))) => msg,
                Ok((orig_msg, Err(e))) => {
                    tracing::error!(orig_msg = ?orig_msg, "{:?}", e);
                    continue; // return None to break conn
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue; // return None to break conn
                }
            };

            tracing::debug!("msg = {:?}", msg);

            let resp = auth.handle(&peer_ip, msg).await;

            tracing::debug!("resp = {:?}", resp);

            let resp = match resp {
                Err(e) => {
                    tracing::error!("{}", e);
                    continue;
                }
                Ok(r) => r,
            };

            //let auth_success = resp == AuthResp::AuthSuccess;

            let res = tokio::task::spawn_blocking(move || {
                (serde_json::to_string::<AuthResp>(&resp), resp)
            })
            .await;

            let (resp_str, resp) = match res {
                Ok((Ok(resp_str), resp)) => (resp_str, resp),
                Ok((Err(e), resp)) => {
                    tracing::error!("{:?}, orig resp: {:?}", e, resp);
                    continue;
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };

            let _ = ws_sink.send(Message::Text(resp_str)).await;

            if let AuthResp::AuthSuccess(user) = resp {
                // from this point on, conn is authenticated
                let ws_stream = ws_sink.reunite(ws_source)?;
                return Ok(Some((user, ws_stream)));
            }
        }

        Ok(None)
    }

    #[tracing::instrument(skip(ws_receiver, msg_in_tx, disconnect_tx, hb_tx))]
    async fn ws_read(
        peer: SocketAddr,
        ws_receiver: SplitStream<WebSocketStream<TcpStream>>,
        msg_in_tx: mpsc::Sender<(Location, String)>,
        disconnect_tx: mpsc::Sender<SocketAddr>,
        hb_tx: mpsc::Sender<()>,
        username: Arc<String>,
    ) {
        tracing::debug!("starting read task");
        // filter non-text or binary messages
        let filtered = ws_receiver.try_filter_map(|msg| async move {
            if msg.is_text() || msg.is_binary() {
                Ok(msg.into_text().ok())
            } else {
                Ok(None)
            }
        });

        // pin stream future
        pin_mut!(filtered);

        // ws -> msg task
        while let Some(Ok(msg)) = filtered.next().await {
            if msg == HEARTBEAT_PING {
                let _ = hb_tx.send(()).await;
            } else {
                // wrap with location
                let msg = (Location::Websocket(username.clone(), peer), msg);
                if msg_in_tx.send(msg).await.is_err() {
                    break;
                }
            }
        }

        // peer disconnected, handle cleanup
        tracing::info!("\x1b[91mdisconnected, cleaning up\x1b[0m");
        // a channel is not an owning ref, so we prefer that?
        let _ = disconnect_tx.send(peer).await;
    }

    fn real_ip(real_ip: &mut Option<IpAddr>, headers: &HeaderMap) -> error::Result<()> {
        let hv = if let Some(hv) = headers.get("x-real-ip") {
            hv
        } else {
            headers
                .get("x-forwarded-for")
                .ok_or(WsError::MissingHeader("x-forwarded-for"))? // fallback
        };
        let ip = hv.to_str()?.parse::<IpAddr>()?;
        real_ip.replace(ip);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn cors(headers: &HeaderMap, res: &mut Response) -> error::Result<()> {
        use tokio_tungstenite::tungstenite::http::header;

        let _origin = headers
            .get("origin")
            .ok_or(WsError::CorsMissingOrigin)?
            .to_str()?;
        let origin = Url::parse(_origin)?;

        //tracing::debug!(headers=?headers, origin=?origin);
        tracing::debug!(origin=?origin);

        let origin = origin.host_str().ok_or(WsError::CorsInvalidOrigin {
            origin: origin.to_string(),
        })?;

        // TODO: this should be in config
        match origin {
            "aussiebot.siid.sh" | "localhost" | "127.0.0.1" => {}
            ip if ip.starts_with("192.168.1.") => {}
            _ => {
                return Err(WsError::CorsInvalidOrigin {
                    origin: origin.to_owned(),
                }
                .into())
            }
        }

        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin#:~:text=Limiting%20the%20possible,the%20Origin%20value.
        res.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            _origin
                .parse::<HeaderValue>()
                .map_err(|_| WsError::Parse("_origin"))?,
        );
        res.headers_mut().insert(
            header::VARY,
            "Origin"
                .parse::<HeaderValue>()
                .map_err(|_| WsError::Parse("Vary: Origin"))?,
        );

        Ok(())
    }

    // TODO: should not be infallible
    #[tracing::instrument(skip_all, fields(peer))]
    async fn new_conn(&self, peer: SocketAddr, stream: TcpStream) {
        let mut real_ip: Option<IpAddr> = None;

        let ws_stream = accept_hdr_async(stream, |req: &Request, mut res: Response| {
            let headers = req.headers();
            if let Err(e) = Self::cors(headers, &mut res) {
                tracing::error!("{}", e);
                return Err(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(None)
                    .unwrap());
            }
            if let Err(e) = Self::real_ip(&mut real_ip, headers) {
                tracing::error!("{}", e);
                return Err(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(None)
                    .unwrap());
            }
            Ok(res)
        })
        .await
        .expect("Failed to accept");

        // replace peer with real peer if available
        let peer = if let Some(ip) = real_ip {
            SocketAddr::from((ip, peer.port()))
        } else {
            peer
        };

        tracing::Span::current().record("peer", &&*peer.to_string());
        tracing::debug!("\x1b[93mnew ws connection, waiting for auth\x1b[0m");

        // wait till auth completes
        let auth_resp = Self::auth(ws_stream, &self.auth, peer.ip().to_string()).await;

        let (username, ws_stream) = match auth_resp {
            Err(e) => {
                tracing::error!("{}", e);
                return;
            }
            Ok(Some((user, ws))) => {
                tracing::info!("\x1b[92mAuth success\x1b[0m");
                (user, ws)
            }
            Ok(_) => {
                tracing::info!("\x1b[91mAuth failed\x1b[0m");
                return;
            }
        };

        let (mut ws_sender, ws_receiver) = ws_stream.split();

        let (ws_in_tx, mut ws_chan) = mpsc::channel::<Arc<String>>(32);

        let disconnect_tx = self.disconnect_tx.clone();
        let msg_in_tx = self.msg_in_tx.clone();

        // heartbeat channel
        let (hb_tx, mut hb_rx) = mpsc::channel::<()>(32);

        //add (peer, ws_in_tx) to self.clients
        // add first before starting
        let clients = self.clients.clone();
        tokio::task::spawn_blocking(move || {
            clients.write().insert(peer, ws_in_tx);
            tracing::debug!("added {} to clients", peer);
        })
        .await
        .unwrap();

        // spawn task to read from ws
        // aborts when peer's incoming stream closes
        let _ = tokio::spawn(Self::ws_read(
            peer,
            ws_receiver,
            msg_in_tx,
            disconnect_tx,
            hb_tx,
            username,
        ));

        // spawn task to write to ws
        // aborts when ws_in_tx is dropped from the peer map
        let _ = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = hb_rx.recv() => {
                      let _ = ws_sender.send(HEARTBEAT_PONG.into()).await;
                    }
                    msg = ws_chan.recv() => {
                        match msg {
                          Some(msg) => {
                            if (ws_sender.send((&*msg).to_owned().into()).await).is_err() {
                                break;
                            }
                        },
                          _ => break
                        }
                    }
                }
            }
            tracing::debug!("\x1b[91mtx task stopped\x1b[0m");
        });
    }

    /// Start the server, consuming it
    #[tracing::instrument(skip(self))]
    pub async fn start(self) {
        tracing::info!("\x1b[92m------------Starting websocket loop------------\x1b[0m");

        let listener = TcpListener::bind(&*crate::WS_BIND)
            .await
            .expect("Can't listen");

        // spawn task to accept new ws conns
        // aborts when listener closes
        // in which case it'll drop self
        tokio::spawn(async move {
            loop {
                if let Ok((stream, peer)) = listener.accept().await {
                    let server = self.clone();
                    tokio::spawn(async move {
                        server.new_conn(peer, stream).await;
                    });
                }
            }
        });

        tracing::info!(addr = %&*crate::WS_BIND, "listening");
    }
}
