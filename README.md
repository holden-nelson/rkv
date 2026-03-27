# rkv - Distributed KV Store w/ Raft Consensus

`rkv` is a distributed key value store backed by the Raft consensus algorithm.

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

There are also health and cluster info endpoints:

```http
GET /healthz
```

example response:

```json
{
  "node_id": "node-1",
  "role": "leader",
  "term": 42,
  "commit_index": 1234
}
```

and

```http
GET /cluster
```

example response:

```json
[
  {
    "id": "node-1",
    "role": "leader",
    "location": "http://node-1:9091/",
    "term": 42,
    "commit_index": 1234
  },
  {
    "id": "node-2",
    "role": "follower",
    "location": "http:/node-2:9091/",
    "term": 42,
    "commit_index": 1231
  },
  {
    "id": "node-3",
    "role": "follower",
    "location": "http://node-3:9091/",
    "term": 41,
    "commit_index": 1200
  }
]
```

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
