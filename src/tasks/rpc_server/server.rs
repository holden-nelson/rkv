use anyhow::Result;
use serde_json::Value;

use std::net::SocketAddr;

use tokio::{net::TcpListener, sync::mpsc};

use crate::{
    core::{
        events::Event,
        net::{JsonRpcRequest, METHOD_APPEND_ENTRIES, METHOD_REQUEST_VOTE, read_frame},
        rpc::{AppendEntries, RequestVote},
    },
    tasks::rpc_server::{
        entries::{handle_append_entries, send_append_entries},
        vote::{handle_request_vote, send_request_vote},
    },
};
pub enum RpcServerCommand {
    RequestVote {
        peer: SocketAddr,
        params: RequestVote,
    },
    AppendEntries {
        peer: SocketAddr,
        params: AppendEntries,
    },
}

pub struct RpcServer {
    pub cmd_tx: mpsc::Sender<RpcServerCommand>,
    pub join: tokio::task::JoinHandle<Result<()>>,
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
                                let event_tx_clone = event_tx.clone();
                                tokio::spawn(async move {
                                    let response = send_request_vote(peer, params).await.unwrap();
                                    let _ = event_tx_clone.send(Event::VoteReceived(response)).await;
                                });
                            }
                            Some(RpcServerCommand::AppendEntries { peer, params }) => {
                                let event_tx_clone = event_tx.clone();
                                tokio::spawn(async move {
                                    let response = send_append_entries(peer, params).await.unwrap();
                                    let _ = event_tx_clone.send(Event::AppendEntriesResponse(response)).await;
                                });
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
                                    METHOD_REQUEST_VOTE => handle_request_vote(req, &mut stream, &event_tx).await?,
                                    METHOD_APPEND_ENTRIES => handle_append_entries(req, &mut stream, &event_tx).await?,
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
