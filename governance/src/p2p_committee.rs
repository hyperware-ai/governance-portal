use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;
use chrono;
use sha3::{Digest, Sha3_256};

use hyperware_app_common::hyperware_process_lib::{our, Address};

use crate::{
    ProposalDraft, Discussion, VoteRecord,
    JoinRequest, JoinResponse, GovernanceEvent,
    P2PMessage, SyncType, SubscriptionFilter,
    SyncResponse, PeerInfo, Message, HLCTimestamp,
};

// ===== Constants =====
const COMMITTEE_QUORUM: f64 = 0.51; // 51% for committee decisions
const KEEPALIVE_INTERVAL_MS: u64 = 60_000; // 1 minute
const KEEPALIVE_TIMEOUT_MS: u64 = 300_000; // 5 minutes
const MAX_SUBSCRIPTIONS: usize = 100;
const MAX_SUBSCRIPTION_QUEUE: usize = 1000;
const SYNC_BATCH_SIZE: usize = 100;

// Use P2PMessage types from lib.rs

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SubscriptionUpdateData {
    ProposalCreated(ProposalDraft),
    ProposalUpdated(ProposalDraft),
    VoteCast(VoteRecord),
    DiscussionAdded(Discussion),
    CommitteeMemberAdded(String),
    CommitteeMemberRemoved(String),
}

