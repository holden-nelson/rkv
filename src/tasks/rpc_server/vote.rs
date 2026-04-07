use anyhow::{Context, Result};
use serde_json::Value;
use tokio::{
    net::TcpStream,
    sync::{mpsc::Sender, oneshot},
    time::{Duration, timeout},
};

use std::net::SocketAddr;

use crate::core::{
    events::Event,
    net::{JsonRpcRequest, JsonRpcResponse, METHOD_REQUEST_VOTE, read_frame, write_frame},
    rpc::{RequestVote, RequestVoteResponse},
};

pub async fn send_request_vote(
    peer: SocketAddr,
    params: RequestVote,
) -> Result<RequestVoteResponse> {
    let mut stream = timeout(Duration::from_millis(250), TcpStream::connect(peer))
        .await
        .context("connect timeout")?
        .with_context(|| format!("connect to {peer}"))?;

    let method = METHOD_REQUEST_VOTE.to_owned();

    let req = JsonRpcRequest::new(method, Some(params));

    let bytes = req
        .to_bytes()
        .context("encode RequestVote JSON-RPC request")?;

    timeout(Duration::from_millis(250), write_frame(&mut stream, &bytes))
        .await
        .context("write timeout")?
        .context("write frame")?;

    let frame = timeout(Duration::from_millis(250), read_frame(&mut stream))
        .await
        .context("read timeout")?
        .context("read frame")?
        .context("connection closed before response")?;

    let resp: JsonRpcResponse<RequestVoteResponse> =
        JsonRpcResponse::from_bytes(&frame).context("decode RequestVote response")?;

    Ok(resp.result)
}

pub async fn handle_request_vote(
    req: JsonRpcRequest<Value>,
    stream: &mut TcpStream,
    event_tx: &Sender<Event>,
) -> Result<()> {
    let Some(id) = req.id else {
        return Ok(());
    };

    let params = req.params.context("RequestVote missing params")?;

    let vote_req: RequestVote =
        serde_json::from_value(params).context("decode RequestVote params")?;

    let (respond_tx, respond_rx) = oneshot::channel();

    event_tx
        .send(Event::VoteRequestReceived {
            request: vote_req,
            respond: respond_tx,
        })
        .await?;

    let vote_resp = respond_rx.await.context("core dropped response channel")?;

    let resp = JsonRpcResponse::new(id, vote_resp);

    let bytes = resp.to_bytes()?;
    write_frame(stream, &bytes).await?;

    Ok(())
}
