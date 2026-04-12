use std::net::SocketAddr;

use anyhow::Result;
use axum::{
    Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};

use crate::{
    core::events::Event,
    tasks::api_server::handlers::{delete_handler, get_handler, put_handler, root_handler},
};

#[derive(Clone)]
pub struct ApiState {
    pub event_tx: mpsc::Sender<Event>,
}

#[derive(Debug)]
pub enum ApiEvent {
    Put {
        key: String,
        value: Vec<u8>,
        respond: oneshot::Sender<Result<()>>,
    },
    Delete {
        key: String,
        respond: oneshot::Sender<Result<()>>,
    },
    Get {
        key: String,
        respond: oneshot::Sender<Result<Option<Vec<u8>>>>,
    },
}

pub struct ApiServer {
    pub join: tokio::task::JoinHandle<Result<()>>,
}

impl ApiServer {
    pub fn spawn(event_tx: mpsc::Sender<Event>, bind_addr: SocketAddr) -> Self {
        let state = ApiState { event_tx };

        let app = Router::new()
            .route("/rkv", get(root_handler))
            .route(
                "/rkv/{*key}",
                put(put_handler).get(get_handler).delete(delete_handler),
            )
            .with_state(state);

        let join = tokio::spawn(async move {
            let listener = TcpListener::bind(bind_addr).await?;
            axum::serve(listener, app).await?;
            Ok(())
        });

        Self { join }
    }
}

#[derive(Debug)]
pub enum ApiError {
    BadKey,
    Unavailable,
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            ApiError::BadKey => StatusCode::BAD_REQUEST,
            ApiError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        status.into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e)
    }
}