// ===== CRDT Operation Types =====

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CRDTOperation {
    pub id: String,  // Unique operation ID
    pub op_type: OperationType,
    pub actor: String,
    pub timestamp: u64,
    pub vector_clock: VectorClock,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OperationType {
    AddProposal { id: String, data: crate::ProposalData },
    UpdateProposal { id: String, data: crate::ProposalData },
    CastVote { proposal_id: String, vote: VoteRecord },
    AddDiscussion { proposal_id: String, discussion: Discussion },
    AddMember { member: String },
    RemoveMember { member: String },
}

impl CRDTOperation {
    pub fn from_event(event: GovernanceEvent, actor: &str, clock: &VectorClock) -> Self {
        let id = format!("{}-{}-{}",
            actor,
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            uuid::Uuid::new_v4()
        );

        let op_type = match event {
            GovernanceEvent::ProposalCreated(data) => {
                OperationType::AddProposal { id: data.id.clone(), data }
            }
            GovernanceEvent::VoteCast(vote) => {
                OperationType::CastVote {
                    proposal_id: vote.proposal_id.clone(),
                    vote
                }
            }
            GovernanceEvent::DiscussionAdded(msg) => {
                OperationType::AddDiscussion {
                    proposal_id: msg.id.clone(),
                    discussion: Discussion {
                        id: msg.id.clone(),
                        proposal_id: msg.id.clone(),
                        parent_id: msg.parent_id,
                        author: msg.author,
                        content: msg.content,
                        timestamp: format!("{:?}", msg.timestamp),
                        upvotes: 0,
                        downvotes: 0,
                        signatures: vec![],
                    }
                }
            }
            GovernanceEvent::CommitteeMemberAdded(member) => {
                OperationType::AddMember { member }
            }
            GovernanceEvent::CommitteeMemberRemoved(member) => {
                OperationType::RemoveMember { member }
            }
        };

        Self {
            id,
            op_type,
            actor: actor.to_string(),
            timestamp: chrono::Utc::now().timestamp() as u64,
            vector_clock: clock.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimestampedEvent {
    pub event: GovernanceEvent,
    pub timestamp: u64,
    pub vector_clock: VectorClock,
    pub actor: String,
}

impl TimestampedEvent {
    pub fn hash(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(format!("{:?}-{}-{}", self.event, self.timestamp, self.actor).as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StateDelta {
    pub operations: Vec<CRDTOperation>,
    pub from_clock: VectorClock,
    pub to_clock: VectorClock,
    pub compressed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CRDTSnapshot {
    pub proposals: HashSet<String>,
    pub proposal_data: HashMap<String, crate::ProposalData>,
    pub vote_records: HashMap<String, HashSet<VoteRecord>>,
    pub discussions: HashMap<String, Vec<Discussion>>,
    pub committee_members: HashSet<String>,
    pub vector_clock: VectorClock,
    pub merkle_root: String,
    pub timestamp: u64,
}

// ===== CRDT Components =====

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VectorClock {
    clocks: HashMap<String, u64>,
}

impl Default for VectorClock {
    fn default() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }
}

impl VectorClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment(&mut self, actor: &str) {
        *self.clocks.entry(actor.to_string()).or_insert(0) += 1;
    }

    pub fn get(&self, actor: &str) -> u64 {
        *self.clocks.get(actor).unwrap_or(&0)
    }

    pub fn merge(&mut self, other: &VectorClock) {
        for (actor, &clock) in &other.clocks {
            let entry = self.clocks.entry(actor.clone()).or_insert(0);
            *entry = (*entry).max(clock);
        }
    }

    pub fn happens_before(&self, other: &VectorClock) -> bool {
        for (actor, &clock) in &self.clocks {
            if clock > other.get(actor) {
                return false;
            }
        }
        true
    }

    pub fn to_vec(&self) -> Vec<(String, u64)> {
        self.clocks.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
}

// Complete CRDT state for governance with proper operations
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitteeCRDT {
    pub actor_id: String,
    pub vector_clock: VectorClock,
    pub proposals: HashSet<String>,  // Track proposal IDs
    pub proposal_data: HashMap<String, crate::ProposalData>,  // Full proposal data
    pub vote_records: HashMap<String, HashSet<VoteRecord>>,  // Vote records by proposal
    pub discussions: HashMap<String, Vec<Discussion>>,
    pub committee_members: HashSet<String>,
    pub event_log: Vec<TimestampedEvent>,  // Events with timestamps for ordering
    pub merkle_root: String,
    pub operations_log: Vec<CRDTOperation>,  // Track all operations for delta sync
    pub garbage_collection_timestamp: u64,  // Track last GC time
}

impl Default for CommitteeCRDT {
    fn default() -> Self {
        Self {
            actor_id: String::new(),
            vector_clock: VectorClock::new(),
            proposals: HashSet::new(),
            proposal_data: HashMap::new(),
            vote_records: HashMap::new(),
            discussions: HashMap::new(),
            committee_members: HashSet::new(),
            event_log: Vec::new(),
            merkle_root: String::new(),
            operations_log: Vec::new(),
            garbage_collection_timestamp: 0,
        }
    }
}

impl CommitteeCRDT {
    pub fn new(actor_id: String) -> Self {
        // Initialize default committee members based on simulation-mode feature
        let default_committee: HashSet<String> = if cfg!(feature = "simulation-mode") {
            HashSet::from_iter(vec!["fake.os".to_string()])
        } else {
            HashSet::from_iter(vec![
                "nick.hypr".to_string(),
                "nick1udwig.os".to_string(),
            ])
        };

        Self {
            actor_id,
            vector_clock: VectorClock::new(),
            proposals: HashSet::new(),
            proposal_data: HashMap::new(),
            vote_records: HashMap::new(),
            discussions: HashMap::new(),
            committee_members: default_committee,
            event_log: Vec::new(),
            merkle_root: String::new(),
            operations_log: Vec::new(),
            garbage_collection_timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }

    pub fn apply_event(&mut self, event: GovernanceEvent) -> Result<(), String> {
        self.vector_clock.increment(&self.actor_id);

        // Create CRDT operation from event
        let operation = CRDTOperation::from_event(event.clone(), &self.actor_id, &self.vector_clock);

        // Apply the operation
        self.apply_operation(operation.clone())?;

        // Log the operation for delta sync
        self.operations_log.push(operation);

        // Add to timestamped event log
        let timestamped = TimestampedEvent {
            event: event.clone(),
            timestamp: chrono::Utc::now().timestamp() as u64,
            vector_clock: self.vector_clock.clone(),
            actor: self.actor_id.clone(),
        };
        self.event_log.push(timestamped);

        self.update_merkle_root();
        Ok(())
    }

    pub fn apply_operation(&mut self, op: CRDTOperation) -> Result<(), String> {
        match op.op_type {
            OperationType::AddProposal { id, data } => {
                self.proposals.insert(id.clone());
                self.proposal_data.insert(id, data);
            }
            OperationType::UpdateProposal { id, data } => {
                // Last-write-wins for proposal updates
                if let Some(existing) = self.proposal_data.get(&id) {
                    if data.voting_start > existing.voting_start {
                        self.proposal_data.insert(id, data);
                    }
                } else {
                    self.proposal_data.insert(id, data);
                }
            }
            OperationType::CastVote { proposal_id, vote } => {
                self.vote_records
                    .entry(proposal_id)
                    .or_insert_with(HashSet::new)
                    .insert(vote);
            }
            OperationType::AddDiscussion { proposal_id, discussion } => {
                self.discussions
                    .entry(proposal_id)
                    .or_insert_with(Vec::new)
                    .push(discussion);
            }
            OperationType::AddMember { member } => {
                self.committee_members.insert(member);
            }
            OperationType::RemoveMember { member } => {
                self.committee_members.remove(&member);
            }
        }
        Ok(())
    }

    pub fn merge(&mut self, other: &CommitteeCRDT) -> Vec<CRDTOperation> {
        let mut applied_ops = Vec::new();

        // Merge vector clocks
        self.vector_clock.merge(&other.vector_clock);

        // Process operations from other CRDT that we haven't seen
        for op in &other.operations_log {
            if !self.has_operation(op) {
                if let Ok(()) = self.apply_operation(op.clone()) {
                    applied_ops.push(op.clone());
                    self.operations_log.push(op.clone());
                }
            }
        }

        // Merge proposal IDs
        for id in &other.proposals {
            self.proposals.insert(id.clone());
        }

        // Merge proposal data (last-write-wins based on timestamp)
        for (id, data) in &other.proposal_data {
            match self.proposal_data.get(id) {
                Some(existing) if existing.voting_start >= data.voting_start => {}
                _ => {
                    self.proposal_data.insert(id.clone(), data.clone());
                }
            }
        }

        // Merge vote records (set union with deduplication)
        for (proposal_id, votes) in &other.vote_records {
            let local_votes = self.vote_records
                .entry(proposal_id.clone())
                .or_insert_with(HashSet::new);
            for vote in votes {
                local_votes.insert(vote.clone());
            }
        }

        // Merge discussions (union with deduplication)
        for (proposal_id, discussions) in &other.discussions {
            let local_discussions = self.discussions
                .entry(proposal_id.clone())
                .or_insert_with(Vec::new);
            for discussion in discussions {
                if !local_discussions.iter().any(|d| d.id == discussion.id) {
                    local_discussions.push(discussion.clone());
                }
            }
            // Sort discussions by timestamp for consistent ordering
            local_discussions.sort_by_key(|d| d.timestamp.clone());
        }

        // Merge committee members (union)
        for member in &other.committee_members {
            self.committee_members.insert(member.clone());
        }

        // Merge event logs with proper deduplication
        let mut seen_events = HashSet::new();
        for event in &self.event_log {
            seen_events.insert(event.hash());
        }

        for event in &other.event_log {
            if !seen_events.contains(&event.hash()) {
                self.event_log.push(event.clone());
            }
        }

        // Sort event log by timestamp and vector clock
        self.event_log.sort_by(|a, b| {
            match a.timestamp.cmp(&b.timestamp) {
                std::cmp::Ordering::Equal => {
                    // If timestamps are equal, use vector clock for ordering
                    a.actor.cmp(&b.actor)
                }
                other => other
            }
        });

        self.update_merkle_root();
        applied_ops
    }

    fn has_operation(&self, op: &CRDTOperation) -> bool {
        self.operations_log.iter().any(|existing| existing.id == op.id)
    }

    pub fn delta_since(&self, since_clock: &VectorClock) -> StateDelta {
        let mut operations = Vec::new();

        // Find all operations that happened after 'since' vector clock
        for op in &self.operations_log {
            if self.operation_is_after(op, since_clock) {
                operations.push(op.clone());
            }
        }

        // Sort operations by vector clock for causal ordering
        operations.sort_by(|a, b| {
            // First by timestamp, then by actor ID for determinism
            match a.timestamp.cmp(&b.timestamp) {
                std::cmp::Ordering::Equal => a.actor.cmp(&b.actor),
                other => other
            }
        });

        StateDelta {
            operations,
            from_clock: since_clock.clone(),
            to_clock: self.vector_clock.clone(),
            compressed: false,
        }
    }

    fn operation_is_after(&self, op: &CRDTOperation, since_clock: &VectorClock) -> bool {
        // Check if this operation happened after the given vector clock
        for (actor, &op_time) in &op.vector_clock.clocks {
            let since_time = since_clock.get(actor);
            if op_time > since_time {
                return true;
            }
        }
        false
    }

    pub fn apply_delta(&mut self, delta: StateDelta) -> Result<(), String> {
        // Apply operations from delta in order
        for op in delta.operations {
            if !self.has_operation(&op) {
                self.apply_operation(op.clone())?;
                self.operations_log.push(op);
            }
        }

        // Update our vector clock
        self.vector_clock.merge(&delta.to_clock);
        self.update_merkle_root();

        Ok(())
    }

    // Garbage collection for old operations
    pub fn garbage_collect(&mut self, cutoff_timestamp: u64) -> usize {
        let initial_count = self.operations_log.len();

        // Remove operations older than cutoff
        self.operations_log.retain(|op| op.timestamp > cutoff_timestamp);

        // Clean up old events
        self.event_log.retain(|event| event.timestamp > cutoff_timestamp);

        // Update GC timestamp
        self.garbage_collection_timestamp = chrono::Utc::now().timestamp() as u64;

        initial_count - self.operations_log.len()
    }

    pub fn create_snapshot(&self) -> CRDTSnapshot {
        CRDTSnapshot {
            proposals: self.proposals.clone(),
            proposal_data: self.proposal_data.clone(),
            vote_records: self.vote_records.clone(),
            discussions: self.discussions.clone(),
            committee_members: self.committee_members.clone(),
            vector_clock: self.vector_clock.clone(),
            merkle_root: self.merkle_root.clone(),
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }

    pub fn restore_from_snapshot(&mut self, snapshot: CRDTSnapshot) {
        self.proposals = snapshot.proposals;
        self.proposal_data = snapshot.proposal_data;
        self.vote_records = snapshot.vote_records;
        self.discussions = snapshot.discussions;
        self.committee_members = snapshot.committee_members;
        self.vector_clock = snapshot.vector_clock;
        self.merkle_root = snapshot.merkle_root;

        // Clear operations log as we're starting fresh from snapshot
        self.operations_log.clear();
        self.event_log.clear();
    }

    fn update_merkle_root(&mut self) {
        let mut hasher = Sha3_256::new();
        hasher.update(format!("{:?}", self.proposals).as_bytes());
        hasher.update(format!("{:?}", self.proposal_data).as_bytes());
        hasher.update(format!("{:?}", self.discussions).as_bytes());
        hasher.update(format!("{:?}", self.committee_members).as_bytes());
        let result = hasher.finalize();
        self.merkle_root = hex::encode(result);
    }

    pub fn compute_state_hash(&self) -> String {
        self.merkle_root.clone()
    }
}

// ===== Committee State Management =====

// Local subscription type for committee
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitteeSubscription {
    pub id: String,
    pub subscriber: String,
    pub filter: SubscriptionFilter,
    pub created_at: u64,
    pub last_update: u64,
    pub sequence: u64,
}

// ===== Consensus Types =====

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ConsensusState {
    pub pending_proposals: Vec<ConsensusProposal>,
    pub voted_proposals: HashSet<String>,
    pub completed_proposals: HashSet<String>,
    pub pending_commits: HashMap<String, ConsensusProposal>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsensusProposal {
    pub id: String,
    pub proposal_type: ConsensusType,
    pub proposer: String,
    pub timestamp: u64,
    pub signatures: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ConsensusType {
    StateUpdate { event: GovernanceEvent },
    AddMember { node_id: String, public_key: Vec<u8> },
    RemoveMember { node_id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PendingOperation {
    pub operation: CRDTOperation,
    pub received_from: String,
    pub timestamp: u64,
}

// ===== State Request/Response Types =====

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StateRequestType {
    FullState,
    DeltaSync { since_clock: VectorClock },
    SpecificProposals { ids: Vec<String> },
    CommitteeInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StateResponseType {
    FullState { data: Vec<u8> },
    Delta { data: Vec<u8> },
    Proposals { data: Vec<u8> },
    CommitteeInfo { data: Vec<u8> },
    RateLimited,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitteeInfo {
    pub members: Vec<String>,
    pub online_count: usize,
    pub quorum_size: usize,
    pub state_hash: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommitteeState {
    pub crdt: CommitteeCRDT,
    pub peers: HashMap<String, PeerInfo>,
    pub subscriptions: HashMap<String, CommitteeSubscription>,
    pub pending_syncs: VecDeque<(String, SyncType)>,
    pub is_committee_member: bool,
    pub node_id: String,
    pub consensus_state: ConsensusState,
    pub pending_operations: VecDeque<PendingOperation>,
    pub snapshot_counter: u64,
    pub last_snapshot_time: u64,
}

impl CommitteeState {
    pub fn new(node_id: String) -> Self {
        let crdt = CommitteeCRDT::new(node_id.clone());

        // Check if this node is a committee member
        let is_committee_member = crdt.committee_members.contains(&node_id);

        Self {
            crdt,
            peers: HashMap::new(),
            subscriptions: HashMap::new(),
            pending_syncs: VecDeque::new(),
            is_committee_member,
            node_id,
            consensus_state: ConsensusState::default(),
            pending_operations: VecDeque::new(),
            snapshot_counter: 0,
            last_snapshot_time: 0,
        }
    }

    pub fn handle_join_request(&mut self, request: JoinRequest) -> Result<JoinResponse, String> {
        // Verify node credentials
        if !self.verify_node_credentials(&request) {
            return Ok(JoinResponse {
                approved: false,
                members: vec![],
                state_hash: String::new(),
                bootstrap_nodes: vec![],
                reason: "Invalid credentials".to_string(),
            });
        }

        // Check if we need committee consensus for adding new member
        if self.is_committee_member && self.crdt.committee_members.len() >= 3 {
            // Initiate consensus for adding member
            let proposal = ConsensusProposal {
                id: uuid::Uuid::new_v4().to_string(),
                proposal_type: ConsensusType::AddMember {
                    node_id: request.node_id.clone(),
                    public_key: request.public_key.clone(),
                },
                proposer: self.node_id.clone(),
                timestamp: chrono::Utc::now().timestamp() as u64,
                signatures: vec![],
            };

            // Queue for consensus
            self.consensus_state.pending_proposals.push(proposal);

            return Ok(JoinResponse {
                approved: false,
                members: vec![],
                state_hash: String::new(),
                bootstrap_nodes: self.get_active_committee_nodes(),
                reason: "Pending committee approval".to_string(),
            });
        }

        // Direct acceptance if we're the only member or not enough for consensus
        self.crdt.committee_members.insert(request.node_id.clone());

        let event = GovernanceEvent::CommitteeMemberAdded(request.node_id.clone());
        self.crdt.apply_event(event)?;

        Ok(JoinResponse {
            approved: true,
            members: self.crdt.committee_members.iter().cloned().collect(),
            state_hash: self.crdt.compute_state_hash(),
            bootstrap_nodes: self.get_active_committee_nodes(),
            reason: String::new(),
        })
    }

    fn verify_node_credentials(&self, request: &JoinRequest) -> bool {
        // TODO: Implement actual credential verification
        // For now, check basic requirements
        !request.node_id.is_empty() && !request.public_key.is_empty()
    }

    pub fn handle_state_update(&mut self, event: GovernanceEvent, vector_clock: VectorClock) -> Result<(), String> {
        // Apply the event to our CRDT
        self.crdt.apply_event(event.clone())?;
        self.crdt.vector_clock.merge(&vector_clock);

        // Broadcast update to subscribers
        self.broadcast_to_subscribers(event)?;

        Ok(())
    }

    pub fn handle_sync_request(&self, sync_type: SyncType, from_clock: Option<VectorClock>) -> SyncResponse {
        match sync_type {
            SyncType::Full => {
                let snapshot = bincode::serialize(&self.crdt).unwrap_or_default();
                SyncResponse::Full(snapshot)
            }
            SyncType::Delta(_) => {
                if let Some(clock) = from_clock {
                    let delta = self.crdt.delta_since(&clock);
                    // Convert delta operations to events
                    let events: Vec<GovernanceEvent> = delta.operations.into_iter()
                        .filter_map(|op| match op.op_type {
                            OperationType::AddProposal { id: _, data } => Some(GovernanceEvent::ProposalCreated(data)),
                            OperationType::CastVote { proposal_id: _, vote } => Some(GovernanceEvent::VoteCast(vote)),
                            OperationType::AddDiscussion { proposal_id: _, discussion } => {
                                Some(GovernanceEvent::DiscussionAdded(Message {
                                    id: discussion.id,
                                    author: discussion.author,
                                    content: discussion.content,
                                    timestamp: HLCTimestamp::now(),
                                    parent_id: discussion.parent_id,
                                }))
                            },
                            OperationType::AddMember { member } => Some(GovernanceEvent::CommitteeMemberAdded(member)),
                            OperationType::RemoveMember { member } => Some(GovernanceEvent::CommitteeMemberRemoved(member)),
                            _ => None,
                        })
                        .collect();
                    SyncResponse::Delta(events)
                } else {
                    // For full delta without a clock, send all events
                    let events: Vec<GovernanceEvent> = self.crdt.event_log.iter()
                        .map(|e| e.event.clone())
                        .collect();
                    SyncResponse::Delta(events)
                }
            }
            SyncType::Proposals(ids) => {
                let mut proposals = Vec::new();
                for id in ids {
                    if let Some(proposal) = self.crdt.proposal_data.get(&id) {
                        proposals.push(proposal.clone());
                    }
                }
                SyncResponse::Proposals(proposals)
            }
        }
    }

    pub fn handle_subscription(&mut self, subscriber: String, filter: SubscriptionFilter) -> Result<String, String> {
        if self.subscriptions.len() >= MAX_SUBSCRIPTIONS {
            return Err("Maximum subscriptions reached".to_string());
        }

        let subscription_id = Uuid::new_v4().to_string();
        let subscription = CommitteeSubscription {
            id: subscription_id.clone(),
            subscriber,
            filter,
            created_at: chrono::Utc::now().timestamp_millis() as u64,
            last_update: chrono::Utc::now().timestamp_millis() as u64,
            sequence: 0,
        };

        self.subscriptions.insert(subscription_id.clone(), subscription);
        Ok(subscription_id)
    }

    pub fn handle_ping(&mut self, from: String, state_hash: String, vector_clock: VectorClock) -> P2PMessage {
        // Update peer info
        let peer_info = PeerInfo {
            node_id: from.clone(),
            last_seen: chrono::Utc::now().timestamp_millis() as u64,
            state_hash: state_hash.clone(),
            vector_clock: vector_clock.clocks.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            is_committee: self.crdt.committee_members.contains(&from),
        };

        // Check if we need to sync before moving from
        if state_hash != self.crdt.compute_state_hash() {
            self.pending_syncs.push_back((from.clone(), SyncType::Delta(vec![])));
        }

        self.peers.insert(from, peer_info);

        P2PMessage::Pong {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            state_hash: self.crdt.compute_state_hash(),
            vector_clock: self.crdt.vector_clock.clocks.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            peer_list: self.get_active_peers(),
        }
    }

    fn broadcast_to_subscribers(&mut self, event: GovernanceEvent) -> Result<(), String> {
        let to_remove: Vec<String> = Vec::new();

        for (_id, subscription) in &mut self.subscriptions {
            let matches = matches_filter_static(&subscription.filter, &event);
            if matches {
                subscription.sequence += 1;
                subscription.last_update = chrono::Utc::now().timestamp_millis() as u64;

                let _update = match &event {
                    GovernanceEvent::ProposalCreated(data) => {
                        // Convert ProposalData to ProposalDraft for compatibility
                        let draft = ProposalDraft {
                            id: data.id.clone(),
                            author: data.author.clone(),
                            title: data.title.clone(),
                            description: data.description.clone(),
                            created_at: format!("{:?}", data.voting_start),
                            updated_at: format!("{:?}", data.voting_start),
                            signatures: vec![],
                        };
                        SubscriptionUpdateData::ProposalCreated(draft)
                    }
                    GovernanceEvent::VoteCast(vote) => {
                        SubscriptionUpdateData::VoteCast(vote.clone())
                    }
                    GovernanceEvent::DiscussionAdded(msg) => {
                        let discussion = Discussion {
                            id: msg.id.clone(),
                            proposal_id: msg.id.clone(),
                            parent_id: msg.parent_id.clone(),
                            author: msg.author.clone(),
                            content: msg.content.clone(),
                            timestamp: format!("{:?}", msg.timestamp),
                            upvotes: 0,
                            downvotes: 0,
                            signatures: vec![],
                        };
                        SubscriptionUpdateData::DiscussionAdded(discussion)
                    }
                    GovernanceEvent::CommitteeMemberAdded(member) => {
                        SubscriptionUpdateData::CommitteeMemberAdded(member.clone())
                    }
                    GovernanceEvent::CommitteeMemberRemoved(member) => {
                        SubscriptionUpdateData::CommitteeMemberRemoved(member.clone())
                    }
                };

                // In a real implementation, we would send this update to the subscriber
                // For now, we just track that we need to send it
            }
        }

        // Remove failed subscriptions
        for id in to_remove {
            self.subscriptions.remove(&id);
        }

        Ok(())
    }

    fn matches_filter(&self, filter: &SubscriptionFilter, event: &GovernanceEvent) -> bool {
        match filter {
            SubscriptionFilter::AllProposals => {
                matches!(event, GovernanceEvent::ProposalCreated(_))
            }
            SubscriptionFilter::ProposalById(id) => {
                match event {
                    GovernanceEvent::ProposalCreated(data) => data.id == *id,
                    GovernanceEvent::DiscussionAdded(msg) => msg.id == *id,
                    _ => false
                }
            }
            SubscriptionFilter::ProposalsByStatus(_status) => {
                // Would need to check proposal status
                matches!(event, GovernanceEvent::ProposalCreated(_))
            }
            SubscriptionFilter::CommitteeUpdates => {
                matches!(event,
                    GovernanceEvent::CommitteeMemberAdded(_) |
                    GovernanceEvent::CommitteeMemberRemoved(_))
            }
            SubscriptionFilter::Discussions(proposal_id) => {
                match event {
                    GovernanceEvent::DiscussionAdded(msg) => msg.id == *proposal_id,
                    _ => false
                }
            }
        }
    }

    fn get_active_committee_nodes(&self) -> Vec<String> {
        self.peers
            .values()
            .filter(|p| p.is_committee)
            .map(|p| p.node_id.clone())
            .collect()
    }

    fn get_active_peers(&self) -> Vec<String> {
        let cutoff = chrono::Utc::now().timestamp_millis() as u64 - KEEPALIVE_TIMEOUT_MS;
        self.peers
            .values()
            .filter(|p| p.last_seen > cutoff)
            .map(|p| p.node_id.clone())
            .collect()
    }

    pub fn cleanup_inactive_peers(&mut self) {
        let cutoff = chrono::Utc::now().timestamp_millis() as u64 - KEEPALIVE_TIMEOUT_MS;
        self.peers.retain(|_, peer| peer.last_seen > cutoff);
    }

    // ===== Committee Consensus Implementation =====

    pub async fn initiate_consensus(&mut self, event: GovernanceEvent) -> Result<bool, String> {
        if !self.is_committee_member {
            return Err("Not a committee member".to_string());
        }

        let proposal_id = uuid::Uuid::new_v4().to_string();
        let proposal = ConsensusProposal {
            id: proposal_id.clone(),
            proposal_type: ConsensusType::StateUpdate { event },
            proposer: self.node_id.clone(),
            timestamp: chrono::Utc::now().timestamp() as u64,
            signatures: vec![self.sign_proposal(&proposal_id)?],
        };

        // Phase 1: Propose to committee
        let mut votes = vec![self.node_id.clone()]; // Self vote

        for member in &self.crdt.committee_members {
            if member != &self.node_id {
                let msg = P2PMessage::ConsensusPropose {
                    proposal: bincode::serialize(&proposal).unwrap_or_default(),
                };

                if let Ok(response) = send_p2p_message(member, msg).await {
                    if let Ok(vote_msg) = serde_json::from_str::<P2PMessage>(&response) {
                        if let P2PMessage::ConsensusVote { voter, approved, .. } = vote_msg {
                            if approved {
                                votes.push(voter);
                            }
                        }
                    }
                }
            }
        }

        // Check if we have quorum
        let has_quorum = (votes.len() as f64) / (self.crdt.committee_members.len() as f64) >= COMMITTEE_QUORUM;

        if has_quorum {
            // Phase 2: Commit
            let commit_msg = P2PMessage::ConsensusCommit {
                proposal_id: proposal.id.clone(),
                voters: votes.clone(),
            };

            // Broadcast commit to all committee members
            broadcast_to_committee(&self.crdt.committee_members, commit_msg, Some(&self.node_id)).await;

            // Apply to local state
            if let ConsensusType::StateUpdate { event } = proposal.proposal_type {
                self.crdt.apply_event(event)?;
            }

            // Update consensus state
            self.consensus_state.completed_proposals.insert(proposal.id);

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn handle_consensus_propose(&mut self, proposal: ConsensusProposal) -> P2PMessage {
        // Verify proposal signature
        if !self.verify_proposal_signature(&proposal) {
            return P2PMessage::ConsensusVote {
                voter: self.node_id.clone(),
                proposal_id: proposal.id.clone(),
                approved: false,
                signature: vec![],
            };
        }

        // Check if we've already voted on this
        if self.consensus_state.voted_proposals.contains(&proposal.id) {
            return P2PMessage::ConsensusVote {
                voter: self.node_id.clone(),
                proposal_id: proposal.id.clone(),
                approved: false,
                signature: vec![],
            };
        }

        // Evaluate proposal
        let approved = self.evaluate_proposal(&proposal);
        let proposal_id = proposal.id.clone();

        if approved {
            self.consensus_state.voted_proposals.insert(proposal_id.clone());
            self.consensus_state.pending_commits.insert(proposal_id.clone(), proposal);
        }

        let signature = self.sign_vote(&proposal_id, approved).unwrap_or_default();

        P2PMessage::ConsensusVote {
            voter: self.node_id.clone(),
            proposal_id,
            approved,
            signature,
        }
    }

    pub fn handle_consensus_commit(&mut self, proposal_id: String, voters: Vec<String>) -> Result<(), String> {
        // Verify we have the proposal
        let proposal = self.consensus_state.pending_commits
            .remove(&proposal_id)
            .ok_or("Unknown proposal")?;

        // Verify quorum
        let has_quorum = (voters.len() as f64) / (self.crdt.committee_members.len() as f64) >= COMMITTEE_QUORUM;
        if !has_quorum {
            return Err("Insufficient quorum".to_string());
        }

        // Apply the proposal
        match proposal.proposal_type {
            ConsensusType::StateUpdate { event } => {
                self.crdt.apply_event(event)?;
            }
            ConsensusType::AddMember { node_id, .. } => {
                let event = GovernanceEvent::CommitteeMemberAdded(node_id);
                self.crdt.apply_event(event)?;
            }
            ConsensusType::RemoveMember { node_id } => {
                let event = GovernanceEvent::CommitteeMemberRemoved(node_id);
                self.crdt.apply_event(event)?;
            }
        }

        self.consensus_state.completed_proposals.insert(proposal_id);
        Ok(())
    }

    fn evaluate_proposal(&self, proposal: &ConsensusProposal) -> bool {
        // Implement proposal evaluation logic
        match &proposal.proposal_type {
            ConsensusType::StateUpdate { .. } => true, // Accept state updates for now
            ConsensusType::AddMember { .. } => {
                // Check if committee size is reasonable
                self.crdt.committee_members.len() < 20
            }
            ConsensusType::RemoveMember { node_id } => {
                // Don't remove if it would break quorum
                let remaining = self.crdt.committee_members.len() - 1;
                remaining >= 3 && self.crdt.committee_members.contains(node_id)
            }
        }
    }

    fn verify_proposal_signature(&self, _proposal: &ConsensusProposal) -> bool {
        // TODO: Implement actual signature verification
        true
    }

    fn sign_proposal(&self, proposal_id: &str) -> Result<Vec<u8>, String> {
        // TODO: Implement actual signing
        Ok(proposal_id.as_bytes().to_vec())
    }

    fn sign_vote(&self, proposal_id: &str, approved: bool) -> Result<Vec<u8>, String> {
        // TODO: Implement actual signing
        Ok(format!("{}-{}", proposal_id, approved).as_bytes().to_vec())
    }

    // ===== State Sharing with Non-Committee Nodes =====

    pub fn handle_state_request(&self, requester: String, request_type: StateRequestType) -> Result<P2PMessage, String> {
        // Rate limiting check
        if !self.check_rate_limit(&requester) {
            return Ok(P2PMessage::StateResponse {
                response_type: bincode::serialize(&StateResponseType::RateLimited).unwrap_or_default(),
                data: vec![],
            });
        }

        let response = match request_type {
            StateRequestType::FullState => {
                // Send full CRDT state
                let snapshot = self.crdt.create_snapshot();
                let data = bincode::serialize(&snapshot)
                    .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;

                StateResponseType::FullState { data }
            }
            StateRequestType::DeltaSync { since_clock } => {
                // Send delta since vector clock
                let delta = self.crdt.delta_since(&since_clock);
                let data = bincode::serialize(&delta)
                    .map_err(|e| format!("Failed to serialize delta: {}", e))?;

                StateResponseType::Delta { data }
            }
            StateRequestType::SpecificProposals { ids } => {
                // Send specific proposals
                let mut proposals = Vec::new();
                for id in ids {
                    if let Some(proposal) = self.crdt.proposal_data.get(&id) {
                        proposals.push(proposal.clone());
                    }
                }

                let data = serde_json::to_vec(&proposals)
                    .map_err(|e| format!("Failed to serialize proposals: {}", e))?;

                StateResponseType::Proposals { data }
            }
            StateRequestType::CommitteeInfo => {
                // Send committee information
                let info = CommitteeInfo {
                    members: self.crdt.committee_members.iter().cloned().collect(),
                    online_count: self.get_active_committee_nodes().len(),
                    quorum_size: ((self.crdt.committee_members.len() as f64) * COMMITTEE_QUORUM) as usize,
                    state_hash: self.crdt.compute_state_hash(),
                };

                let data = serde_json::to_vec(&info)
                    .map_err(|e| format!("Failed to serialize committee info: {}", e))?;

                StateResponseType::CommitteeInfo { data }
            }
        };

        Ok(P2PMessage::StateResponse {
            response_type: bincode::serialize(&response).unwrap_or_default(),
            data: vec![],
        })
    }

    fn check_rate_limit(&self, _requester: &str) -> bool {
        // TODO: Implement actual rate limiting
        // For now, allow all requests
        true
    }

    // ===== Snapshot Management =====

    pub fn should_create_snapshot(&self) -> bool {
        let now = chrono::Utc::now().timestamp() as u64;
        const SNAPSHOT_INTERVAL: u64 = 3600; // 1 hour

        now - self.last_snapshot_time > SNAPSHOT_INTERVAL
    }

    pub fn create_and_save_snapshot(&mut self, storage: &crate::storage::FileStorage) -> Result<(), String> {
        let _snapshot = self.crdt.create_snapshot();

        // Save to VFS
        storage.save_crdt_snapshot(&self.crdt)?;

        self.snapshot_counter += 1;
        self.last_snapshot_time = chrono::Utc::now().timestamp() as u64;

        // Cleanup old snapshots
        storage.cleanup_old_snapshots(5)?;

        Ok(())
    }
}

// Helper function to avoid borrowing issues
fn matches_filter_static(filter: &SubscriptionFilter, event: &GovernanceEvent) -> bool {
    match filter {
        SubscriptionFilter::AllProposals => {
            matches!(event, GovernanceEvent::ProposalCreated(_))
        }
        SubscriptionFilter::ProposalById(id) => {
            match event {
                GovernanceEvent::ProposalCreated(data) => data.id == *id,
                GovernanceEvent::DiscussionAdded(msg) => msg.id == *id,
                _ => false
            }
        }
        SubscriptionFilter::ProposalsByStatus(_status) => {
            // Would need to check proposal status
            matches!(event, GovernanceEvent::ProposalCreated(_))
        }
        SubscriptionFilter::CommitteeUpdates => {
            matches!(event,
                GovernanceEvent::CommitteeMemberAdded(_) |
                GovernanceEvent::CommitteeMemberRemoved(_))
        }
        SubscriptionFilter::Discussions(proposal_id) => {
            match event {
                GovernanceEvent::DiscussionAdded(msg) => msg.id == *proposal_id,
                _ => false
            }
        }
    }
}

// ===== P2P Communication Helpers =====

// Import the generated RPC stubs and types
use caller_utils::governance::*;
use caller_utils::{
    JoinRequest as CallerJoinRequest,
    StateUpdate as CallerStateUpdate,
    SyncRequest as CallerSyncRequest,
    SyncType as CallerSyncType,
    GovernanceEvent as CallerGovernanceEvent,
    ProposalData as CallerProposalData,
    VoteRecord as CallerVoteRecord,
    HlcTimestamp as CallerHlcTimestamp,
    ProposalStatus as CallerProposalStatus,
    VoteChoice as CallerVoteChoice,
};
use caller_utils::hyperware::process::governance::Message as CallerMessage;

// Helper function to convert our SyncType to caller-utils SyncType
fn convert_sync_type(sync_type: SyncType) -> CallerSyncType {
    match sync_type {
        SyncType::Full => CallerSyncType::Full,
        SyncType::Delta(vector_clock) => CallerSyncType::Delta(vector_clock),
        SyncType::Proposals(ids) => CallerSyncType::Proposals(ids),
    }
}

// Helper function to convert HLCTimestamp (note the capital L and C)
fn convert_hlc_timestamp(ts: crate::HLCTimestamp) -> CallerHlcTimestamp {
    CallerHlcTimestamp {
        wall_time: ts.wall_time,
        logical: ts.logical,
        node_id: ts.node_id,
    }
}

// Helper function to convert ProposalStatus
fn convert_proposal_status(status: crate::ProposalStatus) -> CallerProposalStatus {
    match status {
        crate::ProposalStatus::Pending => CallerProposalStatus::Pending,
        crate::ProposalStatus::Active => CallerProposalStatus::Active,
        crate::ProposalStatus::Canceled => CallerProposalStatus::Canceled,
        crate::ProposalStatus::Defeated => CallerProposalStatus::Defeated,
        crate::ProposalStatus::Succeeded => CallerProposalStatus::Succeeded,
        crate::ProposalStatus::Queued => CallerProposalStatus::Queued,
        crate::ProposalStatus::Expired => CallerProposalStatus::Expired,
        crate::ProposalStatus::Executed => CallerProposalStatus::Executed,
        crate::ProposalStatus::Rejected => CallerProposalStatus::Rejected,
    }
}

// Helper function to convert VoteChoice
fn convert_vote_choice(choice: crate::VoteChoice) -> CallerVoteChoice {
    match choice {
        crate::VoteChoice::Yes => CallerVoteChoice::Yes,
        crate::VoteChoice::No => CallerVoteChoice::No,
        crate::VoteChoice::Abstain => CallerVoteChoice::Abstain,
    }
}

// Helper function to convert GovernanceEvent to caller-utils type
fn convert_governance_event(event: GovernanceEvent) -> CallerGovernanceEvent {
    match event {
        GovernanceEvent::ProposalCreated(data) => {
            CallerGovernanceEvent::ProposalCreated(CallerProposalData {
                id: data.id,
                title: data.title,
                description: data.description,
                author: data.author,
                status: convert_proposal_status(data.status),
                voting_start: convert_hlc_timestamp(data.voting_start),
                voting_end: convert_hlc_timestamp(data.voting_end),
                completion_time: data.completion_time.map(convert_hlc_timestamp),
            })
        },
        GovernanceEvent::VoteCast(vote) => {
            CallerGovernanceEvent::VoteCast(CallerVoteRecord {
                voter: vote.voter,
                proposal_id: vote.proposal_id,
                choice: convert_vote_choice(vote.choice),
                voting_power: vote.voting_power,
                timestamp: convert_hlc_timestamp(vote.timestamp),
            })
        },
        GovernanceEvent::DiscussionAdded(msg) => {
            CallerGovernanceEvent::DiscussionAdded(CallerMessage {
                id: msg.id,
                author: msg.author,
                content: msg.content,
                timestamp: convert_hlc_timestamp(msg.timestamp),
                parent_id: msg.parent_id,
            })
        },
        GovernanceEvent::CommitteeMemberAdded(member) => {
            CallerGovernanceEvent::CommitteeMemberAdded(member)
        },
        GovernanceEvent::CommitteeMemberRemoved(member) => {
            CallerGovernanceEvent::CommitteeMemberRemoved(member)
        },
    }
}

pub async fn send_p2p_message(
    target_node: &str,
    message: P2PMessage,
) -> Result<String, String> {
    let target = Address::new(target_node, crate::OUR_PROCESS);

    // Use the appropriate generated RPC stub based on message type
    match message {
        P2PMessage::JoinRequest { node_id, public_key, capabilities } => {
            let request = CallerJoinRequest { node_id, public_key, capabilities };
            match handle_join_request_remote_rpc(&target, request).await {
                Ok(Ok(response)) => Ok(serde_json::to_string(&response).unwrap_or_else(|_| "ACK".to_string())),
                Ok(Err(e)) => Err(format!("Remote error: {}", e)),
                Err(e) => Err(format!("RPC error: {:?}", e)),
            }
        },
        P2PMessage::StateUpdate { event, vector_clock: _, signature, propagation_path: _ } => {
            // Need to convert GovernanceEvent to caller-utils type
            let caller_event = convert_governance_event(event);
            let update = CallerStateUpdate { event: caller_event, signature };
            match handle_state_update_remote_rpc(&target, update).await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(e)) => Err(format!("Remote error: {}", e)),
                Err(e) => Err(format!("RPC error: {:?}", e)),
            }
        },
        P2PMessage::SyncRequest { sync_type, max_events } => {
            // Need to convert SyncType
            let caller_sync_type = convert_sync_type(sync_type);
            let request = CallerSyncRequest { sync_type: caller_sync_type, max_events };
            match handle_sync_request_remote_rpc(&target, request).await {
                Ok(Ok(response)) => Ok(serde_json::to_string(&response).unwrap_or_else(|_| "ACK".to_string())),
                Ok(Err(e)) => Err(format!("Remote error: {}", e)),
                Err(e) => Err(format!("RPC error: {:?}", e)),
            }
        },
        P2PMessage::Subscribe { subscriber, filter: _, subscription_type: _, delivery_mode: _, .. } => {
            // Note: handle_subscription currently only takes subscriber
            match handle_subscription_remote_rpc(&target, subscriber).await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(e)) => Err(format!("Remote error: {}", e)),
                Err(e) => Err(format!("RPC error: {:?}", e)),
            }
        },
        P2PMessage::Ping { timestamp, state_hash, vector_clock: _, available_capacity: _ } => {
            // Use the keepalive RPC (simplified signature)
            match handle_keepalive_remote_rpc(&target, timestamp, state_hash).await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(e)) => Err(format!("Remote error: {}", e)),
                Err(e) => Err(format!("RPC error: {:?}", e)),
            }
        },
        P2PMessage::Pong { .. } => {
            // Pong is a response, not a request - shouldn't be sent this way
            Err("Pong is a response message, not a request".to_string())
        },
        P2PMessage::ConsensusPropose { .. } |
        P2PMessage::ConsensusVote { .. } |
        P2PMessage::ConsensusCommit { .. } |
        P2PMessage::StateRequest { .. } |
        P2PMessage::StateResponse { .. } => {
            // These are handled elsewhere or not sent via this function
            Err("Message type not handled by send_p2p_message".to_string())
        }
    }
}

pub async fn broadcast_to_committee(
    committee_members: &HashSet<String>,
    message: P2PMessage,
    exclude: Option<&str>,
) -> Vec<Result<String, String>> {
    let our_node = our().node;
    let mut results = Vec::new();

    for member in committee_members {
        if member == &our_node || Some(member.as_str()) == exclude {
            continue;
        }

        let msg_clone = message.clone();
        let result = send_p2p_message(member, msg_clone).await;
        results.push(result);
    }

    results
}

// ===== Keepalive Management =====

use hyperware_app_common::hyperware_process_lib::timer;

pub async fn start_keepalive_timer() {
    // Set a timer for the keepalive interval
    timer::set_timer(KEEPALIVE_INTERVAL_MS, Some(b"keepalive".to_vec()));
}

pub async fn handle_keepalive_tick(state: &mut CommitteeState) {
    // Clean up inactive peers
    state.cleanup_inactive_peers();

    // Send pings to active committee members
    for member in state.crdt.committee_members.clone() {
        if member == state.node_id {
            continue;
        }

        let ping = P2PMessage::Ping {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            state_hash: state.crdt.compute_state_hash(),
            vector_clock: state.crdt.vector_clock.clocks.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            available_capacity: (MAX_SUBSCRIPTIONS - state.subscriptions.len()) as u32,
        };

        // Fire and forget ping
        let _ = send_p2p_message(&member, ping).await;
    }

    // Process pending syncs
    while let Some((peer, sync_type)) = state.pending_syncs.pop_front() {
        let sync_request = P2PMessage::SyncRequest {
            sync_type,
            max_events: Some(100),
        };

        if let Ok(response_str) = send_p2p_message(&peer, sync_request).await {
            if let Ok(sync_response) = serde_json::from_str::<SyncResponse>(&response_str) {
                // Handle the sync response based on type
                match sync_response {
                    SyncResponse::Delta(events) => {
                        for event in events {
                            let _ = state.crdt.apply_event(event);
                        }
                    }
                    SyncResponse::Full(snapshot) => {
                        // Deserialize and apply full snapshot
                        if let Ok(remote_crdt) = bincode::deserialize::<CommitteeCRDT>(&snapshot) {
                            state.crdt.merge(&remote_crdt);
                        }
                    }
                    SyncResponse::Proposals(proposals) => {
                        // Update proposal data
                        for proposal in proposals {
                            state.crdt.proposal_data.insert(proposal.id.clone(), proposal);
                        }
                    }
                }
            }
        }
    }

    // Set timer for next keepalive
    start_keepalive_timer().await;
}
