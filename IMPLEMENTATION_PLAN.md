# DAO Governance Portal Implementation Plan

## Overview

This document provides a comprehensive implementation plan for building a decentralized DAO governance portal as a Hyperware application. The portal will enable DAO members to create proposals, vote, and participate in governance discussions using a peer-to-peer architecture where each member runs their own node.

## Architecture

### Core Components

1. **Backend (Rust)**
   - Hyperprocess macro-based state management
   - P2P networking for committee consensus
   - Chain indexing for on-chain proposals
   - CRDT-based state synchronization
   - HTTP endpoints for UI interaction

2. **Frontend (React/TypeScript)**
   - Wallet connection (RainbowKit/wagmi)
   - Zustand for state management
   - Real-time proposal and vote tracking
   - Discussion thread interface

3. **Storage**
   - SQLite for indexed chain data
   - VFS for proposal drafts and attachments
   - In-memory CRDT state for real-time data

## Implementation Steps

### Phase 1: Backend Core Infrastructure

#### 1.1 Project Setup and Naming
- Rename `skeleton-app` to `governance-app` throughout the project
- Update `metadata.json` with DAO governance app details
- Configure package name as `governance:governance:sys`

#### 1.2 State Structure (`governance-app/src/lib.rs`)

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct GovernanceState {
    // Chain indexing state
    last_indexed_block: u64,
    chain_id: u64,
    governor_address: String,
    token_address: String,
    
    // On-chain proposals (indexed from chain)
    onchain_proposals: HashMap<String, OnchainProposal>,
    
    // Off-chain state (CRDT)
    committee_members: HashSet<String>,
    proposal_drafts: HashMap<String, ProposalDraft>,
    discussions: HashMap<String, Vec<Discussion>>,
    
    // Local node state
    is_committee_member: bool,
    is_indexing: bool,
    sync_peers: Vec<String>,
    last_state_hash: String,
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
pub struct ProposalDraft {
    pub id: String,
    pub author: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub signatures: Vec<NodeSignature>,
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
```

#### 1.3 Chain Indexing Module

Create a separate module for chain indexing similar to `hypermap-cacher`:

```rust
// Key functions to implement:
async fn index_proposals(&mut self) -> Result<(), String>
async fn fetch_proposal_events(&self, from_block: u64, to_block: u64) -> Vec<ProposalEvent>
async fn get_proposal_state(&self, proposal_id: &str) -> OnchainProposal
async fn get_voting_power(&self, address: &str, block: u64) -> String
```

#### 1.4 P2P Committee Module with #[remote] Methods

Implement committee consensus using P2P messaging with proper #[remote] endpoints:

```rust
use hyperware_app_common::{send, send_rmp, source};
use uuid::Uuid;

// ===== Remote P2P Endpoints (callable by other nodes) =====

#[remote]
async fn handle_join_request(&mut self, request_json: String) -> Result<String, String> {
    let request: JoinRequest = serde_json::from_str(&request_json)?;
    
    // Verify the requester's credentials
    if !self.verify_node_credentials(&request) {
        return Ok(serde_json::to_string(&JoinResponse::Rejected {
            reason: "Invalid credentials".to_string()
        })?);
    }
    
    // Add to committee if approved
    self.committee_members.insert(request.node_id.clone());
    
    // Send current state hash for sync
    let response = JoinResponse::Approved {
        members: self.committee_members.clone(),
        state_hash: self.compute_state_hash(),
        bootstrap_nodes: self.get_active_committee_nodes(),
    };
    
    Ok(serde_json::to_string(&response)?)
}

#[remote]
async fn handle_state_update(&mut self, update_json: String) -> Result<String, String> {
    let update: StateUpdate = serde_json::from_str(&update_json)?;
    
    // Verify signature
    if !self.verify_event_signature(&update.event, &update.signature) {
        return Err("Invalid signature".to_string());
    }
    
    // Apply CRDT operation
    self.crdt_state.apply_operation(update.event.into())?;
    
    // Broadcast to other committee members (except sender)
    self.broadcast_to_committee_except(update, source().node).await?;
    
    Ok("ACK".to_string())
}

#[remote]
async fn handle_sync_request(&mut self, request_json: String) -> Result<String, String> {
    let request: SyncRequest = serde_json::from_str(&request_json)?;
    
    let response = match request.sync_type {
        SyncType::Full => {
            // Send complete state
            SyncResponse::Full(self.crdt_state.create_snapshot())
        },
        SyncType::Delta(since_clock) => {
            // Send only changes since vector clock
            SyncResponse::Delta(self.crdt_state.delta_since(&since_clock))
        },
        SyncType::Proposals(ids) => {
            // Send specific proposals
            let proposals = self.get_proposals_by_ids(ids);
            SyncResponse::Proposals(proposals)
        }
    };
    
    Ok(serde_json::to_string(&response)?)
}

#[remote]
async fn handle_subscription(&mut self, subscription_json: String) -> Result<String, String> {
    let request: SubscriptionRequest = serde_json::from_str(&subscription_json)?;
    let subscriber = source().node;
    
    // Check if we can handle more subscriptions
    if self.subscriptions.len() >= MAX_SUBSCRIPTIONS {
        return Err("At capacity".to_string());
    }
    
    // Create subscription
    let subscription_id = generate_id();
    self.subscriptions.insert(subscription_id.clone(), Subscription {
        id: subscription_id.clone(),
        subscriber: subscriber.clone(),
        filter: request.filter,
        last_update: current_timestamp(),
    });
    
    // Send initial state based on filter
    let initial_state = self.get_filtered_state(&request.filter);
    
    Ok(serde_json::to_string(&SubscriptionResponse {
        subscription_id,
        initial_state,
    })?)
}

#[remote]
async fn handle_keepalive(&mut self, ping_json: String) -> Result<String, String> {
    let ping: Ping = serde_json::from_str(&ping_json)?;
    let sender = source().node;
    
    // Update peer info
    self.peers.insert(sender.clone(), PeerInfo {
        last_seen: current_timestamp(),
        state_hash: ping.state_hash.clone(),
        vector_clock: ping.vector_clock.clone(),
    });
    
    // Check if states diverged
    if ping.state_hash != self.compute_state_hash() {
        // Initiate sync in background
        self.queue_sync_with_peer(sender);
    }
    
    // Send pong with our state
    Ok(serde_json::to_string(&Pong {
        state_hash: self.compute_state_hash(),
        vector_clock: self.crdt_state.vector_clock.clone(),
        peers: self.get_active_peers(),
    })?)
}

// ===== Both Local and Remote (callable by local processes and other nodes) =====

#[local]
#[remote]
async fn get_committee_status(&self, _request: String) -> Result<String, String> {
    Ok(serde_json::to_string(&CommitteeStatus {
        members: self.committee_members.clone(),
        online_count: self.count_online_members(),
        is_member: self.is_committee_member,
        quorum_size: (self.committee_members.len() as f64 * COMMITTEE_QUORUM) as usize,
    })?)
}

#[local]
#[remote]
async fn get_proposals(&self, filter_json: String) -> Result<String, String> {
    let filter: ProposalFilter = serde_json::from_str(&filter_json)?;
    
    let proposals = match filter {
        ProposalFilter::All => self.get_all_proposals(),
        ProposalFilter::Active => self.get_active_proposals(),
        ProposalFilter::ByStatus(status) => self.get_proposals_by_status(status),
        ProposalFilter::ByIds(ids) => self.get_proposals_by_ids(ids),
    };
    
    Ok(serde_json::to_string(&proposals)?)
}

// ===== Helper Functions for P2P Operations =====

async fn broadcast_to_committee(&self, message: P2PMessage) -> Result<(), String> {
    for member in &self.committee_members {
        if member != &our().node {
            // Fire and forget - don't wait for response
            let _ = self.send_p2p_message(member, message.clone());
        }
    }
    Ok(())
}

async fn send_p2p_message(&self, target_node: &str, message: P2PMessage) -> Result<String, String> {
    // Determine which remote method to call based on message type
    let (method_name, body) = match &message {
        P2PMessage::JoinRequest(_) => ("handle_join_request", serde_json::to_string(&message)?),
        P2PMessage::StateUpdate(_) => ("handle_state_update", serde_json::to_string(&message)?),
        P2PMessage::SyncRequest(_) => ("handle_sync_request", serde_json::to_string(&message)?),
        P2PMessage::Subscribe(_) => ("handle_subscription", serde_json::to_string(&message)?),
        P2PMessage::Ping(_) => ("handle_keepalive", serde_json::to_string(&message)?),
        _ => return Err("Unknown message type".to_string()),
    };
    
    // Call the remote method on target node
    let target = Address::new(target_node, ("governance", "governance", "sys"));
    
    let request = Request::to(target)
        .body(serde_json::to_vec(&json!({
            method_name: body
        }))?);
    
    // Use send() for JSON responses
    let response: String = send(request).await
        .map_err(|e| format!("P2P call failed: {:?}", e))?;
    
    Ok(response)
}

// Alternative version using rmp_serde for binary efficiency
async fn send_p2p_message_rmp(&self, target_node: &str, message: P2PMessage) -> Result<P2PResponse, String> {
    // Call the remote method on target node
    let target = Address::new(target_node, ("governance", "governance", "sys"));
    
    let request = Request::to(target)
        .body(rmp_serde::to_vec(&message)?);
    
    // Use send_rmp() for MessagePack responses
    let response: P2PResponse = send_rmp(request).await
        .map_err(|e| format!("P2P call failed: {:?}", e))?;
    
    Ok(response)
}

// ===== Subscription Update Broadcasting =====

async fn broadcast_subscription_update(&mut self, update: SubscriptionUpdate) {
    let mut failed_subscribers = Vec::new();
    
    for (sub_id, subscription) in &self.subscriptions {
        if subscription.matches_filter(&update) {
            let message = P2PMessage::SubscriptionUpdate {
                subscription_id: sub_id.clone(),
                update: update.clone(),
                sequence: self.get_next_sequence(sub_id),
            };
            
            if let Err(_) = self.send_subscription_update(&subscription.subscriber, message).await {
                failed_subscribers.push(sub_id.clone());
            }
        }
    }
    
    // Clean up failed subscriptions
    for sub_id in failed_subscribers {
        self.subscriptions.remove(&sub_id);
    }
}

// Remote method for receiving subscription updates (on subscriber side)
#[remote]
async fn receive_subscription_update(&mut self, update_json: String) -> Result<String, String> {
    let update: SubscriptionUpdate = serde_json::from_str(&update_json)?;
    
    // Process the update
    match update.update_type {
        UpdateType::ProposalCreated(proposal) => {
            self.handle_new_proposal(proposal);
        },
        UpdateType::VoteCast(vote) => {
            self.handle_new_vote(vote);
        },
        UpdateType::StateChanged(state_hash) => {
            // Queue sync if our state differs
            if state_hash != self.compute_state_hash() {
                self.queue_sync_with_peer(source().node);
            }
        },
        _ => {}
    }
    
    Ok("ACK".to_string())
}

// ===== Initiating P2P Calls from Local Methods =====

// Example: Joining committee from UI
#[http]
async fn request_join_committee(&mut self, target_nodes_json: String) -> Result<String, String> {
    let target_nodes: Vec<String> = serde_json::from_str(&target_nodes_json)?;
    
    let join_request = JoinRequest {
        node_id: our().node.clone(),
        public_key: self.get_public_key(),
        capabilities: vec!["governance".to_string()],
    };
    
    // Try joining through multiple nodes
    for node in target_nodes {
        let target = Address::new(&node, ("governance", "governance", "sys"));
        
        let request = Request::to(target)
            .body(serde_json::to_vec(&json!({
                "handle_join_request": serde_json::to_string(&join_request)?
            }))?);
        
        match send::<String>(request).await {
            Ok(response) => {
                if let Ok(join_response) = serde_json::from_str::<JoinResponse>(&response) {
                    if let JoinResponse::Approved { members, state_hash, .. } = join_response {
                        self.committee_members = members;
                        self.is_committee_member = true;
                        self.initiate_sync(node, state_hash).await?;
                        return Ok("Successfully joined committee".to_string());
                    }
                }
            },
            Err(e) => {
                warn!("Failed to join through node {}: {:?}", node, e);
                continue;
            }
        }
    }
    
    Err("Failed to join committee through any node".to_string())
}

// Example: Syncing state with a specific peer
async fn sync_with_peer(&mut self, peer_node: String) -> Result<(), String> {
    let sync_request = SyncRequest {
        sync_type: SyncType::Delta(self.crdt_state.vector_clock.clone()),
        max_events: Some(1000),
    };
    
    let target = Address::new(&peer_node, ("governance", "governance", "sys"));
    
    let request = Request::to(target)
        .body(rmp_serde::to_vec(&json!({
            "handle_sync_request": sync_request
        }))?);
    
    // Use rmp for efficiency when syncing large state
    let response: SyncResponse = send_rmp(request).await
        .map_err(|e| format!("Sync failed: {:?}", e))?;
    
    // Apply the sync response
    match response {
        SyncResponse::Delta(delta) => {
            self.apply_delta(delta)?;
        },
        SyncResponse::Full(snapshot) => {
            self.restore_from_snapshot(snapshot)?;
        },
        _ => {}
    }
    
    Ok(())
}
```

#### 1.5 HTTP Endpoints

```rust
#[http]
async fn ready(&self) -> String {
    json!({ "ready": !self.is_indexing }).to_string()
}

#[http]
async fn get_proposals(&self) -> String {
    json!({
        "onchain": self.onchain_proposals.values().collect::<Vec<_>>(),
        "drafts": self.proposal_drafts.values().collect::<Vec<_>>()
    }).to_string()
}

#[http]
async fn create_draft(&mut self, request: String) -> Result<String, String> {
    // Parse request, create draft, broadcast to committee
}

#[http]
async fn add_discussion(&mut self, request: String) -> Result<String, String> {
    // Add discussion item, broadcast to committee
}

#[http]
async fn get_committee_status(&self) -> String {
    json!({
        "members": self.committee_members,
        "is_member": self.is_committee_member,
        "online_count": self.get_online_committee_members().len()
    }).to_string()
}

#[http]
async fn get_voting_power_info(&self, request: String) -> String {
    // Query chain for voting power information
}
```

### Phase 2: Frontend Implementation

#### 2.1 Project Structure Updates

```
ui/
├── src/
│   ├── components/
│   │   ├── ProposalList/
│   │   ├── ProposalDetail/
│   │   ├── CreateProposal/
│   │   ├── DiscussionThread/
│   │   ├── VotingPanel/
│   │   ├── DaoInfo/
│   │   └── CommitteeStatus/
│   ├── hooks/
│   │   ├── useWallet.ts
│   │   ├── useGovernance.ts
│   │   └── useDiscussion.ts
│   ├── store/
│   │   └── governance.ts
│   ├── types/
│   │   └── governance.ts
│   └── utils/
│       ├── api.ts
│       └── wallet.ts
```

#### 2.2 Wallet Integration

Install and configure RainbowKit/wagmi:

```typescript
// App.tsx setup
import { WagmiProvider } from 'wagmi';
import { RainbowKitProvider } from '@rainbow-me/rainbowkit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Configure chains and wallet connectors
```

#### 2.3 State Management (store/governance.ts)

```typescript
interface GovernanceStore {
  // State
  proposals: Proposal[];
  drafts: ProposalDraft[];
  discussions: Map<string, Discussion[]>;
  committeeMembers: string[];
  userVotingPower: string;
  totalVotingPower: string;
  isLoading: boolean;
  isSyncing: boolean;
  
  // Actions
  fetchProposals: () => Promise<void>;
  createDraft: (draft: CreateDraftData) => Promise<void>;
  submitProposal: (proposalId: string) => Promise<void>;
  castVote: (proposalId: string, choice: VoteChoice) => Promise<void>;
  addDiscussion: (proposalId: string, content: string, parentId?: string) => Promise<void>;
  syncWithCommittee: () => Promise<void>;
  connectWallet: () => Promise<void>;
}
```

#### 2.4 Main UI Components

**ProposalList Component:**
- Display active, pending, and completed proposals
- Filter and sort capabilities
- Quick vote buttons
- Status indicators

**ProposalDetail Component:**
- Full proposal information
- Timeline visualization
- Vote breakdown charts
- Discussion thread

**CreateProposal Component:**
- Form for title, description
- Target contract configuration
- Preview mode
- Save as draft / Submit to chain

**DiscussionThread Component:**
- Hierarchical comment display
- Reply functionality
- Upvote/downvote system
- Sort by time/votes/depth

**DaoInfo Component:**
- User voting power display
- Total DAO metrics
- Contract addresses
- Committee status

### Phase 3: Chain Integration

#### 3.1 Contract ABIs

Define ABIs for interacting with the HyperwareGovernor contract:

```rust
// Define event structures
#[derive(Serialize, Deserialize)]
pub struct ProposalCreatedEvent {
    pub proposal_id: String,
    pub proposer: String,
    pub targets: Vec<String>,
    pub values: Vec<String>,
    pub signatures: Vec<String>,
    pub calldatas: Vec<String>,
    pub description: String,
}

// Add other events: VoteCast, ProposalQueued, ProposalExecuted, etc.
```

#### 3.2 Chain Indexing Logic

Implement periodic chain scanning:

```rust
// Set up timer for periodic indexing
timer::set_timer(30000, Some(b"index_chain".to_vec()));

// Handle timer events
if message.context() == Some(b"index_chain") {
    self.index_new_blocks().await?;
    timer::set_timer(30000, Some(b"index_chain".to_vec()));
}
```

### Phase 4: Advanced P2P Committee Network & CRDT Implementation

#### 4.1 CRDT Library Selection and Setup

##### Recommended CRDT Library: rust-crdt

Add to `Cargo.toml`:
```toml
[dependencies]
# Primary CRDT library - provides comprehensive CRDT types
rust-crdt = "7.0"
# For hybrid logical clocks (better than simple timestamps)
datacake-crdt = "0.3"
# For efficient serialization
bincode = "1.3"
serde = { version = "1.0", features = ["derive"] }
# For cryptographic operations
sha3 = "0.10"
ed25519-dalek = "2.0"
```

**Why rust-crdt?**
- Comprehensive CRDT types (G-Counter, PN-Counter, LWW-Register, OR-Set)
- Both state-based (CvRDT) and operation-based (CmRDT) replication
- Well-tested and production-ready
- Good serialization support for network transmission

#### 4.2 CRDT State Architecture

```rust
use crdts::{CmRDT, CvRDT, Dot, VClock, LWWReg, ORSet, GCounter};
use datacake_crdt::HLCTimestamp;

#[derive(Serialize, Deserialize, Clone)]
pub struct GovernanceCRDT {
    // Unique actor ID for this node
    actor_id: ActorId,
    
    // Vector clock for causal ordering
    vector_clock: VClock<ActorId>,
    
    // Proposals as OR-Set (handles add/remove with convergence)
    proposals: ORSet<ProposalId, ActorId>,
    
    // Proposal metadata as LWW-Registers (last-write-wins for updates)
    proposal_data: HashMap<ProposalId, LWWReg<ProposalData, HLCTimestamp>>,
    
    // Vote counting using G-Counters (monotonic increase only)
    votes: HashMap<ProposalId, VoteCounters>,
    
    // Individual vote records for deduplication
    vote_records: HashMap<ProposalId, ORSet<VoteRecord, ActorId>>,
    
    // Committee membership as OR-Set
    committee: ORSet<MemberId, ActorId>,
    
    // Discussion threads with CRDT ordering
    discussions: HashMap<ProposalId, DiscussionCRDT>,
    
    // Merkle root for efficient state comparison
    merkle_root: MerkleRoot,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VoteCounters {
    yes_votes: GCounter<ActorId>,      // Can only increase
    no_votes: GCounter<ActorId>,       // Can only increase
    abstain_votes: GCounter<ActorId>,  // Can only increase
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscussionCRDT {
    // Messages as append-only log with causal ordering
    messages: Vec<Message>,
    // DAG structure for threading
    message_dag: HashMap<MessageId, Vec<MessageId>>,
    // Reactions using G-Counters
    upvotes: HashMap<MessageId, GCounter<ActorId>>,
    downvotes: HashMap<MessageId, GCounter<ActorId>>,
}
```

#### 4.3 Detailed P2P Protocol Specification

##### 4.3.1 Message Types and Protocol Flow

```rust
#[derive(Serialize, Deserialize, Clone)]
pub enum P2PMessage {
    // ===== Committee Management Messages =====
    JoinRequest {
        node_id: String,
        public_key: Vec<u8>,
        capabilities: Vec<String>,
        proof_of_stake: Option<Vec<u8>>, // Optional proof of token holding
    },
    JoinApproved {
        members: Vec<CommitteeMember>,
        current_state_hash: String,
        sync_endpoints: Vec<String>,
        bootstrap_nodes: Vec<String>,
    },
    JoinRejected {
        reason: String,
        retry_after: Option<u64>,
    },
    LeaveNotification {
        node_id: String,
        handover_to: Option<String>,
        final_state_hash: String,
    },
    
    // ===== State Synchronization Messages =====
    StateUpdate {
        event: GovernanceEvent,
        vector_clock: VectorClock,
        signature: Vec<u8>,
        propagation_path: Vec<String>, // Track message path for loop prevention
    },
    StateRequest {
        from_vector_clock: Option<VectorClock>,
        request_type: SyncRequestType,
        max_events: Option<u32>,
    },
    StateResponse {
        events: Vec<GovernanceEvent>,
        current_vector_clock: VectorClock,
        merkle_root: String,
        truncated: bool, // If response was truncated due to size
    },
    StateDelta {
        operations: Vec<CRDTOperation>,
        from_clock: VectorClock,
        to_clock: VectorClock,
        compressed: bool,
        compression_type: Option<CompressionType>,
    },
    
    // ===== Subscription Management for Public Nodes =====
    Subscribe {
        subscriber: String,
        subscription_type: SubscriptionType,
        filter: Option<SubscriptionFilter>,
        delivery_mode: DeliveryMode, // Push, Pull, or Hybrid
    },
    Unsubscribe {
        subscriber: String,
        subscription_id: String,
    },
    SubscriptionUpdate {
        subscription_id: String,
        update_type: UpdateType,
        data: Vec<u8>,
        sequence_number: u64, // For ordering and dedup
    },
    SubscriptionAck {
        subscription_id: String,
        last_received_sequence: u64,
        missing_sequences: Vec<u64>, // Request retransmission
    },
    
    // ===== Health, Keepalive, and Discovery =====
    Ping {
        timestamp: u64,
        state_hash: String,
        vector_clock: VectorClock,
        load_metrics: LoadMetrics,
        available_capacity: u32, // How many more subscribers can handle
    },
    Pong {
        timestamp: u64,
        state_hash: String,
        vector_clock: VectorClock,
        peer_list: Vec<PeerInfo>, // Share known peers for discovery
    },
    HealthCheck {
        requester: String,
        include_metrics: bool,
        check_type: HealthCheckType,
    },
    HealthResponse {
        status: NodeStatus,
        metrics: Option<NodeMetrics>,
        peer_count: u32,
        sync_status: SyncStatus,
        uptime: u64,
        last_sync: u64,
    },
    
    // ===== Bulk Sync for Recovery =====
    SnapshotRequest {
        requester: String,
        from_timestamp: Option<HLCTimestamp>,
    },
    SnapshotResponse {
        snapshot_data: Vec<u8>,
        timestamp: HLCTimestamp,
        chunk_index: u32,
        total_chunks: u32,
        checksum: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SyncRequestType {
    FullSync,           // Request complete state
    DeltaSync,          // Request changes since vector clock
    ProposalSync(Vec<ProposalId>), // Specific proposals
    RecentSync(u64),    // Last N events
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SubscriptionType {
    AllProposals,
    ProposalById(String),
    ProposalsByStatus(ProposalStatus),
    CommitteeUpdates,
    DiscussionsByProposal(String),
    VotingPowerChanges,
    CustomFilter(String), // JSON filter expression
}

#[derive(Serialize, Deserialize, Clone)]
pub enum DeliveryMode {
    Push,       // Server pushes updates immediately
    Pull,       // Client polls for updates
    Hybrid {    // Push with pull fallback
        push_timeout: u64,
        pull_interval: u64,
    },
}
```

##### 4.3.2 Protocol State Machine and Timing

```rust
// Protocol timing constants
const KEEPALIVE_INTERVAL_MS: u64 = 60_000;      // 1 minute regular ping
const KEEPALIVE_TIMEOUT_MS: u64 = 300_000;      // 5 minutes before marking dead
const SYNC_BATCH_SIZE: usize = 100;             // Events per sync message
const MAX_SUBSCRIPTION_QUEUE: usize = 1000;     // Max queued updates per subscriber
const COMMITTEE_QUORUM: f64 = 0.51;             // 51% for committee decisions
const STATE_SNAPSHOT_INTERVAL_MS: u64 = 3_600_000; // 1 hour between snapshots

#[derive(Debug, Clone)]
pub enum NodeRole {
    Bootstrapping,      // Initial sync from committee
    CommitteeMember,    // Full participant in consensus
    Observer,           // Receives updates but doesn't participate
    Candidate,          // Requesting to join committee
    Leaving,           // Graceful shutdown in progress
}

pub struct P2PProtocolState {
    role: NodeRole,
    node_id: String,
    peers: HashMap<String, PeerInfo>,
    subscriptions: HashMap<String, Subscription>,
    pending_syncs: VecDeque<SyncRequest>,
    keepalive_timers: HashMap<String, u64>,
    
    // Performance tracking
    message_latencies: VecDeque<(String, u64)>, // Rolling window
    sync_performance: SyncPerformanceMetrics,
    
    // Rate limiting
    rate_limiters: HashMap<String, RateLimiter>,
}

pub struct PeerInfo {
    node_id: String,
    role: NodeRole,
    last_seen: u64,
    vector_clock: VectorClock,
    state_hash: String,
    is_committee: bool,
    connection_quality: f32, // 0.0 to 1.0
    rtt_ms: Option<u32>,     // Round trip time
    subscriptions_served: u32,
}

pub struct Subscription {
    id: String,
    subscriber: String,
    subscription_type: SubscriptionType,
    created_at: u64,
    last_update: u64,
    last_ack: u64,
    update_queue: VecDeque<QueuedUpdate>,
    delivery_mode: DeliveryMode,
    retry_count: u32,
}
```

##### 4.3.3 Subscription and Keepalive Implementation

```rust
impl P2PProtocolState {
    // ===== Subscription Handling for Public Nodes =====
    
    async fn handle_subscription(
        &mut self, 
        subscriber: String, 
        sub_type: SubscriptionType,
        delivery_mode: DeliveryMode
    ) -> Result<String, String> {
        // Check capacity
        if self.subscriptions.len() >= MAX_SUBSCRIPTIONS {
            return Err("Node at maximum subscription capacity".to_string());
        }
        
        // Rate limit check
        if !self.check_rate_limit(&subscriber) {
            return Err("Rate limit exceeded".to_string());
        }
        
        let subscription_id = generate_subscription_id();
        
        let subscription = Subscription {
            id: subscription_id.clone(),
            subscriber: subscriber.clone(),
            subscription_type: sub_type.clone(),
            created_at: current_timestamp(),
            last_update: current_timestamp(),
            last_ack: current_timestamp(),
            update_queue: VecDeque::new(),
            delivery_mode,
            retry_count: 0,
        };
        
        self.subscriptions.insert(subscription_id.clone(), subscription);
        
        // Send initial state based on subscription type
        self.send_initial_state(subscriber, sub_type).await?;
        
        Ok(subscription_id)
    }
    
    // ===== Keepalive and Health Monitoring =====
    
    async fn keepalive_loop(&mut self) {
        let mut interval = timer::interval(KEEPALIVE_INTERVAL_MS);
        
        loop {
            interval.tick().await;
            
            // Check peer health
            for (peer_id, peer_info) in &mut self.peers {
                let elapsed = current_timestamp() - peer_info.last_seen;
                
                if elapsed > KEEPALIVE_TIMEOUT_MS {
                    // Peer is dead, initiate recovery
                    self.handle_peer_failure(peer_id).await;
                } else if elapsed > KEEPALIVE_INTERVAL_MS {
                    // Send ping with current state
                    let ping = P2PMessage::Ping {
                        timestamp: current_timestamp(),
                        state_hash: self.compute_state_hash(),
                        vector_clock: self.get_vector_clock(),
                        load_metrics: self.get_load_metrics(),
                        available_capacity: self.get_available_capacity(),
                    };
                    
                    self.send_to_peer(peer_id, ping).await;
                }
            }
            
            // Process subscription queues
            self.process_subscription_queues().await;
            
            // Check for stale subscriptions
            self.cleanup_stale_subscriptions().await;
        }
    }
    
    async fn handle_peer_failure(&mut self, failed_peer: &str) {
        warn!("Peer {} failed, initiating recovery", failed_peer);
        
        // Remove from active peers
        self.peers.remove(failed_peer);
        
        // If committee member, need to maintain quorum
        if self.role == NodeRole::CommitteeMember {
            let active_committee = self.count_active_committee_members();
            let total_committee = self.count_total_committee_members();
            
            if (active_committee as f64) / (total_committee as f64) < COMMITTEE_QUORUM {
                // Lost quorum, enter recovery mode
                self.enter_recovery_mode().await;
            }
        }
        
        // Find alternative peer for subscriptions
        for subscription in self.subscriptions.values_mut() {
            if subscription.subscriber == failed_peer {
                subscription.retry_count += 1;
                if subscription.retry_count > MAX_RETRY_COUNT {
                    // Give up on this subscription
                    self.subscriptions.remove(&subscription.id);
                }
            }
        }
        
        // Try to discover new peers
        self.discover_peers().await;
    }
    
    // ===== Efficient Update Broadcasting =====
    
    async fn broadcast_update(&mut self, update: UpdateType, data: Vec<u8>) {
        // Group subscriptions by delivery mode for efficiency
        let mut push_subscribers = Vec::new();
        let mut pull_subscribers = Vec::new();
        
        for (sub_id, subscription) in &self.subscriptions {
            if subscription.is_interested_in(&update) {
                match subscription.delivery_mode {
                    DeliveryMode::Push => push_subscribers.push((sub_id.clone(), subscription)),
                    DeliveryMode::Pull => pull_subscribers.push((sub_id.clone(), subscription)),
                    DeliveryMode::Hybrid { .. } => {
                        // Try push first
                        push_subscribers.push((sub_id.clone(), subscription));
                    }
                }
            }
        }
        
        // Send to push subscribers immediately
        for (sub_id, subscription) in push_subscribers {
            let update_msg = P2PMessage::SubscriptionUpdate {
                subscription_id: sub_id,
                update_type: update.clone(),
                data: data.clone(),
                sequence_number: self.get_next_sequence_number(&sub_id),
            };
            
            if !self.send_to_subscriber(&subscription.subscriber, update_msg.clone()).await {
                // Failed to send, queue for retry
                subscription.update_queue.push_back(QueuedUpdate {
                    message: update_msg,
                    timestamp: current_timestamp(),
                    retry_count: 0,
                });
            }
        }
        
        // Queue for pull subscribers
        for (sub_id, subscription) in pull_subscribers {
            let update_msg = P2PMessage::SubscriptionUpdate {
                subscription_id: sub_id,
                update_type: update.clone(),
                data: data.clone(),
                sequence_number: self.get_next_sequence_number(&sub_id),
            };
            
            subscription.update_queue.push_back(QueuedUpdate {
                message: update_msg,
                timestamp: current_timestamp(),
                retry_count: 0,
            });
            
            // Trim queue if too large
            while subscription.update_queue.len() > MAX_SUBSCRIPTION_QUEUE {
                subscription.update_queue.pop_front();
            }
        }
    }
    
    // ===== Committee Consensus Protocol =====
    
    async fn committee_consensus_round(&mut self, event: GovernanceEvent) -> Result<bool, String> {
        // Phase 1: Propose
        let proposal = ConsensusProposal {
            event: event.clone(),
            proposer: self.node_id.clone(),
            timestamp: HLCTimestamp::now(),
            signatures: vec![self.sign_event(&event)],
        };
        
        // Broadcast to all committee members
        let responses = self.broadcast_to_committee(P2PMessage::ConsensusPropose(proposal)).await?;
        
        // Phase 2: Collect votes
        let mut votes = vec![self.node_id.clone()]; // Self vote
        for response in responses {
            if let P2PMessage::ConsensusVote { voter, agree, .. } = response {
                if agree {
                    votes.push(voter);
                }
            }
        }
        
        // Phase 3: Commit if quorum reached
        let has_quorum = (votes.len() as f64) / (self.count_committee_members() as f64) >= COMMITTEE_QUORUM;
        
        if has_quorum {
            // Broadcast commit
            self.broadcast_to_committee(P2PMessage::ConsensusCommit {
                event: event.clone(),
                voters: votes,
            }).await?;
            
            // Apply to local state
            self.apply_event(event).await?;
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
```

##### 4.3.4 CRDT Merge and Conflict Resolution

```rust
impl GovernanceCRDT {
    // ===== Core CRDT Operations =====
    
    pub fn apply_operation(&mut self, op: CRDTOperation) -> Result<(), CRDTError> {
        // Update vector clock first
        self.vector_clock.inc(self.actor_id.clone());
        
        match op {
            CRDTOperation::AddProposal { id, data } => {
                // Add to OR-Set
                self.proposals.add(id.clone(), self.actor_id.clone());
                
                // Set metadata with HLC timestamp for ordering
                self.proposal_data.insert(
                    id,
                    LWWReg::new(data, HLCTimestamp::now())
                );
            },
            
            CRDTOperation::CastVote { proposal_id, voter, choice, voting_power } => {
                let vote_record = VoteRecord { 
                    voter: voter.clone(), 
                    choice, 
                    voting_power,
                    timestamp: HLCTimestamp::now() 
                };
                
                // Check for duplicate votes using OR-Set
                let vote_set = self.vote_records
                    .entry(proposal_id.clone())
                    .or_insert_with(|| ORSet::new());
                
                // Only count if this is a new vote
                if !vote_set.contains(&vote_record) {
                    vote_set.add(vote_record.clone(), self.actor_id.clone());
                    
                    // Update counters (G-Counter only increases)
                    let counters = self.votes
                        .entry(proposal_id)
                        .or_insert_with(VoteCounters::new);
                    
                    match choice {
                        VoteChoice::Yes => {
                            for _ in 0..voting_power {
                                counters.yes_votes.inc(self.actor_id.clone());
                            }
                        },
                        VoteChoice::No => {
                            for _ in 0..voting_power {
                                counters.no_votes.inc(self.actor_id.clone());
                            }
                        },
                        VoteChoice::Abstain => {
                            for _ in 0..voting_power {
                                counters.abstain_votes.inc(self.actor_id.clone());
                            }
                        },
                    }
                }
            },
            
            CRDTOperation::AddDiscussion { proposal_id, message } => {
                let discussion = self.discussions
                    .entry(proposal_id)
                    .or_insert_with(DiscussionCRDT::new);
                
                // Add with causal ordering
                discussion.add_message_with_ordering(message, self.actor_id.clone());
            },
            
            CRDTOperation::UpdateCommittee { action, member } => {
                match action {
                    CommitteeAction::Add => {
                        self.committee.add(member, self.actor_id.clone());
                    },
                    CommitteeAction::Remove => {
                        self.committee.rm(member, self.actor_id.clone());
                    },
                }
            },
        }
        
        // Update merkle root for efficient comparison
        self.update_merkle_root();
        Ok(())
    }
    
    // ===== State Merging =====
    
    pub fn merge(&mut self, remote: &GovernanceCRDT) -> Vec<CRDTOperation> {
        let mut applied_ops = Vec::new();
        
        // Merge proposals (OR-Set convergence)
        let proposal_ops = self.proposals.merge(&remote.proposals);
        
        // Merge proposal metadata (LWW-Register)
        for (id, remote_reg) in &remote.proposal_data {
            match self.proposal_data.get_mut(id) {
                Some(local_reg) => {
                    // LWW merge - latest timestamp wins
                    if remote_reg.read().timestamp > local_reg.read().timestamp {
                        *local_reg = remote_reg.clone();
                        applied_ops.push(CRDTOperation::UpdateProposal {
                            id: id.clone(),
                            data: remote_reg.read().clone(),
                        });
                    }
                },
                None => {
                    // New proposal
                    self.proposal_data.insert(id.clone(), remote_reg.clone());
                    applied_ops.push(CRDTOperation::AddProposal {
                        id: id.clone(),
                        data: remote_reg.read().clone(),
                    });
                }
            }
        }
        
        // Merge vote counters (G-Counter convergence)
        for (proposal_id, remote_votes) in &remote.votes {
            let local_votes = self.votes
                .entry(proposal_id.clone())
                .or_insert_with(VoteCounters::new);
            
            // G-Counter merge takes maximum of each actor's count
            local_votes.yes_votes.merge(&remote_votes.yes_votes);
            local_votes.no_votes.merge(&remote_votes.no_votes);
            local_votes.abstain_votes.merge(&remote_votes.abstain_votes);
        }
        
        // Merge vote records (OR-Set for deduplication)
        for (proposal_id, remote_records) in &remote.vote_records {
            let local_records = self.vote_records
                .entry(proposal_id.clone())
                .or_insert_with(|| ORSet::new());
            local_records.merge(remote_records);
        }
        
        // Merge committee (OR-Set)
        self.committee.merge(&remote.committee);
        
        // Merge discussions with conflict resolution
        for (proposal_id, remote_discussion) in &remote.discussions {
            let local_discussion = self.discussions
                .entry(proposal_id.clone())
                .or_insert_with(DiscussionCRDT::new);
            local_discussion.merge_with_resolution(remote_discussion);
        }
        
        // Merge vector clocks for causality
        self.vector_clock.merge(&remote.vector_clock);
        
        self.update_merkle_root();
        applied_ops
    }
    
    // ===== Delta Synchronization =====
    
    pub fn delta_since(&self, since: &VClock<ActorId>) -> StateDelta {
        let mut operations = Vec::new();
        
        // Find all operations after 'since' vector clock
        for (actor, current_dot) in self.vector_clock.iter() {
            let since_dot = since.get(actor).unwrap_or(&0);
            
            if current_dot > since_dot {
                // Include operations from this actor between since_dot and current_dot
                operations.extend(
                    self.get_operations_range(actor, *since_dot, *current_dot)
                );
            }
        }
        
        // Sort operations by HLC timestamp for causal ordering
        operations.sort_by_key(|op| op.timestamp());
        
        StateDelta {
            operations,
            from_clock: since.clone(),
            to_clock: self.vector_clock.clone(),
            compressed: false,
            compression_type: None,
        }
    }
    
    // ===== Conflict Resolution =====
    
    pub fn resolve_conflicts(&mut self) {
        // Resolve proposal conflicts (LWW)
        for (id, reg) in &mut self.proposal_data {
            // Already handled by LWW-Register
        }
        
        // Resolve discussion ordering conflicts
        for discussion in self.discussions.values_mut() {
            discussion.resolve_message_ordering();
        }
        
        // Clean up invalid votes (e.g., votes after deadline)
        self.validate_and_cleanup_votes();
    }
    
    fn validate_and_cleanup_votes(&mut self) {
        for (proposal_id, records) in &mut self.vote_records {
            if let Some(proposal_data) = self.proposal_data.get(proposal_id) {
                let proposal = proposal_data.read();
                
                // Remove votes cast outside voting period
                let valid_records: ORSet<VoteRecord, ActorId> = records
                    .read()
                    .into_iter()
                    .filter(|vote| {
                        vote.timestamp >= proposal.voting_start &&
                        vote.timestamp <= proposal.voting_end
                    })
                    .fold(ORSet::new(), |mut set, vote| {
                        set.add(vote, self.actor_id.clone());
                        set
                    });
                
                *records = valid_records;
            }
        }
    }
}

impl DiscussionCRDT {
    fn resolve_message_ordering(&mut self) {
        // Sort messages by HLC timestamp, then by author ID for determinism
        self.messages.sort_by(|a, b| {
            match a.timestamp.cmp(&b.timestamp) {
                std::cmp::Ordering::Equal => a.author.cmp(&b.author),
                other => other,
            }
        });
        
        // Rebuild DAG based on parent references
        self.rebuild_message_dag();
    }
}
```

##### 4.3.5 Garbage Collection and Optimization

```rust
impl GovernanceCRDT {
    // ===== Periodic Garbage Collection =====
    
    pub fn garbage_collect(&mut self, cutoff_time: HLCTimestamp) -> Result<(), String> {
        info!("Starting garbage collection for items before {:?}", cutoff_time);
        
        // Identify completed proposals older than cutoff
        let completed_proposals: Vec<_> = self.proposal_data
            .iter()
            .filter_map(|(id, reg)| {
                let data = reg.read();
                if (data.status == ProposalStatus::Executed || 
                    data.status == ProposalStatus::Rejected) &&
                   data.completion_time < cutoff_time {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        
        // Archive before removing
        for proposal_id in &completed_proposals {
            self.archive_proposal(proposal_id)?;
        }
        
        // Remove from active state
        for proposal_id in completed_proposals {
            self.proposals.rm(proposal_id.clone(), self.actor_id.clone());
            self.proposal_data.remove(&proposal_id);
            self.votes.remove(&proposal_id);
            self.vote_records.remove(&proposal_id);
            self.discussions.remove(&proposal_id);
        }
        
        // Compact vector clock - remove inactive actors
        self.compact_vector_clock(cutoff_time);
        
        // Clear OR-Set tombstones older than cutoff
        self.proposals.prune_tombstones(cutoff_time);
        self.committee.prune_tombstones(cutoff_time);
        
        info!("Garbage collection completed");
        Ok(())
    }
    
    // ===== Snapshot Creation for Efficient Bootstrap =====
    
    pub fn create_snapshot(&self) -> Snapshot {
        let snapshot_data = SnapshotData {
            proposals: self.proposals.clone(),
            proposal_data: self.proposal_data.clone(),
            votes: self.votes.clone(),
            committee: self.committee.clone(),
            // Exclude discussions for size (can be fetched separately)
        };
        
        Snapshot {
            timestamp: HLCTimestamp::now(),
            data: bincode::serialize(&snapshot_data).unwrap(),
            merkle_root: self.merkle_root.clone(),
            vector_clock: self.vector_clock.clone(),
            checksum: self.compute_checksum(),
        }
    }
    
    pub fn restore_from_snapshot(snapshot: &Snapshot) -> Result<Self, CRDTError> {
        // Verify checksum
        if !verify_checksum(&snapshot.data, &snapshot.checksum) {
            return Err(CRDTError::InvalidSnapshot);
        }
        
        let snapshot_data: SnapshotData = bincode::deserialize(&snapshot.data)?;
        
        Ok(GovernanceCRDT {
            actor_id: generate_actor_id(),
            vector_clock: snapshot.vector_clock.clone(),
            proposals: snapshot_data.proposals,
            proposal_data: snapshot_data.proposal_data,
            votes: snapshot_data.votes,
            vote_records: HashMap::new(), // Rebuild from votes
            committee: snapshot_data.committee,
            discussions: HashMap::new(), // Fetch separately
            merkle_root: snapshot.merkle_root.clone(),
        })
    }
}
```

### Phase 5: Storage and Persistence

#### 5.1 VFS Setup

```rust
// Create drives for different data types
let proposals_drive = vfs::create_drive(our.package_id(), "proposals", None)?;
let discussions_drive = vfs::create_drive(our.package_id(), "discussions", None)?;
let attachments_drive = vfs::create_drive(our.package_id(), "attachments", None)?;
let snapshots_drive = vfs::create_drive(our.package_id(), "snapshots", None)?;
```

#### 5.2 SQLite for Chain Data

```rust
// Create tables for indexed chain data
const INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS proposals (
    id TEXT PRIMARY KEY,
    proposer TEXT NOT NULL,
    title TEXT,
    description TEXT,
    start_block INTEGER,
    end_block INTEGER,
    votes_for TEXT,
    votes_against TEXT,
    status TEXT,
    tx_hash TEXT,
    block_number INTEGER,
    indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS votes (
    id TEXT PRIMARY KEY,
    proposal_id TEXT,
    voter TEXT,
    choice TEXT,
    voting_power TEXT,
    tx_hash TEXT,
    block_number INTEGER,
    FOREIGN KEY (proposal_id) REFERENCES proposals(id)
);

CREATE INDEX idx_proposals_status ON proposals(status);
CREATE INDEX idx_proposals_block ON proposals(block_number);
CREATE INDEX idx_votes_proposal ON votes(proposal_id);
CREATE INDEX idx_votes_voter ON votes(voter);
"#;
```

### Phase 6: Testing and Deployment

#### 6.1 Local Testing
1. Build with `kit build --hyperapp`
2. Test chain indexing with local RPC
3. Test P2P messaging between local nodes
4. Test wallet connection and voting
5. Test CRDT convergence with network partitions

#### 6.2 Deployment Configuration

Update `pkg/manifest.json`:

```json
{
  "name": "governance",
  "publisher": "governance.os",
  "version": "1.0.0",
  "description": "DAO Governance Portal",
  "request_networking": true,
  "request_capabilities": [
    "homepage:homepage:sys",
    "http-server:distro:sys",
    "vfs:distro:sys",
    "sqlite:distro:sys",
    "timer:distro:sys",
    "eth:distro:sys"
  ]
}
```

## Implementation Order

1. **Week 1: Backend Core & CRDT Setup**
   - Set up project structure
   - Integrate rust-crdt library
   - Implement basic CRDT state types
   - Create HTTP endpoints skeleton

2. **Week 2: P2P Protocol Implementation**
   - Implement message types and handlers
   - Create subscription management system
   - Add keepalive and health monitoring
   - Test multi-node communication

3. **Week 3: Chain Indexing & State Sync**
   - Implement proposal event indexing
   - Add CRDT merge operations
   - Create delta synchronization
   - Test state convergence

4. **Week 4: Committee Consensus**
   - Implement committee management
   - Add consensus protocol
   - Create snapshot mechanism
   - Test with network partitions

5. **Week 5: Frontend Development**
   - Set up React with wallet connection
   - Create proposal interfaces
   - Add voting and discussion UI
   - Implement real-time updates

6. **Week 6: Integration & Optimization**
   - Full integration testing
   - Performance optimization
   - Garbage collection tuning
   - Documentation and deployment

## Key Implementation Notes

### Security Considerations
- Validate all signatures for committee messages
- Rate limit subscription requests
- Verify voting power from chain
- Sanitize user inputs for discussions
- Implement DOS protection for P2P messages

### Performance Optimizations
- Use delta sync instead of full state transfers
- Batch CRDT operations before broadcasting
- Implement snapshot-based bootstrap
- Cache frequently accessed chain data
- Use compression for large messages

### Error Handling
- Graceful degradation if committee unavailable
- Automatic peer discovery on failures
- Retry logic with exponential backoff
- Comprehensive error logging
- User-friendly error messages

### Monitoring and Metrics
- Track CRDT convergence time
- Monitor P2P message latency
- Log subscription queue depths
- Measure state synchronization efficiency
- Track committee member availability

## Resources and References

The implementor should refer to these example applications in the `resources/` directory:
- `resources/example-apps/sign/` - For local messaging patterns
- `resources/example-apps/id/` - For identity and authentication
- `resources/example-apps/file-explorer/` - For VFS file operations
- `resources/guides/04-P2P-PATTERNS.md` - For P2P networking details
- `resources/guides/10-SQLITE-API-GUIDE.md` - For database operations

For CRDT implementation:
- rust-crdt documentation: https://github.com/rust-crdt/rust-crdt
- CRDT primer: https://crdt.tech/
- Hybrid Logical Clocks paper: https://cse.buffalo.edu/tech-reports/2014-04.pdf

## Testing Checklist

- [ ] CRDT convergence with 3+ nodes
- [ ] State sync after network partition
- [ ] Subscription delivery under load
- [ ] Keepalive timeout and recovery
- [ ] Committee consensus with node failures
- [ ] Garbage collection effectiveness
- [ ] Snapshot creation and restoration
- [ ] Delta sync efficiency
- [ ] Rate limiting effectiveness
- [ ] Memory usage under load

## Conclusion

This implementation plan provides a comprehensive roadmap for building a decentralized DAO governance portal on Hyperware with advanced CRDT-based state synchronization and robust P2P protocols. The architecture ensures eventual consistency, handles network partitions gracefully, and provides efficient subscription mechanisms for public nodes while maintaining committee consensus for critical governance operations.