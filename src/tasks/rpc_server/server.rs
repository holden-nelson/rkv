use anyhow::{Context, Result};
use serde_json::Value;

use std::net::SocketAddr;

use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};

use crate::{
    core::{
        events::Event,
        net::{JsonRpcRequest, JsonRpcResponse, METHOD_REQUEST_VOTE, read_frame, write_frame},
        rpc::RequestVote,
    },
    tasks::rpc_server::voting::send_request_vote,
};
pub enum RpcServerCommand {
    RequestVote {
        peer: SocketAddr,
        params: RequestVote,
    },
}

pub struct RpcServer {
    cmd_tx: mpsc::Sender<RpcServerCommand>,
    join: tokio::task::JoinHandle<Result<()>>,
}

impl RpcServer {
    pub fn spawn(event_tx: mpsc::Sender<Event>, bind_addr: SocketAddr) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<RpcServerCommand>(64);

        let join = tokio::spawn(async move {
            let listener = TcpListener::bind(bind_addr).await?;

            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(RpcServerCommand::RequestVote { peer, params }) => {
                                let response = send_request_vote(peer, params).await?;
                                event_tx.send(Event::VoteReceived(response)).await?;
                            }
                            _ => break
                        }
                    }

                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((mut stream, _addr)) => {
                                let Some(frame) = read_frame(&mut stream).await? else {
                                    return Ok(());
                                };

                                let req: JsonRpcRequest<Value> = JsonRpcRequest::from_bytes(&frame)?;

                                match req.method.as_str() {
                                    METHOD_REQUEST_VOTE => {
                                        let Some(id) = req.id else {
                                            return Ok(());
                                        };

                                        let params = req.params.context("RequestVote missing params")?;

                                        let vote_req: RequestVote = serde_json::from_value(params)
                                            .context("decode RequestVote params")?;

                                        let (respond_tx, respond_rx) = oneshot::channel();

                                        event_tx.send(Event::VoteRequestReceived {
                                            request: vote_req,
                                            respond: respond_tx
                                        }).await?;

                                        let vote_resp = respond_rx.await.context("core dropped response channel")?;

                                        let resp = JsonRpcResponse::new(id, vote_resp);

                                        let bytes = resp.to_bytes()?;
                                        write_frame(&mut stream, &bytes).await?;
                                    }
                                    _ => break
                                }
                            }
                            Err(_) => break
                        }
                    }
                }
            }

            Ok(())
        });

        Self { cmd_tx, join }
    }
}
