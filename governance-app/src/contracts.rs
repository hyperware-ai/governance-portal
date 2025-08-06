use serde::{Deserialize, Serialize};
use serde_json::json;

// Contract addresses on Base mainnet
// IMPORTANT: Replace these with actual deployed contract addresses
pub const GOVERNOR_ADDRESS: &str = "0x1234567890123456789012345678901234567890"; // Replace with actual HyperwareGovernor address
pub const TOKEN_ADDRESS: &str = "0x2345678901234567890123456789012345678901"; // Replace with actual HYPR/gHYPR token address  
pub const TIMELOCK_ADDRESS: &str = "0x3456789012345678901234567890123456789012"; // Replace with actual timelock address

// Chain configuration
pub const CHAIN_ID: u64 = 8453; // Base mainnet
pub const RPC_URL: &str = "https://mainnet.base.org";

// Governor contract ABI (minimal subset needed for indexing and interaction)
pub const GOVERNOR_ABI: &str = r#"[
    {
        "anonymous": false,
        "inputs": [
            {"indexed": false, "name": "proposalId", "type": "uint256"},
            {"indexed": false, "name": "proposer", "type": "address"},
            {"indexed": false, "name": "targets", "type": "address[]"},
            {"indexed": false, "name": "values", "type": "uint256[]"},
            {"indexed": false, "name": "signatures", "type": "string[]"},
            {"indexed": false, "name": "calldatas", "type": "bytes[]"},
            {"indexed": false, "name": "voteStart", "type": "uint256"},
            {"indexed": false, "name": "voteEnd", "type": "uint256"},
            {"indexed": false, "name": "description", "type": "string"}
        ],
        "name": "ProposalCreated",
        "type": "event"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": true, "name": "voter", "type": "address"},
            {"indexed": false, "name": "proposalId", "type": "uint256"},
            {"indexed": false, "name": "support", "type": "uint8"},
            {"indexed": false, "name": "weight", "type": "uint256"},
            {"indexed": false, "name": "reason", "type": "string"}
        ],
        "name": "VoteCast",
        "type": "event"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": true, "name": "voter", "type": "address"},
            {"indexed": false, "name": "proposalId", "type": "uint256"},
            {"indexed": false, "name": "support", "type": "uint8"},
            {"indexed": false, "name": "weight", "type": "uint256"},
            {"indexed": false, "name": "reason", "type": "string"},
            {"indexed": false, "name": "params", "type": "bytes"}
        ],
        "name": "VoteCastWithParams",
        "type": "event"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": false, "name": "proposalId", "type": "uint256"}
        ],
        "name": "ProposalCanceled",
        "type": "event"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": false, "name": "proposalId", "type": "uint256"},
            {"indexed": false, "name": "eta", "type": "uint256"}
        ],
        "name": "ProposalQueued",
        "type": "event"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": false, "name": "proposalId", "type": "uint256"}
        ],
        "name": "ProposalExecuted",
        "type": "event"
    },
    {
        "inputs": [
            {"name": "targets", "type": "address[]"},
            {"name": "values", "type": "uint256[]"},
            {"name": "calldatas", "type": "bytes[]"},
            {"name": "description", "type": "string"}
        ],
        "name": "propose",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "proposalId", "type": "uint256"},
            {"name": "support", "type": "uint8"}
        ],
        "name": "castVote",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "proposalId", "type": "uint256"},
            {"name": "support", "type": "uint8"},
            {"name": "reason", "type": "string"}
        ],
        "name": "castVoteWithReason",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "proposalId", "type": "uint256"}
        ],
        "name": "state",
        "outputs": [{"name": "", "type": "uint8"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "proposalId", "type": "uint256"}
        ],
        "name": "proposalVotes",
        "outputs": [
            {"name": "againstVotes", "type": "uint256"},
            {"name": "forVotes", "type": "uint256"},
            {"name": "abstainVotes", "type": "uint256"}
        ],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "proposalId", "type": "uint256"}
        ],
        "name": "proposalSnapshot",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "proposalId", "type": "uint256"}
        ],
        "name": "proposalDeadline",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "proposalThreshold",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "quorum",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "account", "type": "address"},
            {"name": "timepoint", "type": "uint256"}
        ],
        "name": "getVotes",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "targets", "type": "address[]"},
            {"name": "values", "type": "uint256[]"},
            {"name": "calldatas", "type": "bytes[]"},
            {"name": "descriptionHash", "type": "bytes32"}
        ],
        "name": "hashProposal",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "pure",
        "type": "function"
    }
]"#;

