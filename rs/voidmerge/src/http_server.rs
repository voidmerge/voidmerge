//! VoidMerge http server.

use crate::*;
use axum::response::IntoResponse;
use std::sync::Arc;

struct State {
    server: Arc<server::Server>,
}

struct ErrTx(std::io::Error);

impl From<std::io::Error> for ErrTx {
    fn from(e: std::io::Error) -> Self {
        Self(e)
    }
}

impl axum::response::IntoResponse for ErrTx {
    fn into_response(self) -> axum::response::Response {
        let str_err = format!("{:?}", self.0);

        use axum::http::StatusCode as H;
        use std::io::ErrorKind::*;

        match self.0.kind() {
            NotFound => (H::NOT_FOUND, str_err),
            PermissionDenied => (H::UNAUTHORIZED, str_err),
            InvalidInput | InvalidData => (H::BAD_REQUEST, str_err),
            QuotaExceeded => (H::TOO_MANY_REQUESTS, str_err),
            FileTooLarge => (H::PAYLOAD_TOO_LARGE, str_err),
            // Interrupted->CONFLICT because both of these indicate
            // the user should just try again.
            Interrupted => (H::CONFLICT, str_err),
            _ => (H::INTERNAL_SERVER_ERROR, str_err),
        }
        .into_response()
    }
}

impl axum::response::IntoResponse for crate::js::JsResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            crate::js::JsResponse::FnResOk {
                status,
                body,
                headers,
                ..
            } => {
                let mut bld =
                    axum::response::Response::builder().status(status as u16);

                {
                    let hdr = bld.headers_mut().unwrap();
                    for (k, v) in headers.iter() {
                        if let Ok(v) = axum::http::HeaderValue::from_str(v) {
                            if let Ok(k) =
                                axum::http::HeaderName::from_bytes(k.as_bytes())
                            {
                                hdr.insert(k, v);
                            }
                        }
                    }
                }

                bld.body(axum::body::Body::from(body)).unwrap()
            }
            _ => unreachable!(),
        }
    }
}

type AxumResult = std::result::Result<axum::response::Response, ErrTx>;

/// Execute a VoidMerge http server process.
pub async fn http_server(
    running: tokio::sync::oneshot::Sender<std::net::SocketAddr>,
    bind: std::net::SocketAddr,
    server: server::Server,
) -> Result<()> {
    let state = Arc::new(State {
        server: Arc::new(server),
    });

    let cors = tower_http::cors::CorsLayer::new()
        .allow_methods([axum::http::Method::GET, axum::http::Method::PUT])
        .allow_headers([axum::http::header::AUTHORIZATION])
        .allow_origin(tower_http::cors::Any);

    let app: axum::Router<Arc<State>> = axum::Router::new()
        .route("/", axum::routing::get(route_health_get))
        .route("/ctx-setup", axum::routing::put(route_ctx_setup_put))
        .route(
            "/{ctx}/_vm_/config",
            axum::routing::put(route_ctx_config_put),
        )
        .route("/{ctx}/{*path}", axum::routing::get(route_fn_get))
        .route("/{ctx}/", axum::routing::get(route_fn_get_def))
        .route("/{ctx}", axum::routing::get(route_fn_get_def))
        .route("/{ctx}/{*path}", axum::routing::put(route_fn_put))
        .route("/{ctx}/", axum::routing::put(route_fn_put_def))
        .route("/{ctx}", axum::routing::put(route_fn_put_def));

    let app = app
        .layer(cors)
        .with_state(state)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();

    let handle = axum_server::Handle::new();

    let server = axum_server::bind(bind).handle(handle.clone()).serve(app);

    tokio::task::spawn(async move {
        if let Some(bound_addr) = handle.listening().await {
            let _ = running.send(bound_addr);
        }
    });

    server.await
}

fn auth_token(headers: &axum::http::HeaderMap) -> Arc<str> {
    headers
        .get("authorization")
        .and_then(|t| t.to_str().ok())
        .and_then(|t| {
            let (k, v) = t.split_once(" ")?;
            if k.trim().to_lowercase() == "bearer" {
                Some(v.trim())
            } else {
                None
            }
        })
        .unwrap_or("")
        .into()
}

async fn route_health_get(
    axum::extract::State(state): axum::extract::State<Arc<State>>,
) -> AxumResult {
    state.server.health_get().await?;
    Ok("Ok".into_response())
}

async fn route_ctx_setup_put(
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    state
        .server
        .ctx_setup_put(token, payload.to_decode()?)
        .await?;
    Ok("Ok".into_response())
}

