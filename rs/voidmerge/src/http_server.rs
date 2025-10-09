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

impl From<std::num::ParseFloatError> for ErrTx {
    fn from(_: std::num::ParseFloatError) -> Self {
        Self(std::io::Error::other("expected f64"))
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
                        if let Ok(v) = axum::http::HeaderValue::from_str(v)
                            && let Ok(k) =
                                axum::http::HeaderName::from_bytes(k.as_bytes())
                        {
                            hdr.insert(k, v);
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
        .route(
            "/{ctx}/_vm_/obj-list/{app_path_prefix}",
            axum::routing::get(route_ctx_obj_list),
        )
        .route(
            "/{ctx}/_vm_/obj-get/{app_path}",
            axum::routing::get(route_ctx_obj_get),
        )
        .route(
            "/{ctx}/_vm_/obj-put/{app_path}/{created_secs}/{expires_secs}",
            axum::routing::put(route_ctx_obj_put),
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

fn list_limit_default() -> f64 {
    1000.0
}

#[derive(serde::Deserialize)]
struct ObjListQuery {
    #[serde(rename = "created-gt", default)]
    created_gt: f64,
    #[serde(default = "list_limit_default")]
    limit: f64,
}

#[derive(serde::Serialize)]
struct ObjListOutput {
    #[serde(rename = "metaList")]
    meta_list: Vec<crate::obj::ObjMeta>,
}

async fn route_ctx_obj_list(
    headers: axum::http::HeaderMap,
    axum::extract::Path((ctx, app_path_prefix)): axum::extract::Path<(
        String,
        String,
    )>,
    axum::extract::Query(query): axum::extract::Query<ObjListQuery>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
) -> AxumResult {
    let token = auth_token(&headers);
    let limit = query.limit.clamp(0.0, 1000.0).floor() as u32;
    let result = state
        .server
        .obj_list(
            token,
            ctx.into(),
            app_path_prefix.into(),
            query.created_gt,
            limit,
        )
        .await?;
    Ok(
        bytes::Bytes::from_encode(&ObjListOutput { meta_list: result })?
            .into_response(),
    )
}

#[derive(serde::Serialize)]
struct ObjGetOutput {
    meta: crate::obj::ObjMeta,
    data: bytes::Bytes,
}

async fn route_ctx_obj_get(
    headers: axum::http::HeaderMap,
    axum::extract::Path((ctx, app_path)): axum::extract::Path<(String, String)>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
) -> AxumResult {
    let token = auth_token(&headers);
    let (meta, data) =
        state.server.obj_get(token, ctx.into(), app_path).await?;
    Ok(
        bytes::Bytes::from_encode(&ObjGetOutput { meta, data })?
            .into_response(),
    )
}

async fn route_ctx_obj_put(
    headers: axum::http::HeaderMap,
    axum::extract::Path((ctx, app_path, created_secs, expires_secs)): axum::extract::Path<(String, String, String, String)>,
    axum::extract::ConnectInfo(_addr): axum::extract::ConnectInfo<
        std::net::SocketAddr,
    >,
    axum::extract::State(state): axum::extract::State<Arc<State>>,
    payload: bytes::Bytes,
) -> AxumResult {
    let token = auth_token(&headers);
    let meta = state
        .server
        .obj_put(
            token,
            ctx.into(),
            app_path,
            created_secs.parse()?,
            expires_secs.parse()?,
            payload,
        )
        .await?;
    Ok(meta.0.to_string().into_response())
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
        path,
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
        path,
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
