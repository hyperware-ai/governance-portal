use hyperprocess_macro::*;

// Create a module path that the macro expects
pub mod process_macros {
    // Define SerdeJsonInto trait that the macro expects
    pub trait SerdeJsonInto {
        fn serde_json_into<T: for<'a> serde::Deserialize<'a>>(self) -> Result<T, String>;
    }

    impl SerdeJsonInto for String {
        fn serde_json_into<T: for<'a> serde::Deserialize<'a>>(self) -> Result<T, String> {
            serde_json::from_str(&self).map_err(|e| e.to_string())
        }
    }
}
use hyperware_process_lib::{
    our,
    homepage::add_to_homepage,
    timer,
    logging,
};
use hyperware_app_common::{send, source, SaveOptions};
use hyperware_process_lib::{Request, Address};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono;
use base64::{Engine as _, engine::general_purpose};
use sha3::{Digest, Sha3_256};
use hex;

mod chain_indexer;
mod storage;
mod contracts;
mod p2p_committee;

use chain_indexer::{ChainIndexer, ProposalEvent, process_proposal_event};
use storage::FileStorage;

// Import caller_utils types for P2P communication
use caller_utils::{
    GovernanceEvent as CallerGovernanceEvent,
    ProposalData as CallerProposalData,
    VoteRecord as CallerVoteRecord,
    HlcTimestamp as CallerHlcTimestamp,
    ProposalStatus as CallerProposalStatus,
    VoteChoice as CallerVoteChoice,
    StateUpdate as CallerStateUpdate,
};
use caller_utils::hyperware::process::governance::Message as CallerMessage;
// Temporarily comment out p2p_committee imports to isolate errors
// use p2p_committee::{
//     CommitteeState, send_p2p_message, broadcast_to_committee,
// };

pub const OUR_PROCESS: (&str, &str, &str) = ("governance", "governance", "ware.hypr");

#[derive(Serialize, Deserialize)]
pub struct GovernanceState {
    last_indexed_block: u64,
    chain_id: u64,
    governor_address: String,
    token_address: String,

    onchain_proposals: HashMap<String, OnchainProposal>,

    committee_members: HashSet<String>,
    proposal_drafts: HashMap<String, ProposalDraft>,
    discussions: HashMap<String, Vec<Discussion>>,

    is_committee_member: bool,
    is_indexing: bool,
    sync_peers: Vec<String>,
    last_state_hash: String,

    crdt_state: GovernanceCRDT,
    subscriptions: HashMap<String, Subscription>,
    peers: HashMap<String, PeerInfo>,
    
    // Track processed events to prevent infinite loops
    processed_events: HashSet<String>,
    
    // Count tracking for UI display
    committee_count: usize,
    subscriber_count: usize,
    total_participants: usize,

    // Complete committee state management
    #[serde(skip)]
    committee_state: Option<p2p_committee::CommitteeState>,

    #[serde(skip)]
    chain_indexer: Option<ChainIndexer>,
    #[serde(skip)]
    storage: Option<FileStorage>,
}