async fn route_ctx_config_put(
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    state
        .server
        .ctx_config_put(token, payload.to_decode()?)
        .await?;
    Ok("Ok".into_response())
}

fn hdr(m: &axum::http::HeaderMap) -> std::collections::HashMap<String, String> {
    m.into_iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).to_string(),
            )
        })
        .collect()
}

async fn route_fn_get(
    headers: axum::http::HeaderMap,
    axum::extract::Path((ctx, path)): axum::extract::Path<(String, String)>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
) -> AxumResult {
    let req = crate::js::JsRequest::FnReq {
        method: "GET".into(),
        path: path.into(),
        body: None,
        headers: hdr(&headers),
    };
    Ok(state.server.fn_req(ctx.into(), req).await?.into_response())
}

async fn route_fn_get_def(
    headers: axum::http::HeaderMap,
    axum::extract::Path(ctx): axum::extract::Path<String>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
) -> AxumResult {
    let req = crate::js::JsRequest::FnReq {
        method: "GET".into(),
        path: "".into(),
        body: None,
        headers: hdr(&headers),
    };
    Ok(state.server.fn_req(ctx.into(), req).await?.into_response())
}

async fn route_fn_put(
    headers: axum::http::HeaderMap,
    axum::extract::Path((ctx, path)): axum::extract::Path<(String, String)>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let req = crate::js::JsRequest::FnReq {
        method: "PUT".into(),
        path: path.into(),
        body: Some(payload),
        headers: hdr(&headers),
    };
    Ok(state.server.fn_req(ctx.into(), req).await?.into_response())
}

async fn route_fn_put_def(
    headers: axum::http::HeaderMap,
    axum::extract::Path(ctx): axum::extract::Path<String>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let req = crate::js::JsRequest::FnReq {
        method: "PUT".into(),
        path: "".into(),
        body: Some(payload),
        headers: hdr(&headers),
    };
    Ok(state.server.fn_req(ctx.into(), req).await?.into_response())
}

/*
async fn route_fn(
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(_state): axum::extract::State<Arc<State>>,
    _payload: bytes::Bytes,
) -> AxumResult {
    println!("fn {method}");
    /*
    state
        .server
        .ctx_setup_put(token, payload.to_decode()?)
        .await?;
    */
    Ok("Ok".into_response())
}
*/

