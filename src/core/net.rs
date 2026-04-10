use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

fn json_rpc_2_0() -> String {
    "2.0".to_owned()
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(u64),
    String(String),
    Null,
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcRequest<T> {
    #[serde(default = "json_rpc_2_0")]
    pub jsonrpc: String,

    pub method: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<T>,

    pub id: Option<JsonRpcId>,
}

impl<T> JsonRpcRequest<T> {
    pub fn new(method: String, params: Option<T>) -> Self {
        JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            method,
            params,
            id: Some(JsonRpcId::Number(1)),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>>
    where
        T: Serialize,
    {
        serde_json::to_vec(self).context("serialize JsonRpcRequest to JSON bytes")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<JsonRpcRequest<T>>
    where
        T: DeserializeOwned,
    {
        serde_json::from_slice(bytes).context("deserialize JsonRpcRequest from JSON bytes")
    }
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcResponse<R> {
    #[serde(default = "json_rpc_2_0")]
    pub jsonrpc: String,

    pub id: JsonRpcId,

    pub result: R,
}

impl<R> JsonRpcResponse<R> {
    pub fn new(id: JsonRpcId, result: R) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id,
            result,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>>
    where
        R: Serialize,
    {
        serde_json::to_vec(self).context("serialize JsonRpcResponse to JSON bytes")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<JsonRpcResponse<R>>
    where
        R: DeserializeOwned,
    {
        serde_json::from_slice(bytes).context("deserialize JsonRpcResponse from JSON bytes")
    }
}

const MAX_FRAME_LEN: u32 = 4 * 1024 * 1024;

pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, payload: &[u8]) -> io::Result<()> {
    let len: u32 = payload
        .len()
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frame length overflows u32"))?;

    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }

    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(payload).await?;
    w.flush().await?;

    Ok(())
}

pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];

    match r.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_be_bytes(len_buf);
    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }

    let mut payload = vec![0u8; len as usize];
    r.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

pub const METHOD_REQUEST_VOTE: &str = "raft.request_vote";
pub const METHOD_APPEND_ENTRIES: &str = "raft.append_entries";
