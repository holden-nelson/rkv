pub struct RequestVote {
    candidate_id: String,
    term: u64,
    last_index: u64,
    last_term: u64,
}
