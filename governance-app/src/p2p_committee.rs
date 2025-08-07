use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;
use chrono;
use sha3::{Digest, Sha3_256};

use hyperware_app_common::send;
use hyperware_app_common::hyperware_process_lib::{our, Address, Request};

use crate::{
    ProposalDraft, Discussion, VoteRecord,
    JoinRequest, JoinResponse, GovernanceEvent,
    P2PMessage, SyncType, SubscriptionFilter,
    SyncResponse, PeerInfo,
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
}

// Simplified CRDT state for governance
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitteeCRDT {
    pub actor_id: String,
    pub vector_clock: VectorClock,
    pub proposals: HashSet<String>,  // Just track proposal IDs
    pub proposal_data: HashMap<String, crate::ProposalData>,  // Full proposal data
    pub vote_records: HashMap<String, HashSet<VoteRecord>>,  // Vote records by proposal
    pub discussions: HashMap<String, Vec<Discussion>>,
    pub committee_members: HashSet<String>,
    pub event_log: Vec<GovernanceEvent>,
    pub merkle_root: String,
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
        }
    }
}

impl CommitteeCRDT {
    pub fn new(actor_id: String) -> Self {
        Self {
            actor_id,
            vector_clock: VectorClock::new(),
            proposals: HashSet::new(),
            proposal_data: HashMap::new(),
            vote_records: HashMap::new(),
            discussions: HashMap::new(),
            committee_members: HashSet::new(),
            event_log: Vec::new(),
            merkle_root: String::new(),
        }
    }
    
    pub fn apply_event(&mut self, event: GovernanceEvent) -> Result<(), String> {
        self.vector_clock.increment(&self.actor_id);
        
        match event.clone() {
            GovernanceEvent::ProposalCreated(data) => {
                self.proposals.insert(data.id.clone());
                self.proposal_data.insert(data.id.clone(), data);
            }
            GovernanceEvent::VoteCast(vote) => {
                // Track votes by proposal ID
                self.vote_records
                    .entry(vote.proposal_id.clone())
                    .or_insert_with(HashSet::new)
                    .insert(vote);
            }
            GovernanceEvent::DiscussionAdded(msg) => {
                self.discussions
                    .entry(msg.id.clone())
                    .or_insert_with(Vec::new)
                    .push(Discussion {
                        id: msg.id.clone(),
                        proposal_id: msg.id.clone(),
                        parent_id: msg.parent_id.clone(),
                        author: msg.author.clone(),
                        content: msg.content.clone(),
                        timestamp: format!("{:?}", msg.timestamp),
                        upvotes: 0,
                        downvotes: 0,
                        signatures: vec![],
                    });
            }
            GovernanceEvent::CommitteeMemberAdded(member) => {
                self.committee_members.insert(member);
            }
            GovernanceEvent::CommitteeMemberRemoved(member) => {
                self.committee_members.remove(&member);
            }
        }
        
        self.event_log.push(event);
        self.update_merkle_root();
        Ok(())
    }
    
    pub fn merge(&mut self, other: &CommitteeCRDT) {
        // Merge vector clocks
        self.vector_clock.merge(&other.vector_clock);
        
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
        
        // Merge discussions (union)
        for (proposal_id, discussions) in &other.discussions {
            let local_discussions = self.discussions.entry(proposal_id.clone()).or_insert_with(Vec::new);
            for discussion in discussions {
                if !local_discussions.iter().any(|d| d.id == discussion.id) {
                    local_discussions.push(discussion.clone());
                }
            }
        }
        
        // Merge committee members (union)
        for member in &other.committee_members {
            self.committee_members.insert(member.clone());
        }
        
        // Merge event logs
        for event in &other.event_log {
            if !self.event_log.iter().any(|e| {
                // Simple deduplication based on event content
                format!("{:?}", e) == format!("{:?}", event)
            }) {
                self.event_log.push(event.clone());
            }
        }
        
        self.update_merkle_root();
    }
    
    pub fn delta_since(&self, since_clock: &VectorClock) -> Vec<GovernanceEvent> {
        // Return events that happened after the given vector clock
        let mut delta_events = Vec::new();
        
        // Check if there are any new events by comparing vector clocks
        let mut has_new_events = false;
        for (actor, &our_time) in &self.vector_clock.clocks {
            let their_time = since_clock.get(actor);
            if our_time > their_time {
                has_new_events = true;
                break;
            }
        }
        
        if !has_new_events {
            return delta_events;
        }
        
        // For a proper implementation, each event should have its own vector clock
        // For now, we'll use a heuristic: include events that were likely added
        // after the since_clock based on the difference in clock values
        
        // Calculate the minimum event index to include based on clock differences
        let total_their_ops: u64 = since_clock.clocks.values().sum();
        let total_our_ops: u64 = self.vector_clock.clocks.values().sum();
        
        if total_our_ops <= total_their_ops {
            return delta_events;
        }
        
        let events_to_skip = (self.event_log.len() as u64)
            .saturating_sub(total_our_ops - total_their_ops)
            .min(self.event_log.len() as u64);
        
        // Return events after the calculated index
        for event in self.event_log.iter().skip(events_to_skip as usize) {
            delta_events.push(event.clone());
        }
        
        delta_events
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

#[derive(Serialize, Deserialize, Debug)]
pub struct CommitteeState {
    pub crdt: CommitteeCRDT,
    pub peers: HashMap<String, PeerInfo>,
    pub subscriptions: HashMap<String, CommitteeSubscription>,
    pub pending_syncs: VecDeque<(String, SyncType)>,
    pub is_committee_member: bool,
    pub node_id: String,
}

impl CommitteeState {
    pub fn new(node_id: String) -> Self {
        Self {
            crdt: CommitteeCRDT::new(node_id.clone()),
            peers: HashMap::new(),
            subscriptions: HashMap::new(),
            pending_syncs: VecDeque::new(),
            is_committee_member: false,
            node_id,
        }
    }
    
    pub fn handle_join_request(&mut self, request: JoinRequest) -> Result<JoinResponse, String> {
        // Simple acceptance for now - can add validation later
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
                let events = if let Some(clock) = from_clock {
                    self.crdt.delta_since(&clock)
                } else {
                    self.crdt.event_log.clone()
                };
                SyncResponse::Delta(events)
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

pub async fn send_p2p_message(
    target_node: &str,
    message: P2PMessage,
) -> Result<String, String> {
    let target = Address::new(target_node, ("governance", "governance", "sys"));
    
    // Serialize the message directly as bytes
    let request_body = bincode::serialize(&message)
        .map_err(|e| format!("Serialization error: {}", e))?;
    
    let request = Request::to(target)
        .body(request_body)
        .expects_response(30); // 30 second timeout for P2P messages
    
    // Send and await response
    match send::<Vec<u8>>(request).await {
        Ok(response) => {
            // Try to parse the response
            if let Ok(response_str) = String::from_utf8(response) {
                Ok(response_str)
            } else {
                // If not a string, return a success indicator
                Ok("ACK".to_string())
            }
        }
        Err(e) => Err(format!("P2P send failed: {:?}", e))
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