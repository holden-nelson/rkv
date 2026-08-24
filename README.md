# Distributed KV Store w/ Raft Consensus

`rkv` is a distributed key value store backed by the Raft consensus algorithm.

## Project Status

This project is currently not in a fully functional state as I dropped it in the middle of a large refactor to pursue other interests.

The original intention of this project was to read the Raft paper and implement it, as I had worked with many technologies backed by Raft but didn't have an intimate understanding of the algorithm. 

Though I only got through 

- TOML cluster config
- Leader election
- Heartbeats
- Disk-backed log with 
  - Indexed records
  - JSON serialization
  - CRC checksums
  - Durable writes
  - Tail recovery
  - Truncation and conflict replacement

I feel as if I now understand Raft well conceptually and have gotten everything out of this project that I need to.

## Architecture
This Raft implementation handles concurrency by implementing a central, asynchronous event loop and pushing effects "to the edges". The various [tasks](src/tasks/) (election timer, heartbeat timer, user-facing api server, node-facing RPC server) all talk to the core loop through tokio channels.

## API

Store and retrieve values with semantic HTTP requests.

```http
PUT /rkv/foo
Body: bar
```

stores `bar` under the key `foo`

```http
GET /rkv/foo
```

returns `bar`.

`rkv` supports slashes in keys:

```http
PUT /rkv/foo/bar
Body: baz
```

stores `baz` under key `foo/bar`.

## Cluster management / configuration

The cluster has an associated configuration file. A full, commented, example starter config is here: [example_cluster_config.toml](example_cluster_config.toml).

A basic config could look like:

```TOML
members = [
  { id = "node1", raft_addr = "127.0.0.1:9001", client_addr = "127.0.0.1:8081" },
  { id = "node2", raft_addr = "127.0.0.1:9002", client_addr = "127.0.0.1:8082" },
  { id = "node3", raft_addr = "127.0.0.1:9003", client_addr = "127.0.0.1:8083" }
]
```
