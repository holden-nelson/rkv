[x] read Raft paper
[x] decide api for kv store
[ ] basic main.rs - parse config file and instantiate state
[ ] implement `/healthz` and `/cluster` endpoints
[ ] determine raft log entry format
[ ] determine request / response contract for raft RPCs
[ ] implement AppendEntries
[ ] implement raft entry --> kv entry
[ ] implement RequestVote RPC
[ ] implement redirects for requests to followers
[ ] implement log compaction / snapshotting
[ ] implement InstallSnapshot RPC
[ ] implement cluster membership changes
