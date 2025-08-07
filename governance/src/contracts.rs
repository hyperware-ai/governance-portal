use serde::{Deserialize, Serialize};
use hyperware_app_common::hyperware_process_lib::eth;

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
    // Function selector for propose(address[],uint256[],bytes[],string)
    let mut data = vec![0xda, 0x95, 0x69, 0x1a];
    
    // Encode dynamic arrays and string
    // ABI encoding: offset to targets, offset to values, offset to calldatas, offset to description
    // Then the actual data for each
    
    // Calculate offsets (each offset is 32 bytes)
    let offset_base = 4 * 32; // 4 parameters
    let mut current_offset = offset_base;
    
    // Write offsets
    data.extend_from_slice(&encode_uint256(current_offset as u64)); // targets offset
    current_offset += 32 + 32 * targets.len(); // length + items
    
    data.extend_from_slice(&encode_uint256(current_offset as u64)); // values offset  
    current_offset += 32 + 32 * values.len();
    
    data.extend_from_slice(&encode_uint256(current_offset as u64)); // calldatas offset
    let calldatas_size = 32 + 32 * calldatas.len() + calldatas.iter().map(|d| 32 + ((hex::decode(d).unwrap_or_default().len() + 31) / 32) * 32).sum::<usize>();
    current_offset += calldatas_size;
    
    data.extend_from_slice(&encode_uint256(current_offset as u64)); // description offset
    
    // Encode targets array
    data.extend_from_slice(&encode_uint256(targets.len() as u64));
    for target in &targets {
        let addr = target.parse::<eth::Address>().map_err(|e| format!("Invalid address: {:?}", e))?;
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(addr.as_slice());
        data.extend_from_slice(&padded);
    }
    
    // Encode values array
    data.extend_from_slice(&encode_uint256(values.len() as u64));
    for value in &values {
        let val = value.parse::<u64>().unwrap_or(0);
        data.extend_from_slice(&encode_uint256(val));
    }
    
    // Encode calldatas array (array of bytes)
    data.extend_from_slice(&encode_uint256(calldatas.len() as u64));
    let mut calldata_offsets = Vec::new();
    let mut calldata_offset = 32 * calldatas.len();
    
    for calldata in &calldatas {
        calldata_offsets.push(calldata_offset);
        let bytes = hex::decode(calldata).unwrap_or_default();
        calldata_offset += 32 + ((bytes.len() + 31) / 32) * 32;
    }
    
    for offset in calldata_offsets {
        data.extend_from_slice(&encode_uint256(offset as u64));
    }
    
    for calldata in &calldatas {
        let bytes = hex::decode(calldata).unwrap_or_default();
        data.extend_from_slice(&encode_uint256(bytes.len() as u64));
        data.extend_from_slice(&bytes);
        // Pad to 32 bytes
        let padding = ((bytes.len() + 31) / 32) * 32 - bytes.len();
        data.extend_from_slice(&vec![0u8; padding]);
    }
    
    // Encode description string
    let desc_bytes = description.as_bytes();
    data.extend_from_slice(&encode_uint256(desc_bytes.len() as u64));
    data.extend_from_slice(desc_bytes);
    // Pad to 32 bytes
    let padding = ((desc_bytes.len() + 31) / 32) * 32 - desc_bytes.len();
    data.extend_from_slice(&vec![0u8; padding]);
    
    Ok(data)
}

fn encode_uint256(value: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    bytes
}

