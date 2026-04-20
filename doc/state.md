# State management

is starting to feel a little heavy handed, and like I have some leaky abstractions or something.

I think what makes more sense is to have more mid-level abstractions that I can ask some sort of context manager for.

Like instead of state vs. context, I should have something like a context manager that can give me

- a ConfigurationManager
- a ReplicationManager
- an NodeLifecycleManager
- a LogManager

The configuration manager would be responsible for

- knowing things about the rest of the cluster
- timeout values
- directory and storage values

The NodeLifecycleManager would be responsible for

- Managing the Follower <--> Candidate <--> Leader lifecycle
- Managing votes and declaring election victory
- Knowing current term

The LogManager would be responsible for

- Handling writes and reads to and from the log
- Knowing things like last logged term / index, etc

The ReplicationManager would be responsible for

- Replicating log entries out to the rest of the cluster
- Managing the current commit index