impl Default for GovernanceState {
    fn default() -> Self {
        // Set default committee members based on feature flag
        let default_committee: HashSet<String> = if cfg!(feature = "simulation-mode") {
            // Simulation mode: use fake.os
            HashSet::from_iter(vec!["fake.os".to_string()])
        } else {
            // Production mode: use nick.hypr and nick1udwig.os
            HashSet::from_iter(vec![
                "nick.hypr".to_string(),
                "nick1udwig.os".to_string(),
            ])
        };

        // Check if current node is in the default committee
        let our_node = our().node.clone();
        let is_member = default_committee.contains(&our_node);

        let committee_count = default_committee.len();
        
        Self {
            last_indexed_block: 0,
            chain_id: 8453, // Base mainnet
            governor_address: String::new(),
            token_address: String::new(),
            onchain_proposals: HashMap::new(),
            committee_members: default_committee,
            proposal_drafts: HashMap::new(),
            discussions: HashMap::new(),
            is_committee_member: is_member,
            is_indexing: false,
            sync_peers: Vec::new(),
            last_state_hash: String::new(),
            crdt_state: GovernanceCRDT::default(),
            subscriptions: HashMap::new(),
            peers: HashMap::new(),
            processed_events: HashSet::new(),
            committee_count,
            subscriber_count: 0,
            total_participants: committee_count,
            committee_state: None,
            chain_indexer: None,
            storage: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OnchainProposal {
    pub id: String,
    pub proposer: String,
    pub title: String,
    pub description: String,
    pub targets: Vec<String>,
    pub values: Vec<String>,
    pub calldatas: Vec<String>,
    pub start_block: u64,
    pub end_block: u64,
    pub votes_for: String,
    pub votes_against: String,
    pub votes_abstain: String,
    pub status: ProposalStatus,
    pub tx_hash: String,
    pub block_number: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Active,
    Canceled,
    Defeated,
    Succeeded,
    Queued,
    Expired,
    Executed,
    Rejected,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProposalDraft {
    pub id: String,
    pub author: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub signatures: Vec<NodeSignature>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NodeSignature {
    pub node_id: String,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Discussion {
    pub id: String,
    pub proposal_id: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub content: String,
    pub timestamp: String,
    pub upvotes: u32,
    pub downvotes: u32,
    pub signatures: Vec<NodeSignature>,
}

// Type aliases - using String directly for WIT compatibility
type ActorId = String;
type ProposalId = String;
type MessageId = String;
type MemberId = String;

#[derive(Serialize, Deserialize, Clone)]
pub struct GovernanceCRDT {
    actor_id: ActorId,
    vector_clock: HashMap<ActorId, u64>,
    proposals: HashSet<ProposalId>,
    proposal_data: HashMap<ProposalId, ProposalData>,
    votes: HashMap<ProposalId, VoteCounters>,
    vote_records: HashMap<ProposalId, HashSet<VoteRecord>>,
    committee: HashSet<MemberId>,
    discussions: HashMap<ProposalId, DiscussionCRDT>,
    merkle_root: MerkleRoot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProposalData {
    pub id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub status: ProposalStatus,
    pub voting_start: HLCTimestamp,
    pub voting_end: HLCTimestamp,
    pub completion_time: Option<HLCTimestamp>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VoteCounters {
    yes_votes: u64,
    no_votes: u64,
    abstain_votes: u64,
}

impl VoteCounters {
    fn new() -> Self {
        Self {
            yes_votes: 0,
            no_votes: 0,
            abstain_votes: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct VoteRecord {
    pub voter: String,
    pub proposal_id: String,
    pub choice: VoteChoice,
    pub voting_power: u64,
    pub timestamp: HLCTimestamp,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscussionCRDT {
    messages: Vec<Message>,
    message_dag: HashMap<String, Vec<String>>,  // Using String directly instead of MessageId alias
    upvotes: HashMap<String, u32>,  // Using String directly instead of MessageId alias
    downvotes: HashMap<String, u32>,  // Using String directly instead of MessageId alias
}

impl DiscussionCRDT {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            message_dag: HashMap::new(),
            upvotes: HashMap::new(),
            downvotes: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: String,  // Using String directly instead of MessageId alias
    pub author: String,
    pub content: String,
    pub timestamp: HLCTimestamp,
    pub parent_id: Option<String>,  // Using String directly instead of MessageId alias
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HLCTimestamp {
    wall_time: u64,
    logical: u32,
    node_id: String,
}

impl HLCTimestamp {
    fn now() -> Self {
        Self {
            wall_time: chrono::Utc::now().timestamp_millis() as u64,
            logical: 0,
            node_id: our().node.clone(),
        }
    }
}

type MerkleRoot = String;

// Utility functions
fn current_timestamp() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PeerInfo {
    node_id: String,
    last_seen: u64,
    vector_clock: Vec<(String, u64)>,  // Changed from HashMap for WIT compatibility
    state_hash: String,
    is_committee: bool,
}

impl PeerInfo {
    fn is_online(&self) -> bool {
        // Consider a peer online if we've heard from them in the last 5 minutes
        let now = current_timestamp();
        (now - self.last_seen) < 300_000 // 5 minutes in milliseconds
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Subscription {
    id: String,
    subscriber: String,
    subscription_type: SubscriptionType,
    created_at: u64,
    last_update: u64,
    delivery_mode: DeliveryMode,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SubscriptionType {
    AllProposals,
    ProposalById(String),
    ProposalsByStatus(ProposalStatus),
    CommitteeUpdates,
    DiscussionsByProposal(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum DeliveryMode {
    Push,
    Pull,
    Hybrid { push_timeout: u64, pull_interval: u64 },
}

#[derive(Serialize, Deserialize, Clone)]
pub enum P2PMessage {
    JoinRequest {
        node_id: String,
        public_key: Vec<u8>,
        capabilities: Vec<String>,
    },
    StateUpdate {
        event: GovernanceEvent,
        vector_clock: Vec<(String, u64)>,  // Changed from HashMap for WIT compatibility
        signature: Vec<u8>,
        propagation_path: Vec<String>,
    },
    SyncRequest {
        sync_type: SyncType,
        max_events: Option<u32>,
    },
    Subscribe {
        subscriber: String,
        subscription_type: SubscriptionType,
        filter: Option<SubscriptionFilter>,
        delivery_mode: DeliveryMode,
    },
    Ping {
        timestamp: u64,
        state_hash: String,
        vector_clock: Vec<(String, u64)>,  // Changed from HashMap for WIT compatibility
        available_capacity: u32,
    },
    Pong {
        timestamp: u64,
        state_hash: String,
        vector_clock: Vec<(String, u64)>,  // Changed from HashMap for WIT compatibility
        peer_list: Vec<String>,
    },
    // Consensus messages
    ConsensusPropose {
        proposal: Vec<u8>, // Serialized ConsensusProposal
    },
    ConsensusVote {
        voter: String,
        proposal_id: String,
        approved: bool,
        signature: Vec<u8>,
    },
    ConsensusCommit {
        proposal_id: String,
        voters: Vec<String>,
    },
    // State sharing
    StateRequest {
        requester: String,
        request_type: Vec<u8>, // Serialized StateRequestType
    },
    StateResponse {
        response_type: Vec<u8>, // Serialized StateResponseType
        data: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SyncType {
    Full,
    Delta(Vec<(String, u64)>),  // Changed from HashMap for WIT compatibility
    Proposals(Vec<String>),     // Using String instead of ProposalId alias
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SubscriptionFilter {
    AllProposals,
    ProposalById(String),
    ProposalsByStatus(ProposalStatus),
    CommitteeUpdates,
    Discussions(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GovernanceEvent {
    ProposalCreated(ProposalData),
    VoteCast(VoteRecord),
    DiscussionAdded(Message),
    CommitteeMemberAdded(String),
    CommitteeMemberRemoved(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JoinRequest {
    node_id: String,
    public_key: Vec<u8>,
    capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JoinResponse {
    pub approved: bool,
    pub members: Vec<String>,
    pub state_hash: String,
    pub bootstrap_nodes: Vec<String>,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StateUpdate {
    event: GovernanceEvent,
    signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncRequest {
    sync_type: SyncType,
    max_events: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SyncResponse {
    Full(Vec<u8>),
    Delta(Vec<GovernanceEvent>),
    Proposals(Vec<ProposalData>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitteeStatus {
    members: Vec<String>,
    online_count: u32,
    is_member: bool,
    quorum_size: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProposalFilter {
    All,
    Active,
    ByStatus(ProposalStatus),
    ByIds(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PongResponse {
    pub timestamp: u64,
    pub state_hash: String,
    pub vector_clock: Vec<(String, u64)>,  // Changed from HashMap for WIT compatibility
    pub peer_list: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateDraftRequest {
    title: String,
    description: String,
}

#[derive(Serialize, Deserialize)]
pub struct AddDiscussionRequest {
    proposal_id: String,
    content: String,
    parent_id: Option<String>,
}

const COMMITTEE_QUORUM: f64 = 0.51;
const MAX_SUBSCRIPTIONS: usize = 1000;

impl Default for GovernanceCRDT {
    fn default() -> Self {
        // Set default committee members based on feature flag
        let default_committee: HashSet<String> = if cfg!(feature = "simulation-mode") {
            // Simulation mode: use fake.os
            HashSet::from_iter(vec!["fake.os".to_string()])
        } else {
            // Production mode: use nick.hypr and nick1udwig.os
            HashSet::from_iter(vec![
                "nick.hypr".to_string(),
                "nick1udwig.os".to_string(),
            ])
        };

        Self {
            actor_id: generate_actor_id(),
            vector_clock: HashMap::new(),
            proposals: HashSet::new(),
            proposal_data: HashMap::new(),
            votes: HashMap::new(),
            vote_records: HashMap::new(),
            committee: default_committee,
            discussions: HashMap::new(),
            merkle_root: String::new(),
        }
    }
}

fn generate_actor_id() -> String {
    format!("{}_{}", our().node, Uuid::new_v4())
}

// Helper function to convert GovernanceEvent to caller-utils type
fn convert_governance_event_to_caller(event: GovernanceEvent) -> CallerGovernanceEvent {
    match event {
        GovernanceEvent::ProposalCreated(data) => {
            CallerGovernanceEvent::ProposalCreated(CallerProposalData {
                id: data.id,
                title: data.title,
                description: data.description,
                author: data.author,
                status: convert_proposal_status_to_caller(data.status),
                voting_start: convert_hlc_timestamp_to_caller(data.voting_start),
                voting_end: convert_hlc_timestamp_to_caller(data.voting_end),
                completion_time: data.completion_time.map(convert_hlc_timestamp_to_caller),
            })
        },
        GovernanceEvent::VoteCast(vote) => {
            CallerGovernanceEvent::VoteCast(CallerVoteRecord {
                voter: vote.voter,
                proposal_id: vote.proposal_id,
                choice: convert_vote_choice_to_caller(vote.choice),
                voting_power: vote.voting_power,
                timestamp: convert_hlc_timestamp_to_caller(vote.timestamp),
            })
        },
        GovernanceEvent::DiscussionAdded(msg) => {
            CallerGovernanceEvent::DiscussionAdded(CallerMessage {
                id: msg.id,
                author: msg.author,
                content: msg.content,
                timestamp: convert_hlc_timestamp_to_caller(msg.timestamp),
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

// Helper function to convert HLCTimestamp
fn convert_hlc_timestamp_to_caller(ts: HLCTimestamp) -> CallerHlcTimestamp {
    CallerHlcTimestamp {
        wall_time: ts.wall_time,
        logical: ts.logical,
        node_id: ts.node_id,
    }
}

// Helper function to convert ProposalStatus
fn convert_proposal_status_to_caller(status: ProposalStatus) -> CallerProposalStatus {
    match status {
        ProposalStatus::Pending => CallerProposalStatus::Pending,
        ProposalStatus::Active => CallerProposalStatus::Active,
        ProposalStatus::Canceled => CallerProposalStatus::Canceled,
        ProposalStatus::Defeated => CallerProposalStatus::Defeated,
        ProposalStatus::Succeeded => CallerProposalStatus::Succeeded,
        ProposalStatus::Queued => CallerProposalStatus::Queued,
        ProposalStatus::Expired => CallerProposalStatus::Expired,
        ProposalStatus::Executed => CallerProposalStatus::Executed,
        ProposalStatus::Rejected => CallerProposalStatus::Rejected,
    }
}

// Helper function to convert VoteChoice
fn convert_vote_choice_to_caller(choice: VoteChoice) -> CallerVoteChoice {
    match choice {
        VoteChoice::Yes => CallerVoteChoice::Yes,
        VoteChoice::No => CallerVoteChoice::No,
        VoteChoice::Abstain => CallerVoteChoice::Abstain,
    }
}

impl GovernanceState {
    fn compute_state_hash(&self) -> String {
        let mut hasher = Sha3_256::new();
        let state_bytes = bincode::serialize(&self.crdt_state).unwrap_or_default();
        hasher.update(state_bytes);
        general_purpose::STANDARD.encode(hasher.finalize())
    }
    
    fn get_event_id(&self, event: &GovernanceEvent) -> String {
        match event {
            GovernanceEvent::ProposalCreated(proposal) => format!("proposal_{}", proposal.id),
            GovernanceEvent::VoteCast(vote) => format!("vote_{}_{}", vote.proposal_id, vote.voter),
            GovernanceEvent::DiscussionAdded(msg) => format!("discussion_{}", msg.id),
            GovernanceEvent::CommitteeMemberAdded(member) => format!("member_added_{}", member),
            GovernanceEvent::CommitteeMemberRemoved(member) => format!("member_removed_{}", member),
        }
    }

    fn verify_node_credentials(&self, _request: &JoinRequest) -> bool {
        true
    }

    fn get_active_committee_nodes(&self) -> Vec<String> {
        self.committee_members.iter().cloned().collect()
    }

    fn verify_event_signature(&self, _event: &GovernanceEvent, _signature: &Vec<u8>) -> bool {
        true
    }

    fn count_online_members(&self) -> usize {
        self.peers.values().filter(|p| p.is_committee).count()
    }

    fn get_public_key(&self) -> Vec<u8> {
        vec![0; 32]
    }

    async fn broadcast_to_committee_except(&mut self, _update: StateUpdate, _except: String) -> Result<(), String> {
        Ok(())
    }

    async fn broadcast_membership_update(&mut self) {
        // Create a membership update message with all counts
        let committee_count = self.committee_members.len();
        let subscriber_count = self.subscriptions.len();
        let total_participants = committee_count + subscriber_count;
        
        logging::info!("Broadcasting membership update: {} committee members, {} subscribers, {} total", 
                       committee_count, subscriber_count, total_participants);
        
        // Create event with all count info encoded
        let event = GovernanceEvent::CommitteeMemberAdded(format!("counts:{}:{}:{}", 
                                                                  committee_count, 
                                                                  subscriber_count, 
                                                                  total_participants));
        let update = CallerStateUpdate {
            event: convert_governance_event_to_caller(event),
            signature: vec![],
        };
        
        // Notify all subscribers about the membership change
        for subscription in self.subscriptions.values() {
            let target = Address::new(&subscription.subscriber, OUR_PROCESS);
            let _ = caller_utils::governance::handle_state_update_remote_rpc(&target, update.clone()).await;
        }
        
        // Also notify all committee members
        for member in &self.committee_members {
            if member != &our().node {
                let target = Address::new(member, OUR_PROCESS);
                let _ = caller_utils::governance::handle_state_update_remote_rpc(&target, update.clone()).await;
            }
        }
    }

    fn get_events_since_vector_clock(&self, since_clock: Vec<(String, u64)>) -> Vec<GovernanceEvent> {
        // Convert vector clock from WIT format to internal format
        let mut events = Vec::new();

        // Check committee state for recent events
        if let Some(committee_state) = &self.committee_state {
            // Get events from CRDT that are newer than the provided vector clock
            // TODO: Implement history tracking in CommitteeCRDT
            // for event in &committee_state.crdt.history {
            //     ... event processing ...
            // }
        }

        events
    }

    fn timestamped_event_to_governance_event(&self, _event: &p2p_committee::TimestampedEvent) -> Option<GovernanceEvent> {
        // Convert TimestampedEvent to GovernanceEvent
        // This is a simplified conversion - in production, you'd deserialize the actual event data
        match "proposal_created" {
            "proposal_created" => {
                // Look up the proposal data
                // Look up the proposal data - simplified for now
                self.proposal_drafts.values().next().map(|draft| {
                    GovernanceEvent::ProposalCreated(ProposalData {
                        id: draft.id.clone(),
                        title: draft.title.clone(),
                        description: draft.description.clone(),
                        author: draft.author.clone(),
                        status: ProposalStatus::Pending,
                        voting_start: HLCTimestamp {
                            wall_time: current_timestamp(),
                            logical: 0,
                            node_id: our().node.clone()
                        },
                        voting_end: HLCTimestamp {
                            wall_time: current_timestamp() + 7 * 24 * 3600 * 1000, // 7 days later
                            logical: 0,
                            node_id: our().node.clone()
                        },
                        completion_time: None,
                    })
                })
            },
            "vote_cast" => {
                // In production, deserialize vote data from event
                None // Simplified for now
            },
            _ => None
        }
    }

    fn get_proposals_by_ids(&self, ids: Vec<String>) -> Vec<ProposalData> {
        ids.into_iter()
            .filter_map(|id| {
                self.crdt_state.proposal_data.get(&id).cloned()
            })
            .collect()
    }

    fn get_all_proposals(&self) -> Vec<OnchainProposal> {
        self.onchain_proposals.values().cloned().collect()
    }

    fn get_active_proposals(&self) -> Vec<OnchainProposal> {
        self.onchain_proposals
            .values()
            .filter(|p| p.status == ProposalStatus::Active)
            .cloned()
            .collect()
    }

    fn get_proposals_by_status(&self, status: ProposalStatus) -> Vec<OnchainProposal> {
        self.onchain_proposals
            .values()
            .filter(|p| p.status == status)
            .cloned()
            .collect()
    }
}

impl GovernanceCRDT {
    fn update_merkle_root(&mut self) {
        let mut hasher = Sha3_256::new();
        let state_bytes = bincode::serialize(&self.proposals).unwrap_or_default();
        hasher.update(state_bytes);
        self.merkle_root = general_purpose::STANDARD.encode(hasher.finalize());
    }
}

#[hyperprocess(
    name = "DAO Governance Portal",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::default(),
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "governance-app-dot-os-v0"
)]
impl GovernanceState {
    #[init]
    async fn initialize(&mut self) {
        add_to_homepage("DAO Governance", Some("🏛️"), Some("/"), None);

        self.chain_id = 8453; // Base mainnet
        self.governor_address = String::new();
        self.token_address = String::new();
        self.is_committee_member = false;
        self.is_indexing = false;

        // Start keepalive timer if we're a committee member
        if self.is_committee_member {
            let _ = timer::set_timer(60000, Some(b"keepalive".to_vec()));
        }

        // Initialize storage
        match FileStorage::new() {
            Ok(storage) => {
                // Load saved state from storage
                if let Ok(proposals) = storage.load_proposals() {
                    self.onchain_proposals = proposals;
                }
                if let Ok(drafts) = storage.load_drafts() {
                    self.proposal_drafts = drafts;
                }
                if let Ok(discussions) = storage.load_discussions() {
                    self.discussions = discussions;
                }

                // Load committee state
                // Initialize committee state (removed load_committee_state as method doesn't exist)
                /* match storage.load_committee_state() {
                    Ok(committee_state) => {
                        // Update our state from loaded committee state
                        self.is_committee_member = committee_state.is_committee_member;
                        self.committee_members = committee_state.crdt.committee_members.clone();
                        self.committee_state = Some(committee_state);
                        println!("Loaded committee state with {} members", self.committee_members.len());
                    },
                    Err(_) => {
                        // Create new committee state
                        let node_id = our().node.clone();
                        self.committee_state = Some(p2p_committee::CommitteeState::new(node_id));
                        println!("Created new committee state");
                    }
                } */
                // Initialize committee state
                let node_id = our().node.clone();
                self.committee_state = Some(p2p_committee::CommitteeState::new(node_id));

                if let Ok(last_block) = storage.load_metadata() {
                    self.last_indexed_block = last_block;
                }
                self.storage = Some(storage);
            },
            Err(e) => {
                println!("Failed to initialize storage: {}", e);
                // Create committee state even without storage
                let node_id = our().node.clone();
                self.committee_state = Some(p2p_committee::CommitteeState::new(node_id));
            }
        }

        // Initialize chain indexer
        match ChainIndexer::new().await {
            Ok(indexer) => {
                self.chain_indexer = Some(indexer);
                // Start indexing timer - every 30 seconds
                timer::set_timer(30000, Some(b"index_chain".to_vec()));
            },
            Err(e) => {
                println!("Failed to initialize chain indexer: {}", e);
            }
        }

        // Start keepalive timer - every 60 seconds
        timer::set_timer(60000, Some(b"keepalive".to_vec()));

        // Start snapshot timer - every hour
        timer::set_timer(3600000, Some(b"snapshot".to_vec()));

        // Start garbage collection timer - every 24 hours
        timer::set_timer(86400000, Some(b"gc".to_vec()));

        // Check if we're a committee member and handle subscription accordingly
        let our_node = our().node.clone();
        if self.committee_members.contains(&our_node) {
            self.is_committee_member = true;
            if let Some(committee_state) = &mut self.committee_state {
                committee_state.is_committee_member = true;
            }
            println!("Node {} is a committee member", our_node);
        } else {
            // Non-committee member: automatically subscribe to committee updates
            self.is_committee_member = false;

            // Try to find committee members to subscribe through
            if !self.committee_members.is_empty() {
                // Try each committee member until we successfully subscribe
                let mut subscription_success = false;
                for committee_member in self.committee_members.iter().take(3) { // Try up to 3 members
                    let target = Address::new(committee_member, OUR_PROCESS);

                    // Send subscription request
                    let subscriber_node = our_node.clone();
                    match caller_utils::governance::handle_subscription_remote_rpc(&target, subscriber_node).await {
                        Ok(Ok(response)) => {
                            // Parse the subscription ID and counts from response
                            let parts: Vec<&str> = response.split('|').collect();
                            let subscription_id = parts[0].to_string();
                            
                            // Parse counts if present
                            if let Some(counts_part) = parts.iter().find(|s| s.starts_with("counts:")) {
                                if let Some(counts_str) = counts_part.strip_prefix("counts:") {
                                    let count_parts: Vec<&str> = counts_str.split(':').collect();
                                    if count_parts.len() >= 3 {
                                        self.committee_count = count_parts[0].parse().unwrap_or(0);
                                        self.subscriber_count = count_parts[1].parse().unwrap_or(0);
                                        self.total_participants = count_parts[2].parse().unwrap_or(0);
                                        logging::info!("Updated counts - Committee: {}, Subscribers: {}, Total: {}", 
                                                     self.committee_count, self.subscriber_count, self.total_participants);
                                    }
                                }
                            }
                            
                            logging::info!("Successfully subscribed to committee member {} with ID: {}", committee_member, subscription_id);
                            subscription_success = true;
                            break;
                        },
                        Ok(Err(e)) => {
                            println!("Committee member {} rejected subscription: {}", committee_member, e);
                        },
                        Err(e) => {
                            println!("Error subscribing to committee member {}: {}", committee_member, e);
                        }
                    }
                }

                // If we couldn't subscribe, set a timer to retry
                if !subscription_success {
                    println!("Failed to subscribe to any committee member, will retry in 10 seconds");
                    timer::set_timer(10000, Some(b"retry_subscription".to_vec()));
                }
            } else {
                println!("No committee members available to subscribe to");
                // Set timer to retry subscription later
                timer::set_timer(10000, Some(b"retry_subscription".to_vec()));
            }
        }

        println!("DAO Governance Portal initialized on node: {}", our().node);
    }

    #[http]
    async fn ready(&self) -> Result<String, String> {
        Ok(json!({ "ready": !self.is_indexing }).to_string())
    }

    #[http]
    async fn get_proposals(&self) -> Result<String, String> {
        Ok(json!({
            "onchain": self.onchain_proposals.values().collect::<Vec<_>>(),
            "drafts": self.proposal_drafts.values().collect::<Vec<_>>()
        }).to_string())
    }

    #[http]
    async fn edit_draft(&mut self, request_body: String) -> Result<String, String> {
        let req: serde_json::Value = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;
        
        let draft_id = req.get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing draft id")?;
        
        // Check if draft exists and user is the author
        let draft = self.proposal_drafts.get(draft_id)
            .ok_or("Draft not found")?;
        
        if draft.author != our().node {
            return Err("Only the author can edit this draft".to_string());
        }
        
        // Update the draft
        let mut updated_draft = draft.clone();
        if let Some(title) = req.get("title").and_then(|v| v.as_str()) {
            updated_draft.title = title.to_string();
        }
        if let Some(description) = req.get("description").and_then(|v| v.as_str()) {
            updated_draft.description = description.to_string();
        }
        updated_draft.updated_at = chrono::Utc::now().to_rfc3339();
        
        self.proposal_drafts.insert(draft_id.to_string(), updated_draft.clone());
        
        // Persist to VFS
        if let Some(storage) = &self.storage {
            let _ = storage.save_drafts(&self.proposal_drafts);
        }
        
        Ok(json!({
            "success": true,
            "draft": updated_draft
        }).to_string())
    }
    
    #[http]
    async fn delete_draft(&mut self, request_body: String) -> Result<String, String> {
        let req: serde_json::Value = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;
        
        let draft_id = req.get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing draft id")?;
        
        // Check if draft exists and user is the author
        let draft = self.proposal_drafts.get(draft_id)
            .ok_or("Draft not found")?;
        
        if draft.author != our().node {
            return Err("Only the author can delete this draft".to_string());
        }
        
        // Remove the draft
        self.proposal_drafts.remove(draft_id);
        
        // Persist to VFS
        if let Some(storage) = &self.storage {
            let _ = storage.save_drafts(&self.proposal_drafts);
        }
        
        Ok(json!({
            "success": true,
            "message": "Draft deleted successfully"
        }).to_string())
    }
    
    #[http]
    async fn create_draft(&mut self, request_body: String) -> Result<String, String> {
        let req: CreateDraftRequest = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;

        let draft_id = generate_id();
        let draft = ProposalDraft {
            id: draft_id.clone(),
            author: our().node.clone(),
            title: req.title,
            description: req.description,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            signatures: vec![],
        };

        self.proposal_drafts.insert(draft_id.clone(), draft.clone());

        // Persist to VFS
        if let Some(storage) = &self.storage {
            let _ = storage.save_drafts(&self.proposal_drafts);
        }

        // Create a ProposalData for CRDT
        let proposal_data = ProposalData {
            id: draft_id.clone(),
            title: draft.title.clone(),
            description: draft.description.clone(),
            author: draft.author.clone(),
            status: ProposalStatus::Pending,
            voting_start: HLCTimestamp {
                wall_time: current_timestamp(),
                logical: 0,
                node_id: our().node.clone(),
            },
            voting_end: HLCTimestamp {
                wall_time: current_timestamp() + 7 * 24 * 3600 * 1000, // 7 days
                logical: 0,
                node_id: our().node.clone(),
            },
            completion_time: None,
        };

        // Submit proposal to committee for review (both members and non-members can submit)
        let event = GovernanceEvent::ProposalCreated(proposal_data.clone());

        // If we're not a committee member, we need to send it to the committee
        if !self.is_committee_member {
            // Find committee members to submit to
            let committee_members: Vec<String> = self.committee_members.iter().cloned().collect();
            if committee_members.is_empty() {
                return Err("No committee members available to submit proposal".to_string());
            }

            // Send to first available committee member
            for member in committee_members.iter().take(3) { // Try up to 3 members
                let target = Address::new(member, OUR_PROCESS);

                // Create state update with the proposal
                let update = CallerStateUpdate {
                    event: convert_governance_event_to_caller(event.clone()),
                    signature: vec![],
                };

                match caller_utils::governance::handle_state_update_remote_rpc(&target, update).await {
                    Ok(Ok(_)) => {
                        // Successfully submitted to committee
                        break;
                    },
                    _ => continue, // Try next member
                }
            }
        } else {
            // We're a committee member, just broadcast normally
            self.broadcast_event(event).await?;
        }

        Ok(json!({
            "success": true,
            "draft_id": draft_id,
            "draft": draft
        }).to_string())
    }

    #[http]
    async fn get_discussions(&self, request_body: String) -> Result<String, String> {
        let req: serde_json::Value = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;
        
        let proposal_id = req.get("proposal_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing proposal_id")?;
        
        let discussions = self.discussions.get(proposal_id)
            .cloned()
            .unwrap_or_default();
        
        Ok(json!({
            "success": true,
            "discussions": discussions
        }).to_string())
    }
    
    #[http]
    async fn add_discussion(&mut self, request_body: String) -> Result<String, String> {
        let req: AddDiscussionRequest = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;

        let discussion = Discussion {
            id: generate_id(),
            proposal_id: req.proposal_id.clone(),
            parent_id: req.parent_id,
            author: our().node.clone(),
            content: req.content,
            timestamp: chrono::Utc::now().to_rfc3339(),
            upvotes: 0,
            downvotes: 0,
            signatures: vec![],
        };

        self.discussions
            .entry(req.proposal_id.clone())
            .or_insert_with(Vec::new)
            .push(discussion.clone());

        // Persist to VFS
        if let Some(storage) = &self.storage {
            let _ = storage.save_discussions(&self.discussions);
        }

        // Create Message for CRDT
        let message = Message {
            id: discussion.id.clone(),
            author: discussion.author.clone(),
            content: discussion.content.clone(),
            timestamp: HLCTimestamp {
                wall_time: current_timestamp(),
                logical: 0,
                node_id: our().node.clone(),
            },
            parent_id: discussion.parent_id.clone(),
        };

        // Broadcast to committee
        let event = GovernanceEvent::DiscussionAdded(message);
        self.broadcast_event(event).await?;

        Ok(json!({
            "success": true,
            "discussion": discussion
        }).to_string())
    }

    #[http]
    async fn get_committee_status(&self) -> Result<String, String> {
        // Use stored counts if available, otherwise calculate from local state
        let committee_count = if self.committee_count > 0 {
            self.committee_count
        } else {
            self.committee_members.len()
        };
        
        let subscriber_count = if self.subscriber_count > 0 {
            self.subscriber_count
        } else {
            self.subscriptions.len()
        };
        
        let total_participants = if self.total_participants > 0 {
            self.total_participants
        } else {
            committee_count + subscriber_count
        };

        Ok(json!({
            "members": self.committee_members,
            "is_member": self.is_committee_member,
            "committee_count": committee_count,
            "subscriber_count": subscriber_count,
            "total_participants": total_participants
        }).to_string())
    }

    #[http]
    async fn get_voting_power_info(&mut self, request: String) -> Result<String, String> {
        // Parse the request to get the wallet address
        let req: serde_json::Value = serde_json::from_str(&request)
            .unwrap_or_else(|_| json!({}));

        let address = req.get("address")
            .and_then(|a| a.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");

        // Get voting power from chain
        let voting_power = if let Some(indexer) = &self.chain_indexer {
            match indexer.get_voting_power(address, 0).await {
                Ok(power) => power,
                Err(_) => "0".to_string(),
            }
        } else {
            "0".to_string()
        };

        // Get total supply (simplified - would query token contract)
        let total_supply = "1000000000000000000000000".to_string(); // 1M tokens

        Ok(json!({
            "address": address,
            "voting_power": voting_power,
            "delegated_power": "0",
            "total_supply": total_supply,
            "quorum": "100000000000000000000000" // 100k tokens
        }).to_string())
    }

    #[http]
    async fn submit_proposal(&mut self, request: String) -> Result<String, String> {
        let req: serde_json::Value = serde_json::from_str(&request)
            .map_err(|e| format!("Invalid request: {}", e))?;

        let title = req.get("title")
            .and_then(|t| t.as_str())
            .ok_or("Missing title")?;

        let description = req.get("description")
            .and_then(|d| d.as_str())
            .ok_or("Missing description")?;

        let targets = req.get("targets")
            .and_then(|t| t.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(Vec::new);

        let values = req.get("values")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(Vec::new);

        let calldatas = req.get("calldatas")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(Vec::new);

        // Generate proposal ID (would be computed from hash)
        let proposal_id = format!("0x{}", hex::encode(Sha3_256::digest(description.as_bytes())));

        // Create the onchain proposal
        let proposal = OnchainProposal {
            id: proposal_id.clone(),
            proposer: req.get("proposer")
                .and_then(|p| p.as_str())
                .unwrap_or("0x0000000000000000000000000000000000000000")
                .to_string(),
            title: title.to_string(),
            description: description.to_string(),
            targets,
            values,
            calldatas,
            start_block: 0, // Would be set by chain
            end_block: 0, // Would be set by chain
            votes_for: "0".to_string(),
            votes_against: "0".to_string(),
            votes_abstain: "0".to_string(),
            status: ProposalStatus::Pending,
            tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            block_number: 0,
        };

        // Add to local state
        self.onchain_proposals.insert(proposal_id.clone(), proposal);

        // Save to storage
        if let Some(storage) = &self.storage {
            let _ = storage.save_proposals(&self.onchain_proposals);
        }

        // In production, this would submit the transaction to chain
        // For now, return success with the proposal ID
        Ok(json!({
            "success": true,
            "proposal_id": proposal_id,
            "message": "Proposal created locally. Chain submission pending."
        }).to_string())
    }

    #[http]
    async fn cast_vote(&mut self, request: String) -> Result<String, String> {
        let req: serde_json::Value = serde_json::from_str(&request)
            .map_err(|e| format!("Invalid request: {}", e))?;

        let proposal_id = req.get("proposal_id")
            .and_then(|p| p.as_str())
            .ok_or("Missing proposal_id")?
            .to_string();

        let choice_str = req.get("choice")
            .and_then(|c| c.as_str())
            .ok_or("Missing choice")?;

        let choice = match choice_str {
            "yes" | "Yes" => VoteChoice::Yes,
            "no" | "No" => VoteChoice::No,
            "abstain" | "Abstain" => VoteChoice::Abstain,
            _ => return Err("Invalid choice".to_string()),
        };

        let voter = req.get("voter")
            .and_then(|v| v.as_str())
            .unwrap_or(&our().node)
            .to_string();

        let voting_power = req.get("voting_power")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        // Create vote record
        let vote = VoteRecord {
            voter: voter.clone(),
            proposal_id: proposal_id.clone(),
            choice,
            voting_power,
            timestamp: HLCTimestamp {
                wall_time: current_timestamp(),
                logical: 0,
                node_id: our().node.clone(),
            },
        };

        // Add to local state
        self.crdt_state.vote_records
            .entry(proposal_id.clone())
            .or_insert_with(HashSet::new)
            .insert(vote.clone());

        // Broadcast to committee
        let event = GovernanceEvent::VoteCast(vote);
        self.broadcast_event(event).await?;

        Ok(json!({
            "success": true,
            "proposal_id": proposal_id,
            "voter": voter,
            "choice": choice_str
        }).to_string())
    }

    #[http]
    async fn request_join_committee(&mut self, _request_body: String) -> Result<String, String> {
        // Check if already a committee member
        if self.is_committee_member {
            return Ok(json!({
                "success": false,
                "message": "Already a committee member"
            }).to_string());
        }

        // Get list of committee members to try
        let mut target_nodes: Vec<String> = self.committee_members.iter().cloned().collect();
        
        // If no known committee members, use default bootstrap nodes
        if target_nodes.is_empty() {
            target_nodes = vec![
                "nick.hypr".to_string(),
                "nick1udwig.os".to_string(),
            ];
        }
        
        // Shuffle the list to try random members
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        target_nodes.shuffle(&mut rng);

        // Create join request
        let join_request = JoinRequest {
            node_id: our().node.clone(),
            public_key: self.get_public_key(),
            capabilities: vec!["governance".to_string()],
        };

        // Try joining through each target node
        for node in target_nodes.iter().take(5) { // Try up to 5 members
            let target = Address::new(node, OUR_PROCESS);

            // Use the generated RPC stub
            // Convert to caller_utils type
            let caller_request = caller_utils::JoinRequest {
                node_id: join_request.node_id.clone(),
                public_key: join_request.public_key.clone(),
                capabilities: join_request.capabilities.clone(),
            };
            match caller_utils::governance::handle_join_request_remote_rpc(&target, caller_request).await {
                Ok(Ok(response)) => {
                    if response.approved {
                        // Update our state
                        self.committee_members = HashSet::from_iter(response.members.clone());
                        self.is_committee_member = true;

                        // Parse counts from reason field
                        let mut committee_count = self.committee_members.len();
                        let mut subscriber_count = 0;
                        let mut total_participants = committee_count;
                        
                        if let Some(counts_part) = response.reason.split('|').find(|s| s.starts_with("counts:")) {
                            if let Some(counts_str) = counts_part.strip_prefix("counts:") {
                                let parts: Vec<&str> = counts_str.split(':').collect();
                                if parts.len() >= 3 {
                                    committee_count = parts[0].parse().unwrap_or(committee_count);
                                    subscriber_count = parts[1].parse().unwrap_or(0);
                                    total_participants = parts[2].parse().unwrap_or(committee_count + subscriber_count);
                                }
                            }
                        }
                        
                        // Update our counts
                        self.subscriber_count = subscriber_count;
                        self.committee_count = committee_count;
                        self.total_participants = total_participants;
                        
                        logging::info!("Joined committee - Committee members: {}, Subscribers: {}, Total participants: {}", 
                                     committee_count, subscriber_count, total_participants);

                        // Initialize committee state if needed
                        if self.committee_state.is_none() {
                            self.committee_state = Some(p2p_committee::CommitteeState::new(our().node.clone()));
                        }

                        // Store members before moving them
                        let members_list = response.members.clone();
                        let bootstrap_list = response.bootstrap_nodes.clone();

                        if let Some(committee_state) = &mut self.committee_state {
                            committee_state.is_committee_member = true;
                            for member in response.members {
                                committee_state.crdt.committee_members.insert(member);
                            }
                        }

                        // Save state
                        if let Some(storage) = &self.storage {
                            let _ = storage.save_committee_members(&self.committee_members);
                        }

                        // Start syncing with the committee
                        let _ = self.initiate_sync(node.to_string(), response.state_hash).await;

                        return Ok(json!({
                            "success": true,
                            "message": "Successfully joined committee",
                            "members": members_list,
                            "bootstrap_nodes": bootstrap_list
                        }).to_string());
                    } else {
                        // Join request rejected
                    }
                },
                Ok(Err(e)) => {
                    // Remote error from node
                },
                Err(e) => {
                    // Failed to contact node
                }
            }
        }

        Ok(json!({
            "success": false,
            "message": "Failed to join committee through any node"
        }).to_string())
    }

    #[http]
    async fn subscribe_to_committee(&mut self, _request_body: String) -> Result<String, String> {
        // Non-committee members can subscribe to receive updates
        if self.is_committee_member {
            return Ok(json!({
                "success": false,
                "message": "Committee members don't need to subscribe"
            }).to_string());
        }

        // Find a committee member to subscribe through
        let committee_member = self.committee_members.iter().next()
            .ok_or("No committee members available")?;

        let target = Address::new(committee_member, OUR_PROCESS);

        // Send subscription request
        match caller_utils::governance::handle_subscription_remote_rpc(&target, our().node.clone()).await {
            Ok(Ok(response)) => {
                // Parse the subscription ID and counts from response
                let parts: Vec<&str> = response.split('|').collect();
                let subscription_id = parts[0].to_string();
                
                // Parse counts if present
                if let Some(counts_part) = parts.iter().find(|s| s.starts_with("counts:")) {
                    if let Some(counts_str) = counts_part.strip_prefix("counts:") {
                        let count_parts: Vec<&str> = counts_str.split(':').collect();
                        if count_parts.len() >= 3 {
                            self.committee_count = count_parts[0].parse().unwrap_or(0);
                            self.subscriber_count = count_parts[1].parse().unwrap_or(0);
                            self.total_participants = count_parts[2].parse().unwrap_or(0);
                        }
                    }
                }
                
                Ok(json!({
                    "success": true,
                    "subscription_id": subscription_id,
                    "message": "Successfully subscribed to committee updates"
                }).to_string())
            },
            Ok(Err(e)) => Err(format!("Subscription rejected: {}", e)),
            Err(e) => Err(format!("Failed to subscribe: {}", e))
        }
    }

    #[http]
    async fn request_state_from_committee(&mut self, request_body: String) -> Result<String, String> {
        // For non-committee nodes to request state from committee
        let req: serde_json::Value = serde_json::from_str(&request_body)
            .unwrap_or_else(|_| json!({}));

        let request_type = req.get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("full");

        if let Some(committee_state) = &mut self.committee_state {
            // If we're already a committee member, no need to request
            if committee_state.is_committee_member {
                return Ok(json!({
                    "success": false,
                    "message": "Already a committee member with full state"
                }).to_string());
            }

            // Pick a committee member to request from
            let target = if let Some(member) = committee_state.crdt.committee_members.iter().next() {
                member.clone()
            } else {
                return Err("No committee members available".to_string());
            };

            // Create state request
            let state_request = match request_type {
                "full" => p2p_committee::StateRequestType::FullState,
                "delta" => p2p_committee::StateRequestType::DeltaSync {
                    since_clock: committee_state.crdt.vector_clock.clone(),
                },
                "committee" => p2p_committee::StateRequestType::CommitteeInfo,
                _ => p2p_committee::StateRequestType::FullState,
            };

            let msg = P2PMessage::StateRequest {
                requester: our().node.clone(),
                request_type: bincode::serialize(&state_request).unwrap(),
            };

            // Send request and handle response
            match p2p_committee::send_p2p_message(&target, msg).await {
                Ok(_response) => {
                    // Response handling would be done in the message handler
                    Ok(json!({
                        "success": true,
                        "message": "State request sent to committee"
                    }).to_string())
                },
                Err(e) => Err(format!("Failed to request state: {}", e))
            }
        } else {
            Err("Committee state not initialized".to_string())
        }
    }


    #[remote]
    async fn handle_join_request(&mut self, request: JoinRequest) -> Result<JoinResponse, String> {
        logging::info!("Received join request from {}", request.node_id);
        
        if !self.verify_node_credentials(&request) {
            logging::info!("Rejected join request from {} - invalid credentials", request.node_id);
            return Ok(JoinResponse {
                approved: false,
                members: vec![],
                state_hash: String::new(),
                bootstrap_nodes: vec![],
                reason: "Invalid credentials".to_string(),
            });
        }

        // Check if this node was previously a subscriber and remove them
        let was_subscriber = self.subscriptions.contains_key(&request.node_id);
        if was_subscriber {
            self.subscriptions.remove(&request.node_id);
            logging::info!("Promoting subscriber {} to committee member", request.node_id);
        }
        
        self.committee_members.insert(request.node_id.clone());

        // Log at info level with both committee and subscriber counts
        logging::info!("New committee member joined: {} | Total committee members: {} | Total subscribers: {}", 
                       request.node_id, self.committee_members.len(), self.subscriptions.len());

        // Select a random committee member to handle this node's subscription
        let mut committee_vec: Vec<String> = self.committee_members.iter().cloned().collect();
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        committee_vec.shuffle(&mut rng);
        let subscription_handler = committee_vec.first().cloned().unwrap_or_else(|| our().node.clone());

        // Calculate total participants
        let committee_count = self.committee_members.len();
        let subscriber_count = self.subscriptions.len();
        let total_participants = committee_count + subscriber_count;
        
        let response = JoinResponse {
            approved: true,
            members: self.committee_members.iter().cloned().collect(),
            state_hash: self.compute_state_hash(),
            bootstrap_nodes: vec![subscription_handler.clone()], // Use first bootstrap node as subscription handler
            reason: format!("subscription_handler:{}|counts:{}:{}:{}", 
                          subscription_handler, committee_count, subscriber_count, total_participants), // Pass handler and counts in reason field
        };

        // Broadcast new member to all subscribers
        self.broadcast_membership_update().await;

        Ok(response)
    }

    #[remote]
    async fn handle_state_update(&mut self, update: StateUpdate) -> Result<String, String> {
        logging::info!("Received state update: {:?}", update.event);
        
        // Get the sender to avoid echoing back
        let sender = source().node;
        
        if !self.verify_event_signature(&update.event, &update.signature) {
            logging::info!("Rejected state update - invalid signature");
            return Err("Invalid signature".to_string());
        }

        // Check if we've already processed this event to prevent infinite loops
        let event_id = self.get_event_id(&update.event);
        if self.processed_events.contains(&event_id) {
            logging::info!("Already processed event {}, skipping", event_id);
            return Ok("Already processed".to_string());
        }
        self.processed_events.insert(event_id.clone());
        
        // Keep only recent events to prevent memory growth
        if self.processed_events.len() > 1000 {
            self.processed_events.clear();
        }

        // Handle special count updates from membership changes
        if let GovernanceEvent::CommitteeMemberAdded(ref data) = update.event {
            if data.starts_with("counts:") {
                // Parse counts from the data: "counts:committee:subscribers:total"
                let parts: Vec<&str> = data.split(':').collect();
                if parts.len() == 4 {
                    if let (Ok(committee), Ok(subscribers), Ok(total)) = 
                        (parts[1].parse::<usize>(), parts[2].parse::<usize>(), parts[3].parse::<usize>()) {
                        logging::info!("Received count update: {} committee, {} subscribers, {} total", committee, subscribers, total);
                        // Update our local counts
                        self.committee_count = committee;
                        self.subscriber_count = subscribers;
                        self.total_participants = total;
                    }
                }
            }
        }
        
        // Apply the update to our CRDT
        self.apply_event_to_crdt(update.event.clone());

        // Also update proposal_drafts if this is a proposal creation
        if let GovernanceEvent::ProposalCreated(ref proposal_data) = update.event {
            // Convert ProposalData to ProposalDraft for UI display
            let draft = ProposalDraft {
                id: proposal_data.id.clone(),
                author: proposal_data.author.clone(),
                title: proposal_data.title.clone(),
                description: proposal_data.description.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                signatures: vec![],
            };

            // Store in proposal_drafts so UI can see it
            self.proposal_drafts.insert(proposal_data.id.clone(), draft);

            // Persist to VFS
            if let Some(storage) = &self.storage {
                let _ = storage.save_drafts(&self.proposal_drafts);
            }
        }

        // If we're a committee member, broadcast to other committee members and subscribers
        if self.is_committee_member {
            // Convert to caller type for RPC
            let caller_update = CallerStateUpdate {
                event: convert_governance_event_to_caller(update.event.clone()),
                signature: update.signature.clone(),
            };

            // Broadcast to ALL other committee members (use self.committee_members, not committee_state)
            for member in &self.committee_members {
                if member != &our().node && member != &sender {
                    let target = Address::new(member, OUR_PROCESS);
                    logging::info!("Broadcasting update to committee member: {}", member);
                    let _ = caller_utils::governance::handle_state_update_remote_rpc(&target, caller_update.clone()).await;
                }
            }

            // Also broadcast to all subscribers (non-committee DAO members)
            println!("Broadcasting to {} subscribers", self.subscriptions.len());
            for subscription in self.subscriptions.values() {
                println!("Sending update to subscriber: {}", subscription.subscriber);
                let target = Address::new(&subscription.subscriber, OUR_PROCESS);
                match caller_utils::governance::handle_state_update_remote_rpc(&target, caller_update.clone()).await {
                    Ok(Ok(_)) => println!("Successfully sent update to {}", subscription.subscriber),
                    Ok(Err(e)) => println!("Subscriber {} rejected update: {}", subscription.subscriber, e),
                    Err(e) => println!("Failed to send update to {}: {}", subscription.subscriber, e),
                }
            }
        }

        Ok("ACK".to_string())
    }

    #[remote]
    async fn handle_sync_request(&mut self, request: SyncRequest) -> Result<SyncResponse, String> {
        logging::info!("Received sync request: {:?}", request.sync_type);
        
        let response = match request.sync_type {
            SyncType::Full => {
                let snapshot = bincode::serialize(&self.crdt_state)
                    .map_err(|e| format!("Serialization error: {}", e))?;
                SyncResponse::Full(snapshot)
            },
            SyncType::Delta(since_clock) => {
                // Implement delta sync based on vector clock
                // Get all events that happened after the given vector clock
                let delta_events = self.get_events_since_vector_clock(since_clock);
                SyncResponse::Delta(delta_events)
            },
            SyncType::Proposals(ids) => {
                let proposals = self.get_proposals_by_ids(ids);
                SyncResponse::Proposals(proposals)
            }
        };

        Ok(response)
    }

    #[remote]
    async fn handle_subscription(&mut self, subscriber: String) -> Result<String, String> {
        if self.subscriptions.len() >= MAX_SUBSCRIPTIONS {
            return Err("At capacity".to_string());
        }

        // Use the sender's node ID as the subscription ID
        let sender_node = source().node;
        let subscription_id = sender_node.to_string();

        // Check if this node is already subscribed
        if self.subscriptions.contains_key(&subscription_id) {
            logging::info!("Node {} is already subscribed", sender_node);
            return Ok(subscription_id);
        }

        let subscription = Subscription {
            id: subscription_id.clone(),
            subscriber: subscriber.clone(),
            subscription_type: SubscriptionType::AllProposals,
            created_at: current_timestamp(),
            last_update: current_timestamp(),
            delivery_mode: DeliveryMode::Push,
        };

        self.subscriptions.insert(subscription_id.clone(), subscription.clone());

        // Log at info level with both committee and subscriber counts
        let committee_count = self.committee_members.len();
        let subscriber_count = self.subscriptions.len();
        let total_participants = committee_count + subscriber_count;
        
        logging::info!("New subscription from {} | Committee members: {} | Total subscribers: {}", 
                       sender_node, committee_count, subscriber_count);

        // Broadcast the membership update to all participants
        self.broadcast_membership_update().await;

        // Return the subscription ID with counts embedded in response
        Ok(format!("{}|counts:{}:{}:{}", subscription_id, committee_count, subscriber_count, total_participants))
    }

    #[remote]
    async fn handle_keepalive(&mut self, _timestamp: u64, state_hash: String) -> Result<String, String> {
        // Update peer info
        let sender = source().node;
        logging::info!("Received keepalive from {} with state hash {}", sender, state_hash);

        let peer_info = PeerInfo {
            node_id: sender.clone(),
            last_seen: current_timestamp(),
            state_hash: state_hash.clone(),
            vector_clock: vec![],  // Empty vector for WIT compatibility
            is_committee: self.committee_members.contains(&sender),
        };

        // Update in main peers list
        self.peers.insert(sender.clone(), peer_info.clone());

        // Also update in committee state if we have one
        if let Some(committee_state) = &mut self.committee_state {
            // Use our local PeerInfo struct for now
            // TODO: Sync with committee_state.peers when p2p_committee::PeerInfo is public
        }

        // Return state hash as acknowledgment
        Ok(self.compute_state_hash())
    }

    #[local]
    #[remote]
    async fn get_committee_status_both(&self) -> Result<CommitteeStatus, String> {
        Ok(CommitteeStatus {
            members: self.committee_members.iter().cloned().collect(),
            online_count: self.count_online_members() as u32,
            is_member: self.is_committee_member,
            quorum_size: (self.committee_members.len() as f64 * COMMITTEE_QUORUM) as u32,
        })
    }

    #[local]
    #[remote]
    async fn get_proposals_filtered(&self, filter: ProposalFilter) -> Result<Vec<OnchainProposal>, String> {
        let proposals = match filter {
            ProposalFilter::All => self.get_all_proposals(),
            ProposalFilter::Active => self.get_active_proposals(),
            ProposalFilter::ByStatus(status) => self.get_proposals_by_status(status),
            ProposalFilter::ByIds(ids) => {
                ids.into_iter()
                    .filter_map(|id| self.onchain_proposals.get(&id).cloned())
                    .collect()
            },
        };

        Ok(proposals)
    }
}


// Helper methods implementation
impl GovernanceState {
    async fn handle_timer(&mut self, context: Option<Vec<u8>>) -> Result<(), String> {
        if let Some(ctx) = context {
            match ctx.as_slice() {
                b"index_chain" => {
                    self.index_chain().await?;
                    // Schedule next indexing
                    let _ = timer::set_timer(30000, Some(b"index_chain".to_vec()));
                },
                b"keepalive" => {
                    self.send_keepalive().await?;
                    // Schedule next keepalive
                    let _ = timer::set_timer(60000, Some(b"keepalive".to_vec()));
                },
                b"retry_subscription" => {
                    // Retry subscription for non-committee members
                    if !self.is_committee_member && !self.committee_members.is_empty() {
                        let our_node = our().node.clone();
                        let mut subscription_success = false;

                        for committee_member in self.committee_members.iter().take(3) {
                            let target = Address::new(committee_member, OUR_PROCESS);
                            match caller_utils::governance::handle_subscription_remote_rpc(&target, our_node.clone()).await {
                                Ok(Ok(response)) => {
                                    // Parse the subscription ID and counts from response
                                    let parts: Vec<&str> = response.split('|').collect();
                                    let subscription_id = parts[0].to_string();
                                    
                                    // Parse counts if present
                                    if let Some(counts_part) = parts.iter().find(|s| s.starts_with("counts:")) {
                                        if let Some(counts_str) = counts_part.strip_prefix("counts:") {
                                            let count_parts: Vec<&str> = counts_str.split(':').collect();
                                            if count_parts.len() >= 3 {
                                                self.committee_count = count_parts[0].parse().unwrap_or(0);
                                                self.subscriber_count = count_parts[1].parse().unwrap_or(0);
                                                self.total_participants = count_parts[2].parse().unwrap_or(0);
                                            }
                                        }
                                    }
                                    
                                    logging::info!("Successfully subscribed to committee member {} with ID: {}", committee_member, subscription_id);
                                    subscription_success = true;
                                    break;
                                },
                                _ => continue,
                            }
                        }

                        // If still failed, retry again later
                        if !subscription_success {
                            let _ = timer::set_timer(30000, Some(b"retry_subscription".to_vec())); // Retry in 30 seconds
                        }
                    }
                },
                _ => {}
            }
        }
        Ok(())
    }

    async fn index_chain(&mut self) -> Result<(), String> {
        if self.is_indexing {
            return Ok(());
        }

        self.is_indexing = true;

        if let Some(indexer) = &mut self.chain_indexer {
            // Index from last block to current
            match indexer.index_proposals(self.last_indexed_block + 1, None).await {
                Ok(events) => {
                    // Process events
                    for event in &events {
                        process_proposal_event(event.clone(), &mut self.onchain_proposals);
                    }

                    // Update proposals status based on current block
                    if let Ok(current_block) = indexer.get_block_number().await {
                        for proposal in self.onchain_proposals.values_mut() {
                            if proposal.status == ProposalStatus::Pending && current_block >= proposal.start_block {
                                proposal.status = ProposalStatus::Active;
                            } else if proposal.status == ProposalStatus::Active && current_block > proposal.end_block {
                                // Check vote counts to determine if succeeded or defeated
                                let for_votes: u128 = proposal.votes_for.parse().unwrap_or(0);
                                let against_votes: u128 = proposal.votes_against.parse().unwrap_or(0);

                                if for_votes > against_votes {
                                    proposal.status = ProposalStatus::Succeeded;
                                } else {
                                    proposal.status = ProposalStatus::Defeated;
                                }
                            }
                        }

                        self.last_indexed_block = current_block;
                    }

                    // Save to storage
                    if let Some(storage) = &self.storage {
                        let _ = storage.save_proposals(&self.onchain_proposals);
                        let _ = storage.save_metadata(self.last_indexed_block);

                        // Save events log
                        if !events.is_empty() {
                            let _ = storage.save_event_log(self.last_indexed_block, &events);
                        }
                    }

                    // Broadcast state update to committee
                    for event in events {
                        let gov_event = match event {
                            ProposalEvent::Created { proposal_id, .. } => {
                                if let Some(proposal) = self.onchain_proposals.get(&proposal_id) {
                                    Some(GovernanceEvent::ProposalCreated(ProposalData {
                                        id: proposal.id.clone(),
                                        title: proposal.title.clone(),
                                        description: proposal.description.clone(),
                                        author: proposal.proposer.clone(),
                                        status: proposal.status.clone(),
                                        voting_start: HLCTimestamp::now(),
                                        voting_end: HLCTimestamp::now(),
                                        completion_time: None,
                                    }))
                                } else {
                                    None
                                }
                            },
                            ProposalEvent::VoteCast { proposal_id, voter, support, weight, .. } => {
                                let choice = match support {
                                    0 => VoteChoice::No,
                                    1 => VoteChoice::Yes,
                                    _ => VoteChoice::Abstain,
                                };
                                Some(GovernanceEvent::VoteCast(VoteRecord {
                                    voter,
                                    proposal_id: proposal_id.clone(),
                                    choice,
                                    voting_power: weight.parse().unwrap_or(0),
                                    timestamp: HLCTimestamp::now(),
                                }))
                            },
                            _ => None,
                        };

                        if let Some(event) = gov_event {
                            let _ = self.broadcast_event(event).await;
                        }
                    }
                },
                Err(e) => {
                    println!("Failed to index proposals: {}", e);
                }
            }
        }

        self.is_indexing = false;
        Ok(())
    }

    async fn start_keepalive_timer(&mut self) -> Result<(), String> {
        // Start a timer to send keepalives every minute
        let _ = timer::set_timer(60000, Some(vec![1])); // 1 minute timer with context [1]
        Ok(())
    }

    async fn send_keepalive(&mut self) -> Result<(), String> {
        // Send keepalive ping to all committee members
        let our_node = our().node.clone();
        let state_hash = self.compute_state_hash();
        let timestamp = current_timestamp();

        for member in self.committee_members.clone() {
            if member == our_node {
                continue;
            }

            let ping = P2PMessage::Ping {
                timestamp,
                state_hash: state_hash.clone(),
                vector_clock: self.crdt_state.vector_clock.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                available_capacity: 100 - self.subscriptions.len() as u32,
            };

            // Fire and forget - we don't wait for responses
            let _ = self.send_p2p_message(&member, ping).await;
        }

        // Clean up inactive peers
        let cutoff = timestamp.saturating_sub(300_000); // 5 minutes
        self.peers.retain(|_, peer| peer.last_seen > cutoff);

        Ok(())
    }

    async fn broadcast_event(&mut self, event: GovernanceEvent) -> Result<(), String> {
        // Use committee state if available
        if let Some(committee_state) = &mut self.committee_state {
            // Apply event to committee CRDT if we're a member
            if committee_state.is_committee_member {
                committee_state.crdt.apply_event(event.clone())?;

                // Save state to storage
                if let Some(storage) = &self.storage {
                    let _ = storage.save_committee_state(committee_state);
                    let _ = storage.save_crdt_state(&committee_state.crdt);
                }
            }

            // Sign the event for authenticity
            let _event_bytes = bincode::serialize(&event)
                .unwrap_or_default();
            // Sign the event - in production, use proper signing
            let signature = vec![];

            let state_update = P2PMessage::StateUpdate {
                event,
                vector_clock: committee_state.crdt.vector_clock.to_vec(),
                signature,
                propagation_path: vec![our().node.clone()],
            };

            // Broadcast to committee members (both members and non-members can broadcast)
            let _ = p2p_committee::broadcast_to_committee(
                &committee_state.crdt.committee_members,
                state_update,
                Some(&our().node)
            ).await;
        } else {
            // Fallback to old CRDT if committee state not initialized
            self.apply_event_to_crdt(event);
        }

        Ok(())
    }

    fn apply_event_to_crdt(&mut self, event: GovernanceEvent) {
        match event {
            GovernanceEvent::ProposalCreated(data) => {
                self.crdt_state.proposals.insert(data.id.clone());
                self.crdt_state.proposal_data.insert(data.id.clone(), data);
            },
            GovernanceEvent::VoteCast(vote) => {
                // VoteRecord already includes proposal_id
                self.crdt_state.vote_records
                    .entry(vote.proposal_id.clone())
                    .or_insert_with(HashSet::new)
                    .insert(vote);
            },
            GovernanceEvent::DiscussionAdded(msg) => {
                self.crdt_state.discussions
                    .entry(msg.id.clone())
                    .or_insert_with(DiscussionCRDT::new)
                    .messages.push(msg);
            },
            GovernanceEvent::CommitteeMemberAdded(member) => {
                self.crdt_state.committee.insert(member);
            },
            GovernanceEvent::CommitteeMemberRemoved(member) => {
                self.crdt_state.committee.remove(&member);
            },
        }

        self.crdt_state.update_merkle_root();
    }

    async fn send_p2p_message(&self, target_node: &str, message: P2PMessage) -> Result<String, String> {
        let target = Address::new(target_node, OUR_PROCESS);

        let request = Request::to(target)
            .body(serde_json::to_vec(&message).unwrap());

        match send::<String>(request).await {
            Ok(response) => Ok(response),
            Err(e) => Err(format!("P2P send failed: {:?}", e))
        }
    }

    async fn send_state_update(&self, target_node: &str, update: StateUpdate) -> Result<(), String> {
        let target = Address::new(target_node, OUR_PROCESS);

        let request = Request::to(target)
            .body(serde_json::to_vec(&json!({
                "handle_state_update": serde_json::to_string(&update).unwrap()
            })).unwrap());

        let _ = send::<String>(request).await;
        Ok(())
    }


    async fn initiate_sync(&mut self, peer_node: String, state_hash: String) -> Result<(), String> {
        // Check if state hashes match
        if state_hash == self.compute_state_hash() {
            return Ok(()); // Already in sync
        }

        // Request full sync from peer
        let sync_request = caller_utils::SyncRequest {
            sync_type: caller_utils::SyncType::Full,
            max_events: None,
        };

        let target = Address::new(&peer_node, OUR_PROCESS);

        match caller_utils::governance::handle_sync_request_remote_rpc(&target, sync_request).await {
            Ok(Ok(response)) => {
                match response {
                    caller_utils::SyncResponse::Full(snapshot) => {
                        // Deserialize and apply snapshot
                        if let Ok(crdt) = bincode::deserialize::<GovernanceCRDT>(&snapshot) {
                            self.crdt_state = crdt;

                            // Save to storage
                            if let Some(storage) = &self.storage {
                                let _ = storage.save_crdt_state(&self.crdt_state);
                            }

                            // Successfully synced state
                        }
                    },
                    _ => {
                        // Unexpected sync response type
                    }
                }
            },
            _ => {
                // Failed to sync from peer
            }
        }

        Ok(())
    }

    async fn trigger_consensus_if_needed(&mut self) -> Result<(), String> {
        if let Some(committee_state) = &mut self.committee_state {
            if !committee_state.is_committee_member {
                return Ok(());
            }

            // Check if there are pending proposals needing consensus
            for proposal in self.proposal_drafts.values() {
                if proposal.signatures.len() < (self.committee_members.len() / 2) {
                    // Needs more signatures - initiate consensus
                    let event = GovernanceEvent::ProposalCreated(ProposalData {
                        id: proposal.id.clone(),
                        title: proposal.title.clone(),
                        description: proposal.description.clone(),
                        author: proposal.author.clone(),
                        status: ProposalStatus::Pending,
                        voting_start: HLCTimestamp {
                            wall_time: current_timestamp(),
                            logical: 0,
                            node_id: our().node.clone(),
                        },
                        voting_end: HLCTimestamp {
                            wall_time: current_timestamp() + 7 * 24 * 3600 * 1000,
                            logical: 0,
                            node_id: our().node.clone(),
                        },
                        completion_time: None,
                    });

                    let _ = committee_state.initiate_consensus(event).await;
                }
            }
        }

        Ok(())
    }
}