// Token contract ABI (for voting power)
pub const TOKEN_ABI: &str = r#"[
    {
        "inputs": [{"name": "account", "type": "address"}],
        "name": "getVotes",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [{"name": "account", "type": "address"}],
        "name": "delegates",
        "outputs": [{"name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "totalSupply",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [{"name": "delegatee", "type": "address"}],
        "name": "delegate",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": true, "name": "delegator", "type": "address"},
            {"indexed": true, "name": "fromDelegate", "type": "address"},
            {"indexed": true, "name": "toDelegate", "type": "address"}
        ],
        "name": "DelegateChanged",
        "type": "event"
    },
    {
        "anonymous": false,
        "inputs": [
            {"indexed": true, "name": "delegate", "type": "address"},
            {"indexed": false, "name": "previousBalance", "type": "uint256"},
            {"indexed": false, "name": "newBalance", "type": "uint256"}
        ],
        "name": "DelegateVotesChanged",
        "type": "event"
    }
]"#;

// Proposal states from OpenZeppelin Governor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProposalState {
    Pending = 0,
    Active = 1,
    Canceled = 2,
    Defeated = 3,
    Succeeded = 4,
    Queued = 5,
    Expired = 6,
    Executed = 7,
}

impl From<u8> for ProposalState {
    fn from(value: u8) -> Self {
        match value {
            0 => ProposalState::Pending,
            1 => ProposalState::Active,
            2 => ProposalState::Canceled,
            3 => ProposalState::Defeated,
            4 => ProposalState::Succeeded,
            5 => ProposalState::Queued,
            6 => ProposalState::Expired,
            7 => ProposalState::Executed,
            _ => ProposalState::Pending,
        }
    }
}

// Vote support values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VoteSupport {
    Against = 0,
    For = 1,
    Abstain = 2,
}

impl From<u8> for VoteSupport {
    fn from(value: u8) -> Self {
        match value {
            0 => VoteSupport::Against,
            1 => VoteSupport::For,
            2 => VoteSupport::Abstain,
            _ => VoteSupport::Abstain,
        }
    }
}

// Event structures for parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalCreatedEvent {
    pub proposal_id: String,
    pub proposer: String,
    pub targets: Vec<String>,
    pub values: Vec<String>,
    pub signatures: Vec<String>,
    pub calldatas: Vec<String>,
    pub vote_start: u64,
    pub vote_end: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteCastEvent {
    pub voter: String,
    pub proposal_id: String,
    pub support: u8,
    pub weight: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalQueuedEvent {
    pub proposal_id: String,
    pub eta: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalExecutedEvent {
    pub proposal_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalCanceledEvent {
    pub proposal_id: String,
}

// Helper functions for encoding/decoding
pub fn encode_propose_call(
    targets: Vec<String>,
    values: Vec<String>,
    calldatas: Vec<String>,
    description: String,
) -> Result<Vec<u8>, String> {
    // This would use ethers or alloy to properly encode the function call
    // For now, return a placeholder
    Ok(vec![])
}

pub fn encode_cast_vote_call(
    proposal_id: String,
    support: u8,
    reason: Option<String>,
) -> Result<Vec<u8>, String> {
    // This would use ethers or alloy to properly encode the function call
    Ok(vec![])
}

pub fn decode_proposal_state(data: Vec<u8>) -> Result<ProposalState, String> {
    if data.is_empty() {
        return Err("Empty data".to_string());
    }
    
    // Decode the uint8 from the response
    // In a real implementation, this would properly decode the ABI response
    Ok(ProposalState::from(data[0]))
}

pub fn decode_proposal_votes(data: Vec<u8>) -> Result<(String, String, String), String> {
    // Decode the three uint256 values
    // In a real implementation, this would properly decode the ABI response
    Ok(("0".to_string(), "0".to_string(), "0".to_string()))
}

pub fn decode_get_votes(data: Vec<u8>) -> Result<String, String> {
    // Decode the uint256 value
    Ok("0".to_string())
}

// Generate a proposal ID from the proposal data (keccak256 hash)
pub fn compute_proposal_id(
    targets: &[String],
    values: &[String],
    calldatas: &[String],
    description_hash: [u8; 32],
) -> String {
    // This would compute keccak256(abi.encode(targets, values, calldatas, descriptionHash))
    // For now, return a placeholder
    format!("0x{}", hex::encode(description_hash))
}