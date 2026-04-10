use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::{
    net::TcpStream,
    sync::{mpsc::Sender, oneshot},
    time::timeout,
};

use crate::core::{
    events::Event,
    net::{JsonRpcRequest, JsonRpcResponse, METHOD_APPEND_ENTRIES, read_frame, write_frame},
    rpc::{AppendEntries, AppendEntriesResponse},
};

pub async fn send_append_entries(
    peer: SocketAddr,
    params: AppendEntries,
) -> Result<AppendEntriesResponse> {
    let mut stream = timeout(Duration::from_millis(250), TcpStream::connect(peer))
        .await
        .context("connect timeout")?
        .with_context(|| format!("connect to {peer}"))?;

    let method = METHOD_APPEND_ENTRIES.to_owned();

    let req = JsonRpcRequest::new(method, Some(params));

    let bytes = req
        .to_bytes()
        .context("encode AppendEntries JSON-RPC request")?;

    timeout(Duration::from_millis(250), write_frame(&mut stream, &bytes))
        .await
        .context("write timeout")?
        .context("write frame")?;

    let frame = timeout(Duration::from_millis(250), read_frame(&mut stream))
        .await
        .context("read timeout")?
        .context("read frame")?
        .context("connection closed before response")?;

    let resp: JsonRpcResponse<AppendEntriesResponse> =
        JsonRpcResponse::from_bytes(&frame).context("decode AppendEntriesResponse")?;

    Ok(resp.result)
}

pub async fn handle_append_entries(
    req: JsonRpcRequest<Value>,
    stream: &mut TcpStream,
    event_tx: &Sender<Event>,
) -> Result<()> {
    let Some(id) = req.id else { return Ok(()) };

    let params = req.params.context("AppendEntries missing params")?;

    let entries_req: AppendEntries =
        serde_json::from_value(params).context("decode AppendEntries params")?;

    let (respond_tx, respond_rx) = oneshot::channel();

    event_tx
        .send(Event::AppendEntriesReceived {
            request: entries_req,
            respond: respond_tx,
        })
        .await?;

    let entries_resp = respond_rx.await.context("core dropped response channel")?;

    let resp = JsonRpcResponse::new(id, entries_resp);

    let bytes = resp.to_bytes()?;
    write_frame(stream, &bytes).await?;

    Ok(())
}
