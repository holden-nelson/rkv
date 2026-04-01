# Node communication - JSON over TCP

Nodes will call and respond to Raft RPCs by connecting over a TCP socket and sending JSON-RPC payloads.

## Connections and Sockets

Each node in the cluster advertises (via the config) where it listens for incoming RPCs. Nodes establish a TCP connection on that port when they want to send a RPC.

This means that the connections ostensibly function unidirectionally - the original sender initiates communication on that socket and the receiver responds. If the receiver, then or later, wants to initiate its own RPC to the original sender, it would do that on the original sender's advertised socket.

Leaders will keep connections alive for fast entry appending and snapshot transfers. Elections are short-lived and closed after vote response.

## Framing

For framing, we will keep it basic: `[u32 payload-length][payload bytes]`

## Correlation

To start, we'll avoid the need for correlation by only sending the next request after receiving the previous response. So peers are all connected in parallel but requests happen serially.

## Max message length

4MB

## Timeouts/retries

RPCs will timeout after 250ms. For AppendEntries calls, we will retry with backoff - 100ms --> 200ms --> 500ms --> 1s, with jitter. For elections, we just wait for the election to timeout if we couldn't declare victory and try again next term.