/*
use crate::*;
use axum::response::IntoResponse;
use std::collections::HashMap;
use types::*;

type WsSend = tokio::sync::mpsc::Sender<axum::extract::ws::Message>;

struct WsInfo {
    send: WsSend,
    peer_token: Hash,
}

struct AppState {
    server: Arc<server::Server>,
    ws_senders: Mutex<HashMap<Hash, WsInfo>>,
}

impl AppState {
    pub fn new(server: Arc<server::Server>) -> Self {
        Self {
            server,
            ws_senders: Default::default(),
        }
    }

    pub fn get_hash_for_token(&self, token: &Hash) -> Option<Hash> {
        let lock = self.ws_senders.lock().unwrap();
        for (hash, WsInfo { peer_token, .. }) in lock.iter() {
            if peer_token == token {
                return Some(hash.clone());
            }
        }
        None
    }

    pub async fn ws_send(
        &self,
        hash: &Hash,
        msg: axum::extract::ws::Message,
    ) -> Result<()> {
        let send = {
            let mut lock = self.ws_senders.lock().unwrap();
            let send = match lock.get(hash) {
                None => return Err(std::io::Error::other("hash not found")),
                Some(s) => s.send.clone(),
            };
            if send.is_closed() {
                let _ = lock.remove(hash);
                return Err(std::io::Error::other("hash not found"));
            }
            send
        };
        send.send(msg).await.map_err(std::io::Error::other)
    }
}

/// VoidMerge http server.
pub struct HttpServer {
    server_task: tokio::task::JoinHandle<()>,
    maint_task: tokio::task::JoinHandle<()>,
    bound_addr: std::net::SocketAddr,
    handle: axum_server::Handle,
    notify_shutdown: Arc<tokio::sync::Notify>,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.maint_task.abort();
        self.handle
            .graceful_shutdown(Some(std::time::Duration::from_secs(5)));
        self.server_task.abort();
    }
}

impl HttpServer {
    /// Bind a new HttpServer listening on the configured interface.
    pub async fn new(server: Arc<server::Server>) -> Result<Arc<Self>> {
        let config = server.config().clone();
        let app_state = Arc::new(AppState::new(server));

        let app_state2 = app_state.clone();
        let maint_task = tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                app_state2
                    .ws_senders
                    .lock()
                    .unwrap()
                    .retain(|_k, v| !v.send.is_closed());
            }
        });

        let cors = tower_http::cors::CorsLayer::new()
            .allow_methods([axum::http::Method::GET, axum::http::Method::PUT])
            .allow_headers([axum::http::header::AUTHORIZATION])
            .allow_origin(tower_http::cors::Any);

        let mut app: axum::Router<Arc<AppState>> = axum::Router::new()
            .route("/health", axum::routing::get(route_health))
            .route("/context/{context_hash}", axum::routing::put(route_context))
            .route("/listen/{token}", axum::routing::any(route_listen))
            .route(
                "/send/{context_hash}/{peer_hash}",
                axum::routing::put(route_send),
            )
            .route("/status/{context_hash}", axum::routing::get(route_status))
            .route("/auth-chal-req", axum::routing::get(route_auth_chal_req))
            .route("/auth-chal-res", axum::routing::put(route_auth_chal_res))
            .route("/insert/{context_hash}", axum::routing::put(route_insert))
            .route("/select/{context_hash}", axum::routing::put(route_select))
            .route(
                "/web/{context_hash}/{*rest}",
                axum::routing::get(route_web),
            );

        if let Some(default_context) = &config.default_context {
            app = app.route(
                "/",
                axum::routing::any(axum::response::Redirect::to(&format!(
                    "/web/{default_context}/index.html"
                ))),
            );
        }

let app = app
            .layer(cors)
            .with_state(app_state)
            .into_make_service_with_connect_info::<std::net::SocketAddr>();

        let handle = axum_server::Handle::new();

        let bind: std::net::SocketAddr =
            config.http_addr.parse().map_err(std::io::Error::other)?;

        let server = axum_server::bind(bind).handle(handle.clone()).serve(app);

        let notify_shutdown = Arc::new(tokio::sync::Notify::new());
        let notify_shutdown2 = notify_shutdown.clone();

        let server_task = tokio::task::spawn(async move {
            let _ = server.await;
            notify_shutdown2.notify_waiters();
        });

        let bound_addr = match handle.listening().await {
            Some(a) => a,
            None => {
                server_task.abort();
                return Err(std::io::Error::other(
                    "failed to start http server",
                ));
            }
        };

        let this = Arc::new(Self {
            server_task,
            maint_task,
            bound_addr,
            handle,
            notify_shutdown,
        });

        Ok(this)
    }

    /// Get the bound address of this http server.
    pub fn bound_addr(&self) -> &std::net::SocketAddr {
        &self.bound_addr
    }

    /// Wait for the server to exit.
    pub async fn wait(&self) {
        self.notify_shutdown.notified().await;
    }
}

struct ErrTx(std::io::Error);

impl From<std::io::Error> for ErrTx {
    fn from(e: std::io::Error) -> Self {
        Self(e)
    }
}

impl axum::response::IntoResponse for ErrTx {
    fn into_response(self) -> axum::response::Response {
        let str_err = format!("{:?}", self.0);

        use axum::http::StatusCode as H;
        use std::io::ErrorKind::*;

        match self.0.kind() {
            NotFound => (H::NOT_FOUND, str_err),
            PermissionDenied => (H::UNAUTHORIZED, str_err),
            InvalidInput | InvalidData => (H::BAD_REQUEST, str_err),
            QuotaExceeded => (H::TOO_MANY_REQUESTS, str_err),
            FileTooLarge => (H::PAYLOAD_TOO_LARGE, str_err),
            // Interrupted->CONFLICT because both of these indicate
            // the user should just try again.
            Interrupted => (H::CONFLICT, str_err),
            _ => (H::INTERNAL_SERVER_ERROR, str_err),
        }
        .into_response()
    }
}

type AxumResult = std::result::Result<axum::response::Response, ErrTx>;

fn auth_token(headers: &axum::http::HeaderMap) -> Hash {
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            let (k, v) = h.split_once(" ")?;
            if k.trim().to_lowercase() == "bearer" {
                Some(v.trim())
            } else {
                None
            }
        })
        .map_or_else(Hash::default, |h| h.parse::<Hash>().unwrap_or_default())
}

async fn route_health() -> AxumResult {
    Ok("Ok".into_response())
}

async fn route_context(
    axum::extract::Path(context_hash): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    let ctx: Hash = context_hash.parse()?;

    let config: VmContextConfig = decode(&payload)?;

    app_state.server.context(token, ctx, config).await?;

    Ok("Ok".into_response())
}

async fn route_listen(
    ws: axum::extract::ws::WebSocketUpgrade,
    axum::extract::Path(token): axum::extract::Path<String>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> AxumResult {
    let token: Hash = token.parse()?;

    app_state.server.listen(token.clone()).await?;

    Ok(ws.on_upgrade(|ws| async move {
        use axum::extract::ws::Message::*;
        use futures::{SinkExt, StreamExt};

        let (mut low_send, mut low_recv) = ws.split();
        let (up_send, mut up_recv) = tokio::sync::mpsc::channel(8);

        let hash = Hash::nonce();

        app_state.ws_senders.lock().unwrap().insert(
            hash.clone(),
            WsInfo {
                send: up_send.clone(),
                peer_token: token.clone(),
            },
        );

        up_send.try_send(Binary(hash.clone().into())).unwrap();

        tokio::select! {
            _ = async {
                while let Some(Ok(msg)) = low_recv.next().await {
                    match msg {
                        Ping(b) => {
                            // auto-respond to pings
                            if up_send.send(Pong(b)).await.is_err() {
                                return;
                            }
                            continue;
                        },
                        Pong(_) => continue,
                        // close in all other cases
                        // it is not valid to send data to this websocket
                        _ => return,
                    };
                }
            } => (),
            _ = async {
                while let Some(msg) = up_recv.recv().await {
                    if low_send.send(msg).await.is_err() {
                        return;
                    }
                }
            } => (),
        }
    }))
}

async fn route_send(
    axum::extract::Path((context_hash, peer_hash)): axum::extract::Path<(
        String,
        String,
    )>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    data: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    let ctx: Hash = context_hash.parse()?;
    let peer_hash: Hash = peer_hash.parse()?;

    let this_hash = match app_state.get_hash_for_token(&token) {
        None => return Err(std::io::ErrorKind::PermissionDenied.with_info(
            "you must have an active listen websocket connection to send messages".into()).into()),
        Some(h) => h,
    };

    let peer_token = match app_state.ws_senders.lock().unwrap().get(&peer_hash)
    {
        None => {
            return Err(
                std::io::Error::from(std::io::ErrorKind::NotFound).into()
            );
        }
        Some(info) => info.peer_token.clone(),
    };

    app_state
        .server
        .send(ctx.clone(), token, peer_token)
        .await?;

    let data = encode(&VmMsg {
        ctx,
        peer: this_hash,
        data,
    })?;

    app_state
        .ws_send(&peer_hash, axum::extract::ws::Message::Binary(data))
        .await?;

    Ok("Ok".into_response())
}

async fn route_status(
    axum::extract::Path(context_hash): axum::extract::Path<String>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> AxumResult {
    let ctx: Hash = context_hash.parse()?;
    let out = app_state.server.status(ctx).await?;
    let out = out.transform(&mut ValueTxToHuman).await?;
    let out =
        serde_json::to_string_pretty(&out).map_err(std::io::Error::other)?;
    Ok(axum::response::Response::builder()
        .header("content-type", "application/json")
        .body(axum::body::Body::from(out))
        .map_err(std::io::Error::other)?)
}

async fn route_auth_chal_req(
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> AxumResult {
    let resp = app_state.server.auth_chal_req().await?;
    let resp = encode(&resp)?;

    Ok(resp.into_response())
}

async fn route_auth_chal_res(
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);

    let res: AuthChalRes = decode(&payload)?;

    app_state.server.auth_chal_res(token, res).await?;

    Ok("Ok".into_response())
}

async fn route_insert(
    axum::extract::Path(context_hash): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    let ctx: Hash = context_hash.parse()?;

    let enc: Arc<VmObjSigned> = decode(&payload)?;

    app_state.server.insert(token, ctx, enc).await?;

    Ok("Ok".into_response())
}

async fn route_select(
    axum::extract::Path(context_hash): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    let ctx: Hash = context_hash.parse()?;

    let enc: VmSelect = decode(&payload)?;

    let res = app_state.server.select(token, ctx, enc).await?;
    let res = encode(&res)?;

    Ok(res.into_response())
}

async fn route_web(
    axum::extract::Path((context_hash, path)): axum::extract::Path<(
        String,
        String,
    )>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> AxumResult {
    let ctx: Hash = context_hash.parse()?;
    let path = format!("/{path}");
    let ident: Hash = path.as_bytes().into();

    let (mime, data) = app_state.server.get_web(ctx, ident).await?;

    Ok(axum::response::Response::builder()
        .header("content-type", mime)
        .body(axum::body::Body::from(data))
        .map_err(std::io::Error::other)?)
}
*/
