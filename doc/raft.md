## Raft Distributed Consensus Algorithm

- Consensus algorithms allow a collection of machines to work as a coherent group that can survive the failure of its members.
- These algorithms typically arise in the context of _replicated state machines_ - an approach where state machines on a collection of servers compute identical copies of the same state
  - Typically implemented using a replicated log
  - The job of the consensus algorithm is to keep the replicated log consistent
- Consensus algos for practical systems typically have the following properties:
  - They ensure _safety_ (never returning an incorrect result) under all non-Byzantine conditions
  - They are fully _available_ as long as any majority of the servers are online
  - They do not depend on timing to ensure consistency of logs; at worst, faulty clocks and latency can only affect availability, not correctness
  - Commonly, a command completes as soon as a majority of the cluster has responded to a single round of remote procedure calls

### The Raft algorithm

- Raft implements consensus by first electing a _leader_, which has complete responsibility for managing the replicated log.
  - The leader
    - accepts log entries from clients,
    - replicates them on other servers,
    - and tells servers when it is safe to apply log entries to their state machines.
- Raft decomposes the consensus problem into three relatively independent subproblems:
  - **Leader election** - when leader fails
  - **Log replication** - leader accepts log entries from clients and replicates them across the cluster, forcing other logs to agree with its own
  - **Safety** - key property is State Machine Safety Property (see below)
- Raft formally makes these guarantees:
  - **Election Safety** - at most one leader can be elected in a given term
  - **Leader Append-Only** - a leader never overwrites or deleted entries in its log
  - **Log Matching** - if two logs contain an entry with the same index and term, the the logs are identical in all entries up through the given index
  - **Leader Completeness** - if a log entry is committed in a given term, then that entry will be present in the logs of the leaders for all higher-numbered terms
  - **State Machine Safety** - if any server has applied a log entry to its state machine, no other server may apply a different command for the same log index.

- Raft basics
  - Raft _cluster_ contains several servers; five allows two failures
  - At a given time, each server is in one of three states: _leader_, _follower_, or _candidate_.
    - In normal operation, one leader, the rest are followers
    - Followers are passive, only respond to leaders and candidates
    - The leader handles all client requests
  - A _term_ is a unit of time of arbitrary length. Terms are numbered with consecutive integers.
    - Each term begins with an _election_, in which one or more candidates attempt to become leader. Whichever candidate wins is leader for the rest of the term.
    - Different servers may observe the transition between terms at different times, and in some situations a server may not observe an election or even entire terms.
    - Each server stores its own `current_term` number, and sends it along with all communication
  - Servers communicate via RPC calls
    - Basic algo requires only two types of RPCs
      - `RequestVote` - initiated by candidates
      - `AppendEntries` - command from leader to append to log (also provides a form of heartbeat)
    - Servers retry RPCs if they do not receive a response in a timely manner and they issue them in parallel

### Leader election

- Raft uses a heartbeat mechanism to trigger leader election
- When servers start up, they begin as followers
- A server remains a follower as long as it is receiving valid RPCs from a leader or candidate
- Leaders send periodic heartbeats (`AppendEntries` with no entries) to maintain authority
- If a follower receives no communication over a certain period of time (the _election timeout_) then it assumes there is no viable leader and begins an election
- Beginning an election:
  - follower increments its current term, becomes candidate
  - Votes for itself
  - Issues `RequestVote` RPCs in parallel to the other servers in cluster

- A candidate continues in this state until
  - it wins the election
  - another server wins the election,
  - a period of time goes by with no winner

- Winning the election:
  - Candidate must receive votes from a majority of the servers in the full cluster for the same term.
  - Each server votes for at most one candidate in a given term, on first-come first-served basis
  - Majority rule ensures election safety
  - Winner sends heartbeats to establish authority
- Another server winning:
  - A candidate may receive an `AppendEntries` RPC from another server claiming to be the leader
  - If the alleged leader's term is at least as large as the candidate's current term, then the candidate recognizes the leader's legitimacy
  - If the term is smaller, the candidate rejects RPC and awaits victory
- No winner:
  - When many followers become candidates at the same time, votes could split and no body obtains majority
  - Raft minimizes election deadlock with randomized election timeouts

### Log Replication

