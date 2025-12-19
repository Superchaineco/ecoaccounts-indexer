use axum::{
    Json, Router,
    extract::State,
    http::{Request, StatusCode, Method, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use tower_http::cors::{CorsLayer, Any};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

// ============================================================================
// State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Running,
    Paused,
    Reindexing,
}

/// Tracks current indexing progress (normal or reindex)
#[derive(Debug, Clone, Default)]
pub struct IndexState {
    pub from: u64,
    pub to: u64,
    pub current: u64,
    pub strategy: Option<String>,
    pub is_reindex: bool,
}

#[derive(Default)]
pub struct State_ {
    pub status: Status,
    pub last_block: u64,
    pub head: u64,
    pub index: Option<IndexState>,
    pub pending_reindex: Option<IndexState>, // New reindex request waiting to be processed
}

pub struct App {
    pub state: RwLock<State_>,
    paused: AtomicBool,
    api_key: String,
}

impl App {
    pub fn new(api_key: String) -> Arc<Self> {
        Arc::new(Self {
            state: RwLock::new(State_::default()),
            paused: AtomicBool::new(false),
            api_key,
        })
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, v: bool) {
        self.paused.store(v, Ordering::SeqCst);
    }

    /// Check if there's a pending reindex that should interrupt current work
    pub async fn should_interrupt(&self) -> bool {
        self.is_paused() || self.state.read().await.pending_reindex.is_some()
    }
}

// ============================================================================
// Request/Response
// ============================================================================

#[derive(Serialize)]
struct Resp { ok: bool, msg: String }

#[derive(Serialize)]
struct StatusResp {
    status: Status,
    last_block: u64,
    head: u64,
    behind: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<IndexProgress>,
}

#[derive(Serialize)]
struct IndexProgress {
    from: u64,
    to: u64,
    current: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    strategy: Option<String>,
    is_reindex: bool,
}

#[derive(Deserialize, Default)]
pub struct ReindexReq {
    #[serde(default)]
    pub from: Option<u64>,
    #[serde(default)]
    pub to: Option<u64>,
    #[serde(default)]
    pub strategy: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

async fn get_status(State(app): State<Arc<App>>) -> Json<StatusResp> {
    let s = app.state.read().await;
    Json(StatusResp {
        status: s.status,
        last_block: s.last_block,
        head: s.head,
        behind: s.head.saturating_sub(s.last_block),
        index: s.index.as_ref().map(|i| IndexProgress {
            from: i.from,
            to: i.to,
            current: i.current,
            strategy: i.strategy.clone(),
            is_reindex: i.is_reindex,
        }),
    })
}

async fn pause(State(app): State<Arc<App>>) -> Json<Resp> {
    app.set_paused(true);
    app.state.write().await.status = Status::Paused;
    Json(Resp { ok: true, msg: "paused".into() })
}

async fn resume(State(app): State<Arc<App>>) -> Json<Resp> {
    app.set_paused(false);
    let mut s = app.state.write().await;
    s.status = if s.index.as_ref().is_some_and(|i| i.is_reindex) {
        Status::Reindexing
    } else {
        Status::Running
    };
    Json(Resp { ok: true, msg: "resumed".into() })
}

async fn reindex(
    State(app): State<Arc<App>>,
    body: Option<Json<ReindexReq>>,
) -> Result<Json<Resp>, StatusCode> {
    let req = body.map(|b| b.0).unwrap_or_default();
    
    if matches!((req.from, req.to), (Some(f), Some(t)) if f > t) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Set pending reindex - will interrupt current indexing
    let mut s = app.state.write().await;
    s.pending_reindex = Some(IndexState {
        from: req.from.unwrap_or(0),
        to: req.to.unwrap_or(0),
        current: 0,
        strategy: req.strategy,
        is_reindex: true,
    });
    drop(s);
    
    // Wake up if paused
    app.set_paused(false);

    Ok(Json(Resp { ok: true, msg: "reindexing".into() }))
}

// ============================================================================
// Auth & Router
// ============================================================================

async fn auth(
    State(app): State<Arc<App>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for OPTIONS requests (CORS preflight)
    if req.method() == Method::OPTIONS {
        return Ok(next.run(req).await);
    }
    
    match req.headers().get("X-API-Key").and_then(|v| v.to_str().ok()) {
        Some(k) if k == app.api_key => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub fn router(app: Arc<App>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::HeaderName::from_static("x-api-key")]);

    Router::new()
        .route("/status", get(get_status))
        .route("/pause", post(pause))
        .route("/resume", post(resume))
        .route("/reindex", post(reindex))
        .layer(middleware::from_fn_with_state(app.clone(), auth))
        .layer(cors)
        .with_state(app)
}
