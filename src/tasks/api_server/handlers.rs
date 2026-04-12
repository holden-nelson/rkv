use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use bytes::Bytes;

use percent_encoding::percent_decode_str;
use tokio::sync::oneshot;

use crate::{
    core::events::Event,
    tasks::api_server::server::{ApiError, ApiEvent, ApiState},
};

pub async fn put_handler(
    State(state): State<ApiState>,
    Path(key): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let key = normalize_key(&key)?;
    let (respond_tx, respond_rx) = oneshot::channel();

    state
        .event_tx
        .send(Event::ClientRequestReceived(ApiEvent::Put {
            key,
            value: body.to_vec(),
            respond: respond_tx,
        }))
        .await
        .map_err(|_| ApiError::Unavailable)?;

    respond_rx.await.map_err(|_| ApiError::Unavailable)??;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_handler(
    State(state): State<ApiState>,
    Path(key): Path<String>,
) -> Result<StatusCode, ApiError> {
    let key = normalize_key(&key)?;
    let (respond_tx, respond_rx) = oneshot::channel();

    state
        .event_tx
        .send(Event::ClientRequestReceived(ApiEvent::Delete {
            key,
            respond: respond_tx,
        }))
        .await
        .map_err(|_| ApiError::Unavailable)?;

    respond_rx.await.map_err(|_| ApiError::Unavailable)??;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_handler(
    State(state): State<ApiState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let key = normalize_key(&key)?;
    let (respond_tx, respond_rx) = oneshot::channel();

    state
        .event_tx
        .send(Event::ClientRequestReceived(ApiEvent::Get {
            key,
            respond: respond_tx,
        }))
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let maybe = respond_rx.await.map_err(|_| ApiError::Unavailable)??;

    match maybe {
        Some(bytes) => Ok((StatusCode::OK, bytes)),
        None => Ok((StatusCode::NOT_FOUND, Vec::<u8>::new())),
    }
}

pub async fn root_handler() -> impl IntoResponse {
    StatusCode::BAD_REQUEST
}

fn normalize_key(raw: &str) -> Result<String, ApiError> {
    let decoded = percent_decode_str(raw)
        .decode_utf8()
        .map_err(|_| ApiError::BadKey)?
        .to_string();

    if decoded.is_empty() {
        return Err(ApiError::BadKey);
    }

    Ok(decoded)
}