- Client requests contain a command to be executed by the replicated state machines
- Leader
  - appends command to its log as a new entry,
  - then issues `AppendEntries` RPCs in parallel to each of the other servers
  - When the entry has been safely replicated, the leader applies the entry to its state machine and returns the result of that execution to the client
  - If followers crash or run slowly, or if network packets are lost, the leader retries `AppendEntries` RPCs indefinitely (even after consensus is reached and the client's request is satisfied) until all followers store all log entries.
- Log entries store
  - a state machine command,
  - a term number denoting in which term the entry was received by the leader,
  - and a log index denoting position in the log.
- An entry that has been deemed safe to apply is called _committed_.
  - Raft guarantees that all committed entries are durable and will eventually be executed by all available state machines.
  - A log entry is committed once the leader that created the entry has replicated it onto the majority of the servers.
  - This also commits all preceding entries in leader's log, including those created by previous leaders.
  - The leader keeps track of the highest index it knows to be committed, and it includes that index in future `AppendEntries` RPCs so that followers can apply those entries to their own state machines.
- Raft maintains the following properties to ensure safety. Together these constitute the Log Matching Property:
  - If two entries in different logs have the same index and term, then they store the same command
    - Follows from leader creating at most one entry with a given log index in a given term, and that log entries never change their position in the log
  - If two entries in different logs have the same index and term, the the logs are identical in all preceding entries.
    - Guaranteed by consistency check performed by the follower on `AppendEntries`: the leader includes with the `AppendEntries` RPC the index and term of the _preceding_ entry. If the follower does not find an entry in its log matching that preceding index and term, the new entries are refused
      - Acts as an induction step

#### Log Consistency

- Leader crashes can leave the logs inconsistent
  - Leaders may have appended logs that were never able to be committed
  - A follower may be missing entries
  - A follower may have extra uncommitted entries
  - or both
- In Raft, inconsistencies are handled by the leader forcing followers' logs to duplicate its own
  - To do this, leader finds the latest log entry where the two logs agree,
  - deletes entries in follower's log after that point,
  - and then sends follower all of the leader's entries after that point.
- These actions happen in response to consistency check performed by `AppendEntries` RPC.
  - Leader maintains a `nextIndex` for each follower
    - which is the next log entry the leader will send to that follower
  - When a leader comes to power, it inits all `nextIndex` values to the index just after the last one in its log
  - If a follower's log is inconsistent (entry is refused), the leader decrements `nextIndex` for that follower and tries again
  - (If desired, the algo could be improved to make this faster; for example, the follower could include with the rejection the term of the conflicing entry and the first index it stores for that term)

### Safety

#### Election Restriction

- The mechanisms described so far are not quite sufficient to ensure that each state machine executes exactly the same commands in the same order.
  - For example, a follower could be unavailable while the leader commits several log entries, then it is elected leader and overwrites these entries with new ones
- We rectify this by placing restrictions on which servers may be elected leader
- Raft uses the voting process to prevent a candidate from winning an election unless its log contains all committed entries
  - A candidate must contact a majority of the cluster in order to be elected
  - This mathematically guarantees that it will contact a server with all committed entries
  - It follows that if the candidate's log is at least as up to date as any other log in that majority, then it will hold all committed entries
- This restriction is implemented in the `RequestVote` RPC
  - the RPC includes information about the candidate's log
  - the voter denies the vote if its own log is more up to date than the candidate's
  - comparison is made by comparing the index and term of the last entries in the logs
  - if the logs have last entries with different terms, then the one with the later term is more up to date
  - if the logs end with the same term, whichever log is longer is more up to date

#### Committing Previous Entries

- A leader knows that an entry from its current term is committed once that entry is stored on a majority of the servers
- But a leader could crash before comitting an entry. Future leaders will try to commit that entry.
- However, as it stands now, a leader cannot immediately conclude that an entry from a previous term is committed once it is stored on a majority of servers.
- To eliminate this problem, Raft never commits log entries from previous terms by counting replicas. Only log entries from the current term are committed by counting replicas.
  - Once an entry from the current term has been committed in this way, then all prior entries are committed anyways

### Follower and Candidate Crashes

- Much simpler
- If a follower or candidate crashes, future RequestVote and AppendEntries RPCs sent to it will fail
- These are just retried indefinitely
- RPCs are idempotent so no race conditions / critical sections

### Timing and Availability

- Timing does not affect safety

- But it inevitably affects availability

- Timing is particularly critical in regards to leader election

- Raft will be able to elect and maintain a steady leader as long as the system satisfies the following _timing requirement_:

  $$
  broadcastTime \ll electionTimeout \ll MTBF
  $$
  - Where `broadcastTime` is the average time it takes a server to send RPCs in parallel to every server in the cluster and receive responses;
  - `electionTimeout` is the election timeout described above;
  - and `MTBF` is "median time between failures", the average time between failures for a single server

### Cluster membership changes

- Up until now we've assumed the cluster _configuration_ - the set of servers participating in the consensus algorithm - is fixed.
- In practice it is sometimes necessary to change the configuration
- We need a way to do this without taking the entire cluster offline
- For configuration change to be safe, there must never be a point during the transition where it is possible for two leaders to be elected for the same term.
- We then cannot have servers switch directly from the old configuration to the new
  - E.g. there are three servers, and you add two more. At a certain point in time, S1 and S2 are on `C_old` and S3, S4, S5 are on `C_New`. S1 could hold an election and declare victory as soon as it got any other vote, as it thinks there are only 3 servers, not 5
- We instead use a 2 phase approach: the cluster switches to a transitional configuration we call _joint consensus_
  - Log entries are replicated to all servers in both configurations
  - Any server from either configuration may serve as leader
  - Agreement (for elections and entry commitment) requires separate majorities from both the old and new configurations
- Cluster configurations are stored and communicated using special entries in the replicated log
  - When the leader received a request to change the configuration from `C_old` to `C_new`, it stores the configuration for joint consensus as a log entry (`C_old,new`) and replicates it
  - Once a given server adds this entry to its log, it uses that configuration for all future decisions
  - Once `C_old,new` is committed, the leader can create a log entry describing `C_new` and replicate it to the cluster.
  - When `C_new` has been committed, servers not in the new configuration can shut down
- One issue is that new servers may not initially store any log entries. If they are added to the cluster in this state, it could take a while for them to catch up, during which time we could not commit new entries.
  - In order to avoid availability gaps, Raft introduces an additional phase before the configuration change in which new servers join the cluster as non-voting members (the leader replicates log entries to them, but they are not considered for majorities)
  - Once the new servers have caught up with the rest of the cluster, the reconfiguration can proceed as described above
- Another issue is that the cluster leader may not be part of the new configuration. In this case, the leader steps down once it has committed the `C_new` log entry. This means the leader needs to be able to manage a cluster that does not include itself; it replicates log entries but does not include itself in majorities
- The last issue is that removed clusters (those not in `C_new`) can disrupt the cluster. Since they will not receive heartbeats, they will timeout and start new elections
  - This would cause current leaders to revert to follower state
  - To prevent this problem, servers disregard RequestVote RPCs when they believe a current leader exists (i.e. when it received a RequestVote within the minimum election timeout from hearing from a current leader, it does not update its term or grant its vote.)
  - This does not affect normal elections as each server waits at least a minimum election timeout before starting one

## Log compaction

- Raft's log grows during normal operation; in a practical system, it can not grow unbounded
- We need a mechanism to discard obsolete information that has accumulated in the log
- _Snapshotting_ is the simplest approach to compaction; in snapshotting, the entire current system state is written to a snapshot on stable storage; then, the enture log up to that point is discarded.
  - Each server takes snapshots independently
  - The current state of the state machine is written to the snapshot
  - Some metadata as well:
    - The last included index
    - The last included term
    - The latest configuration as of the last included index
- The leader may also occasionally send snapshots to followers that lag behind
  - The leader uses a new RPC called `InstallSnapshot` to send snapshots to followers
  - This snapshot generally supercedes any portion of the log on the follower which the snapshot "covers"
- Servers must decide when to snapshot
  - Too often - wasted disk bandwidth and energy
  - Too infrequently - risks exhaustion of storage capacity and risks log log replays
- Also, writing a snapshot can take a significant amount of time
  - Mitigated by Copy-on-write techniques so new updates can be accepted without impacting the snapshot being written

## Client interaction

- Clients send all of their requests to the leader
- When a client first starts up, it connects to a randomly chosen server. If that is not the leader, the server will reject the client's request and supply information about the most recent leader
- If the leader crashes, clients will again try random servers
