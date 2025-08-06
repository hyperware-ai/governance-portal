use hyperprocess_macro::*;
use hyperware_process_lib::{
    our,
    homepage::add_to_homepage,
    timer,
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

mod chain_indexer;
mod storage;
mod contracts;

use chain_indexer::{ChainIndexer, ProposalEvent, process_proposal_event};
use storage::FileStorage;

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
    
    #[serde(skip)]
    chain_indexer: Option<ChainIndexer>,
    #[serde(skip)]
    storage: Option<FileStorage>,
}

impl Default for GovernanceState {
    fn default() -> Self {
        Self {
            last_indexed_block: 0,
            chain_id: 8453, // Base mainnet
            governor_address: String::new(),
            token_address: String::new(),
            onchain_proposals: HashMap::new(),
            committee_members: HashSet::new(),
            proposal_drafts: HashMap::new(),
            discussions: HashMap::new(),
            is_committee_member: false,
            is_indexing: false,
            sync_peers: Vec::new(),
            last_state_hash: String::new(),
            crdt_state: GovernanceCRDT::default(),
            subscriptions: HashMap::new(),
            peers: HashMap::new(),
            chain_indexer: None,
            storage: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ProposalDraft {
    pub id: String,
    pub author: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub signatures: Vec<NodeSignature>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct NodeSignature {
    pub node_id: String,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct VoteRecord {
    pub voter: String,
    pub choice: VoteChoice,
    pub voting_power: u64,
    pub timestamp: HLCTimestamp,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscussionCRDT {
    messages: Vec<Message>,
    message_dag: HashMap<MessageId, Vec<MessageId>>,
    upvotes: HashMap<MessageId, u32>,
    downvotes: HashMap<MessageId, u32>,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: MessageId,
    pub author: String,
    pub content: String,
    pub timestamp: HLCTimestamp,
    pub parent_id: Option<MessageId>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    node_id: String,
    last_seen: u64,
    vector_clock: HashMap<ActorId, u64>,
    state_hash: String,
    is_committee: bool,
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
        vector_clock: HashMap<ActorId, u64>,
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
        vector_clock: HashMap<ActorId, u64>,
        available_capacity: u32,
    },
    Pong {
        timestamp: u64,
        state_hash: String,
        vector_clock: HashMap<ActorId, u64>,
        peer_list: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SyncType {
    Full,
    Delta(HashMap<ActorId, u64>),
    Proposals(Vec<ProposalId>),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionFilter {
    status: Option<ProposalStatus>,
    author: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum GovernanceEvent {
    ProposalCreated(ProposalData),
    VoteCast(VoteRecord),
    DiscussionAdded(Message),
    CommitteeMemberAdded(String),
    CommitteeMemberRemoved(String),
}

#[derive(Serialize, Deserialize)]
pub struct JoinRequest {
    node_id: String,
    public_key: Vec<u8>,
    capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub enum JoinResponse {
    Approved {
        members: HashSet<String>,
        state_hash: String,
        bootstrap_nodes: Vec<String>,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StateUpdate {
    event: GovernanceEvent,
    signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct SyncRequest {
    sync_type: SyncType,
    max_events: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub enum SyncResponse {
    Full(Vec<u8>),
    Delta(Vec<GovernanceEvent>),
    Proposals(Vec<ProposalData>),
}

#[derive(Serialize, Deserialize)]
pub struct CommitteeStatus {
    members: HashSet<String>,
    online_count: usize,
    is_member: bool,
    quorum_size: usize,
}

#[derive(Serialize, Deserialize)]
pub enum ProposalFilter {
    All,
    Active,
    ByStatus(ProposalStatus),
    ByIds(Vec<String>),
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
        Self {
            actor_id: generate_actor_id(),
            vector_clock: HashMap::new(),
            proposals: HashSet::new(),
            proposal_data: HashMap::new(),
            votes: HashMap::new(),
            vote_records: HashMap::new(),
            committee: HashSet::new(),
            discussions: HashMap::new(),
            merkle_root: String::new(),
        }
    }
}

fn generate_actor_id() -> String {
    format!("{}_{}", our().node, Uuid::new_v4())
}

fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

fn current_timestamp() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

impl GovernanceState {
    fn compute_state_hash(&self) -> String {
        let mut hasher = Sha3_256::new();
        let state_bytes = bincode::serialize(&self.crdt_state).unwrap_or_default();
        hasher.update(state_bytes);
        general_purpose::STANDARD.encode(hasher.finalize())
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
                if let Ok(crdt) = storage.load_crdt_state() {
                    self.crdt_state = crdt;
                }
                if let Ok(last_block) = storage.load_metadata() {
                    self.last_indexed_block = last_block;
                }
                self.storage = Some(storage);
            },
            Err(e) => {
                println!("Failed to initialize storage: {}", e);
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

        Ok(json!({
            "success": true,
            "draft_id": draft_id,
            "draft": draft
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

        Ok(json!({
            "success": true,
            "discussion": discussion
        }).to_string())
    }

    #[http]
    async fn get_committee_status(&self) -> Result<String, String> {
        Ok(json!({
            "members": self.committee_members,
            "is_member": self.is_committee_member,
            "online_count": self.count_online_members()
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
            .ok_or("Missing proposal_id")?;
            
        let support = req.get("support")
            .and_then(|s| s.as_u64())
            .ok_or("Missing support value")? as u8;
            
        let voter = req.get("voter")
            .and_then(|v| v.as_str())
            .ok_or("Missing voter address")?;
            
        let reason = req.get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("");
        
        // Get voting power for the voter
        let voting_power = if let Some(indexer) = &self.chain_indexer {
            match indexer.get_voting_power(voter, 0).await {
                Ok(power) => power,
                Err(_) => "0".to_string(),
            }
        } else {
            "1000000000000000000".to_string() // 1 token as default
        };
        
        // Create vote event
        let vote_event = ProposalEvent::VoteCast {
            proposal_id: proposal_id.to_string(),
            voter: voter.to_string(),
            support,
            weight: voting_power.clone(),
            reason: reason.to_string(),
            block_number: 0,
            tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        };
        
        // Process the vote
        process_proposal_event(vote_event, &mut self.onchain_proposals);
        
        // Save to storage
        if let Some(storage) = &self.storage {
            let _ = storage.save_proposals(&self.onchain_proposals);
        }
        
        // Broadcast to committee
        let gov_event = GovernanceEvent::VoteCast(VoteRecord {
            voter: voter.to_string(),
            choice: match support {
                0 => VoteChoice::No,
                1 => VoteChoice::Yes,
                _ => VoteChoice::Abstain,
            },
            voting_power: voting_power.parse().unwrap_or(0),
            timestamp: HLCTimestamp::now(),
        });
        
        let _ = self.broadcast_event(gov_event).await;
        
        Ok(json!({
            "success": true,
            "proposal_id": proposal_id,
            "voter": voter,
            "support": support,
            "voting_power": voting_power,
            "message": "Vote recorded locally. Chain submission pending."
        }).to_string())
    }

    #[http]
    async fn request_join_committee(&mut self, request_body: String) -> Result<String, String> {
        let target_nodes: Vec<String> = serde_json::from_str(&request_body)
            .map_err(|e| format!("Invalid request: {}", e))?;

        let join_request = JoinRequest {
            node_id: our().node.clone(),
            public_key: self.get_public_key(),
            capabilities: vec!["governance".to_string()],
        };

        for node in target_nodes {
            let target = Address::new(&node, ("governance", "governance", "sys"));

            let request = Request::to(target)
                .body(serde_json::to_vec(&json!({
                    "handle_join_request": serde_json::to_string(&join_request).unwrap()
                })).unwrap());

            match send::<String>(request).await {
                Ok(response) => {
                    if let Ok(join_response) = serde_json::from_str::<JoinResponse>(&response) {
                        if let JoinResponse::Approved { members, state_hash, .. } = join_response {
                            self.committee_members = members;
                            self.is_committee_member = true;
                            self.last_state_hash = state_hash;
                            return Ok("Successfully joined committee".to_string());
                        }
                    }
                },
                Err(_) => continue,
            }
        }

        Err("Failed to join committee through any node".to_string())
    }

    #[remote]
    async fn handle_join_request(&mut self, request_json: String) -> Result<String, String> {
        let request: JoinRequest = serde_json::from_str(&request_json)
            .map_err(|e| format!("Invalid request: {}", e))?;

        if !self.verify_node_credentials(&request) {
            return Ok(serde_json::to_string(&JoinResponse::Rejected {
                reason: "Invalid credentials".to_string()
            }).unwrap());
        }

        self.committee_members.insert(request.node_id.clone());

        let response = JoinResponse::Approved {
            members: self.committee_members.clone(),
            state_hash: self.compute_state_hash(),
            bootstrap_nodes: self.get_active_committee_nodes(),
        };

        Ok(serde_json::to_string(&response).unwrap())
    }

    #[remote]
    async fn handle_state_update(&mut self, update_json: String) -> Result<String, String> {
        let update: StateUpdate = serde_json::from_str(&update_json)
            .map_err(|e| format!("Invalid update: {}", e))?;

        if !self.verify_event_signature(&update.event, &update.signature) {
            return Err("Invalid signature".to_string());
        }

        // self.broadcast_to_committee_except(update.clone(), our().node.clone()).await?;

        Ok("ACK".to_string())
    }

    #[remote]
    async fn handle_sync_request(&mut self, request_json: String) -> Result<String, String> {
        let request: SyncRequest = serde_json::from_str(&request_json)
            .map_err(|e| format!("Invalid request: {}", e))?;

        let response = match request.sync_type {
            SyncType::Full => {
                let snapshot = bincode::serialize(&self.crdt_state).unwrap();
                SyncResponse::Full(snapshot)
            },
            SyncType::Delta(_since_clock) => {
                SyncResponse::Delta(vec![])
            },
            SyncType::Proposals(ids) => {
                let proposals = self.get_proposals_by_ids(ids);
                SyncResponse::Proposals(proposals)
            }
        };

        Ok(serde_json::to_string(&response).unwrap())
    }

    #[remote]
    async fn handle_subscription(&mut self, subscription_json: String) -> Result<String, String> {
        let _request: serde_json::Value = serde_json::from_str(&subscription_json)
            .map_err(|e| format!("Invalid request: {}", e))?;

        if self.subscriptions.len() >= MAX_SUBSCRIPTIONS {
            return Err("At capacity".to_string());
        }

        let subscription_id = generate_id();

        Ok(json!({
            "subscription_id": subscription_id,
            "initial_state": {}
        }).to_string())
    }

    #[remote]
    async fn handle_keepalive(&mut self, ping_json: String) -> Result<String, String> {
        let _ping: serde_json::Value = serde_json::from_str(&ping_json)
            .map_err(|e| format!("Invalid ping: {}", e))?;

        Ok(json!({
            "state_hash": self.compute_state_hash(),
            "vector_clock": {},
            "peers": self.get_active_committee_nodes()
        }).to_string())
    }

    #[local]
    #[remote]
    async fn get_committee_status_both(&self) -> Result<String, String> {
        Ok(serde_json::to_string(&CommitteeStatus {
            members: self.committee_members.clone(),
            online_count: self.count_online_members(),
            is_member: self.is_committee_member,
            quorum_size: (self.committee_members.len() as f64 * COMMITTEE_QUORUM) as usize,
        }).unwrap())
    }

    #[local]
    #[remote]
    async fn get_proposals_filtered(&self, filter_json: String) -> Result<String, String> {
        let filter: ProposalFilter = serde_json::from_str(&filter_json)
            .map_err(|e| format!("Invalid filter: {}", e))?;

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

        Ok(serde_json::to_string(&proposals).unwrap())
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
                    timer::set_timer(30000, Some(b"index_chain".to_vec()));
                },
                b"keepalive" => {
                    self.send_keepalive().await?;
                    // Schedule next keepalive
                    timer::set_timer(60000, Some(b"keepalive".to_vec()));
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
    
    async fn send_keepalive(&mut self) -> Result<(), String> {
        let ping = P2PMessage::Ping {
            timestamp: current_timestamp(),
            state_hash: self.compute_state_hash(),
            vector_clock: self.crdt_state.vector_clock.clone(),
            available_capacity: (MAX_SUBSCRIPTIONS - self.subscriptions.len()) as u32,
        };
        
        // Send to all committee peers
        for peer in self.committee_members.iter() {
            if peer != &our().node {
                let _ = self.send_p2p_message(peer, ping.clone()).await;
            }
        }
        
        // Clean up stale peers
        let now = current_timestamp();
        self.peers.retain(|_, info| {
            now - info.last_seen < 300000 // 5 minutes
        });
        
        Ok(())
    }
    
    async fn broadcast_event(&mut self, event: GovernanceEvent) -> Result<(), String> {
        let update = StateUpdate {
            event: event.clone(),
            signature: vec![],
        };
        
        for member in self.committee_members.iter() {
            if member != &our().node {
                let _ = self.send_state_update(member, update.clone()).await;
            }
        }
        
        // Apply to local CRDT
        self.apply_event_to_crdt(event);
        
        // Save CRDT state
        if let Some(storage) = &self.storage {
            let _ = storage.save_crdt_state(&self.crdt_state);
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
                // For simplicity, just tracking votes in a basic way
                // Real CRDT implementation would use GCounter
                self.crdt_state.vote_records
                    .entry(vote.voter.clone())
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
        let target = Address::new(target_node, ("governance", "governance", "sys"));
        
        let request = Request::to(target)
            .body(serde_json::to_vec(&message).unwrap());
            
        match send::<String>(request).await {
            Ok(response) => Ok(response),
            Err(e) => Err(format!("P2P send failed: {:?}", e))
        }
    }
    
    async fn send_state_update(&self, target_node: &str, update: StateUpdate) -> Result<(), String> {
        let target = Address::new(target_node, ("governance", "governance", "sys"));
        
        let request = Request::to(target)
            .body(serde_json::to_vec(&json!({
                "handle_state_update": serde_json::to_string(&update).unwrap()
            })).unwrap());
            
        let _ = send::<String>(request).await;
        Ok(())
    }
}
