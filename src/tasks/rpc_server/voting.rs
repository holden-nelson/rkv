use anyhow::{Context, Result};
use tokio::{
    net::TcpStream,
    time::{Duration, timeout},
};

use std::net::SocketAddr;

use crate::core::{
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