pub fn encode_cast_vote_call(
    proposal_id: String,
    support: u8,
    reason: Option<String>,
) -> Result<Vec<u8>, String> {
    let mut data = if reason.is_some() {
        // castVoteWithReason(uint256,uint8,string)
        vec![0x7b, 0x3c, 0x69, 0xe3]
    } else {
        // castVote(uint256,uint8)
        vec![0x56, 0x78, 0x13, 0x88]
    };
    
    // Encode proposal ID
    let id_bytes = if proposal_id.starts_with("0x") {
        hex::decode(&proposal_id[2..]).unwrap_or_else(|_| vec![0; 32])
    } else {
        let id = proposal_id.parse::<u64>().unwrap_or(0);
        encode_uint256(id).to_vec()
    };
    
    let mut padded = [0u8; 32];
    let start = 32usize.saturating_sub(id_bytes.len());
    padded[start..].copy_from_slice(&id_bytes[..id_bytes.len().min(32)]);
    data.extend_from_slice(&padded);
    
    // Encode support (uint8 padded to 32 bytes)
    let mut support_bytes = [0u8; 32];
    support_bytes[31] = support;
    data.extend_from_slice(&support_bytes);
    
    // Encode reason if provided
    if let Some(reason_text) = reason {
        // Offset to string data
        data.extend_from_slice(&encode_uint256(96)); // Fixed offset for third parameter
        
        // String length and data
        let reason_bytes = reason_text.as_bytes();
        data.extend_from_slice(&encode_uint256(reason_bytes.len() as u64));
        data.extend_from_slice(reason_bytes);
        
        // Pad to 32 bytes
        let padding = ((reason_bytes.len() + 31) / 32) * 32 - reason_bytes.len();
        data.extend_from_slice(&vec![0u8; padding]);
    }
    
    Ok(data)
}

pub fn decode_proposal_state(data: Vec<u8>) -> Result<ProposalState, String> {
    if data.is_empty() {
        return Err("Empty data".to_string());
    }
    
    // Decode the uint8 from the response
    // In a real implementation, this would properly decode the ABI response
    Ok(ProposalState::from(data[0]))
}

pub fn decode_proposal_votes(_data: Vec<u8>) -> Result<(String, String, String), String> {
    // Decode the three uint256 values
    // In a real implementation, this would properly decode the ABI response
    Ok(("0".to_string(), "0".to_string(), "0".to_string()))
}

pub fn decode_get_votes(_data: Vec<u8>) -> Result<String, String> {
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
    use alloy_primitives::keccak256;
    
    // Compute keccak256(abi.encode(targets, values, calldatas, descriptionHash))
    // Following OpenZeppelin Governor's hashProposal function
    let mut data = Vec::new();
    
    // Encode addresses array (targets)
    data.extend_from_slice(&encode_uint256(0x80)); // Offset to targets array
    
    // Encode values array offset
    let values_offset = 0x80 + 32 + 32 * targets.len();
    data.extend_from_slice(&encode_uint256(values_offset as u64));
    
    // Encode calldatas array offset
    let calldatas_offset = values_offset + 32 + 32 * values.len();
    data.extend_from_slice(&encode_uint256(calldatas_offset as u64));
    
    // Encode descriptionHash (bytes32)
    data.extend_from_slice(&description_hash);
    
    // Encode targets array
    data.extend_from_slice(&encode_uint256(targets.len() as u64));
    for target in targets {
        let addr = target.parse::<eth::Address>()
            .unwrap_or_else(|_| eth::Address::ZERO);
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(addr.as_slice());
        data.extend_from_slice(&padded);
    }
    
    // Encode values array
    data.extend_from_slice(&encode_uint256(values.len() as u64));
    for value in values {
        let val = value.parse::<u64>().unwrap_or(0);
        data.extend_from_slice(&encode_uint256(val));
    }
    
    // Encode calldatas array
    data.extend_from_slice(&encode_uint256(calldatas.len() as u64));
    let mut calldata_data = Vec::new();
    let mut calldata_offsets = Vec::new();
    
    if !calldatas.is_empty() {
        let mut offset = 32 * calldatas.len();
        for calldata in calldatas {
            calldata_offsets.push(offset);
            let bytes = if calldata.starts_with("0x") {
                hex::decode(&calldata[2..]).unwrap_or_default()
            } else {
                hex::decode(calldata).unwrap_or_default()
            };
            offset += 32 + ((bytes.len() + 31) / 32) * 32;
            calldata_data.push(bytes);
        }
        
        // Write offsets
        for offset in &calldata_offsets {
            data.extend_from_slice(&encode_uint256(*offset as u64));
        }
        
        // Write calldata bytes
        for bytes in &calldata_data {
            data.extend_from_slice(&encode_uint256(bytes.len() as u64));
            data.extend_from_slice(bytes);
            let padding = ((bytes.len() + 31) / 32) * 32 - bytes.len();
            data.extend_from_slice(&vec![0u8; padding]);
        }
    }
    
    // Compute keccak256 hash of the encoded data
    let hash = keccak256(&data);
    format!("0x{}", hex::encode(hash))
}