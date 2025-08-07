use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use hyperware_app_common::hyperware_process_lib::eth;
use alloy_sol_types::{SolEvent, sol};
// Event decoding is handled by alloy_sol_types

use crate::{OnchainProposal, ProposalStatus};

// Define the Governor contract events using alloy_sol_types
sol! {
    #[derive(Debug)]
    event ProposalCreated(
        uint256 proposalId,
        address proposer,
        address[] targets,
        uint256[] values,
        string[] signatures,
        bytes[] calldatas,
        uint256 voteStart,
        uint256 voteEnd,
        string description
    );
    
    #[derive(Debug)]
    event VoteCast(
        address indexed voter,
        uint256 proposalId,
        uint8 support,
        uint256 weight,
        string reason
    );
    
    #[derive(Debug)]
    event VoteCastWithParams(
        address indexed voter,
        uint256 proposalId,
        uint8 support,
        uint256 weight,
        string reason,
        bytes params
    );
    
    #[derive(Debug)]
    event ProposalCanceled(uint256 proposalId);
    
    #[derive(Debug)]
    event ProposalQueued(uint256 proposalId, uint256 eta);
    
    #[derive(Debug)]
    event ProposalExecuted(uint256 proposalId);
}

// Chain indexer that interacts with the actual blockchain using eth provider
#[derive(Clone)]
pub struct ChainIndexer {
    pub last_indexed_block: u64,
    pub chain_id: u64,
    pub governor_address: eth::Address,
    pub token_address: eth::Address,
    pub provider: eth::Provider,
}

impl ChainIndexer {
    pub async fn new() -> Result<Self, String> {
        // Create eth provider with timeout of 60 seconds
        let provider = eth::Provider::new(8453, 60); // Base mainnet chain ID
        
        // Parse contract addresses
        let governor_address = crate::contracts::GOVERNOR_ADDRESS.parse::<eth::Address>()
            .map_err(|e| format!("Invalid governor address: {:?}", e))?;
        let token_address = crate::contracts::TOKEN_ADDRESS.parse::<eth::Address>()
            .map_err(|e| format!("Invalid token address: {:?}", e))?;
        
        Ok(Self {
            last_indexed_block: 19_000_000, // Starting block for Base mainnet
            chain_id: crate::contracts::CHAIN_ID,
            governor_address,
            token_address,
            provider,
        })
    }
    
    pub async fn index_proposals(
        &mut self,
        from_block: u64,
        to_block: Option<u64>,
    ) -> Result<Vec<ProposalEvent>, String> {
        let to_block = to_block.unwrap_or(from_block + 1000); // Default to 1000 blocks at a time
        
        // Create filter for ProposalCreated events with proper topic
        let filter = eth::Filter::new()
            .address(self.governor_address)
            .from_block(eth::BlockNumberOrTag::Number(from_block))
            .to_block(eth::BlockNumberOrTag::Number(to_block))
            .event_signature(ProposalCreated::SIGNATURE_HASH);
        
        // Get logs from chain
        let logs = self.provider.get_logs(&filter)
            .map_err(|e| format!("Failed to get logs: {:?}", e))?;
        
        // Parse logs into events
        let mut events = Vec::new();
        for log in logs {
            if let Some(event) = self.parse_proposal_created_log(&log) {
                events.push(event);
            }
        }
        
        // Also get VoteCast events with proper topics
        let vote_filter = eth::Filter::new()
            .address(self.governor_address)
            .from_block(eth::BlockNumberOrTag::Number(from_block))
            .to_block(eth::BlockNumberOrTag::Number(to_block))
            .events(vec![VoteCast::SIGNATURE, VoteCastWithParams::SIGNATURE]);
        
        let vote_logs = self.provider.get_logs(&vote_filter)
            .map_err(|e| format!("Failed to get vote logs: {:?}", e))?;
        
        for log in vote_logs {
            if let Some(event) = self.parse_vote_cast_log(&log) {
                events.push(event);
            }
        }
        
        // Get other governance events (Canceled, Queued, Executed)
        let other_filter = eth::Filter::new()
            .address(self.governor_address)
            .from_block(eth::BlockNumberOrTag::Number(from_block))
            .to_block(eth::BlockNumberOrTag::Number(to_block))
            .events(vec![
                ProposalCanceled::SIGNATURE,
                ProposalQueued::SIGNATURE,
                ProposalExecuted::SIGNATURE,
            ]);
        
        let other_logs = self.provider.get_logs(&other_filter)
            .map_err(|e| format!("Failed to get other logs: {:?}", e))?;
        
        for log in other_logs {
            if let Some(event) = self.parse_other_governance_log(&log) {
                events.push(event);
            }
        }
        
        // Sort by block number
        events.sort_by_key(|e| match e {
            ProposalEvent::Created { block_number, .. } => *block_number,
            ProposalEvent::VoteCast { block_number, .. } => *block_number,
            ProposalEvent::Canceled { block_number, .. } => *block_number,
            ProposalEvent::Queued { block_number, .. } => *block_number,
            ProposalEvent::Executed { block_number, .. } => *block_number,
        });
        
        self.last_indexed_block = to_block;
        Ok(events)
    }
    
    fn parse_other_governance_log(&self, log: &eth::Log) -> Option<ProposalEvent> {
        let topics = log.topics();
        if topics.is_empty() {
            return None;
        }
        
        let block_number = log.block_number.unwrap_or(0);
        
        // Check which event type this is by matching the topic signature
        if topics[0] == ProposalCanceled::SIGNATURE_HASH {
            if let Ok(decoded) = ProposalCanceled::decode_log_data(log.data(), false) {
                return Some(ProposalEvent::Canceled {
                    proposal_id: format!("0x{:064x}", decoded.proposalId),
                    block_number,
                });
            }
        } else if topics[0] == ProposalQueued::SIGNATURE_HASH {
            if let Ok(decoded) = ProposalQueued::decode_log_data(log.data(), false) {
                return Some(ProposalEvent::Queued {
                    proposal_id: format!("0x{:064x}", decoded.proposalId),
                    eta: decoded.eta.to::<u64>(),
                    block_number,
                });
            }
        } else if topics[0] == ProposalExecuted::SIGNATURE_HASH {
            if let Ok(decoded) = ProposalExecuted::decode_log_data(log.data(), false) {
                return Some(ProposalEvent::Executed {
                    proposal_id: format!("0x{:064x}", decoded.proposalId),
                    block_number,
                });
            }
        }
        
        None
    }
    
    fn parse_proposal_created_log(&self, log: &eth::Log) -> Option<ProposalEvent> {
        // Properly decode the ProposalCreated event using alloy_sol_types
        let decoded = match ProposalCreated::decode_log_data(log.data(), false) {
            Ok(decoded) => decoded,
            Err(e) => {
                eprintln!("Failed to decode ProposalCreated event: {:?}", e);
                return None;
            }
        };
        
        let block_number = log.block_number.unwrap_or(0);
        
        // Extract title from description (often first line or formatted)
        let title = decoded.description
            .lines()
            .next()
            .unwrap_or("Untitled Proposal")
            .to_string();
        
        Some(ProposalEvent::Created {
            proposal_id: format!("0x{:064x}", decoded.proposalId),
            proposer: format!("0x{:x}", decoded.proposer),
            title,
            description: decoded.description.clone(),
            targets: decoded.targets.iter()
                .map(|addr| format!("0x{:x}", addr))
                .collect(),
            values: decoded.values.iter()
                .map(|val| val.to_string())
                .collect(),
            calldatas: decoded.calldatas.iter()
                .map(|data| format!("0x{}", hex::encode(data)))
                .collect(),
            start_block: decoded.voteStart.to::<u64>(),
            end_block: decoded.voteEnd.to::<u64>(),
            block_number,
            tx_hash: format!("{:?}", log.transaction_hash.unwrap_or_default()),
        })
    }
    
    fn parse_vote_cast_log(&self, log: &eth::Log) -> Option<ProposalEvent> {
        // Try to decode as VoteCast or VoteCastWithParams
        let (proposal_id, voter, support, weight, reason) = 
            if let Ok(decoded) = VoteCast::decode_log_data(log.data(), true) {
                (
                    decoded.proposalId,
                    decoded.voter,
                    decoded.support,
                    decoded.weight,
                    decoded.reason,
                )
            } else if let Ok(decoded) = VoteCastWithParams::decode_log_data(log.data(), true) {
                (
                    decoded.proposalId,
                    decoded.voter,
                    decoded.support,
                    decoded.weight,
                    decoded.reason,
                )
            } else {
                eprintln!("Failed to decode VoteCast event");
                return None;
            };
        
        let block_number = log.block_number.unwrap_or(0);
        
        Some(ProposalEvent::VoteCast {
            proposal_id: format!("0x{:064x}", proposal_id),
            voter: format!("0x{:x}", voter),
            support,
            weight: weight.to_string(),
            reason,
            block_number,
            tx_hash: format!("{:?}", log.transaction_hash.unwrap_or_default()),
        })
    }
    
    pub async fn get_block_number(&self) -> Result<u64, String> {
        self.provider.get_block_number()
            .map_err(|e| format!("Failed to get block number: {:?}", e))
    }
    
    pub async fn get_proposal_state(&self, proposal_id: &str) -> Result<ProposalStatus, String> {
        // Encode state(uint256) call
        let mut data = vec![0x3e, 0x4f, 0x49, 0xe6]; // state(uint256) selector
        
        // Parse proposal ID as U256
        let id_bytes = if proposal_id.starts_with("0x") {
            hex::decode(&proposal_id[2..]).unwrap_or_else(|_| vec![0; 32])
        } else {
            proposal_id.parse::<u64>()
                .map(|n| {
                    let mut bytes = vec![0u8; 24];
                    bytes.extend_from_slice(&n.to_be_bytes());
                    bytes
                })
                .unwrap_or_else(|_| vec![0; 32])
        };
        
        // Ensure 32 bytes
        let mut padded = [0u8; 32];
        let start = 32usize.saturating_sub(id_bytes.len());
        padded[start..].copy_from_slice(&id_bytes[..id_bytes.len().min(32)]);
        data.extend_from_slice(&padded);
        
        // Call the contract
        let mut tx = eth::TransactionRequest::default();
        tx.to = Some(self.governor_address.into());
        tx.input = data.into();
        let result = self.provider.call(tx, None)
            .map_err(|e| format!("Failed to get proposal state: {:?}", e))?;
        
        // Decode uint8 result
        if !result.is_empty() {
            let state_num = result[result.len() - 1]; // Last byte is the state
            Ok(match state_num {
                0 => ProposalStatus::Pending,
                1 => ProposalStatus::Active,
                2 => ProposalStatus::Canceled,
                3 => ProposalStatus::Defeated,
                4 => ProposalStatus::Succeeded,
                5 => ProposalStatus::Queued,
                6 => ProposalStatus::Expired,
                7 => ProposalStatus::Executed,
                _ => ProposalStatus::Pending,
            })
        } else {
            Ok(ProposalStatus::Pending)
        }
    }
    
    pub async fn get_voting_power(&self, voter: &str, block_number: u64) -> Result<String, String> {
        // Query token contract for voter's balance at specific block
        let voter_address = voter.parse::<eth::Address>()
            .map_err(|e| format!("Invalid voter address: {:?}", e))?;
        
        // Encode balanceOf(address) call
        let mut data = vec![0x70, 0xa0, 0x82, 0x31]; // balanceOf selector
        data.extend_from_slice(&[0u8; 12]); // Pad address to 32 bytes
        data.extend_from_slice(voter_address.as_slice());
        
        // Make the call at specific block
        let mut tx = eth::TransactionRequest::default();
        tx.to = Some(self.token_address.into());
        tx.input = data.into();
        let result = self.provider.call(tx, Some(eth::BlockId::Number(eth::BlockNumberOrTag::Number(block_number))))
            .map_err(|e| format!("Failed to get voting power: {:?}", e))?;
        
        // Decode uint256 result
        if result.len() >= 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&result[..32]);
            let value = eth::U256::from_be_bytes(bytes);
            Ok(value.to_string())
        } else {
            Ok("0".to_string())
        }
    }
    
    pub async fn get_proposal_votes(&self, proposal_id: &str) -> Result<(String, String, String), String> {
        // Encode proposalVotes(uint256) call
        let mut data = vec![0x54, 0x4f, 0xbc, 0xac]; // proposalVotes selector
        
        // Parse and pad proposal ID
        let id_bytes = if proposal_id.starts_with("0x") {
            hex::decode(&proposal_id[2..]).unwrap_or_else(|_| vec![0; 32])
        } else {
            proposal_id.parse::<u64>()
                .map(|n| {
                    let mut bytes = vec![0u8; 24];
                    bytes.extend_from_slice(&n.to_be_bytes());
                    bytes
                })
                .unwrap_or_else(|_| vec![0; 32])
        };
        
        let mut padded = [0u8; 32];
        let start = 32usize.saturating_sub(id_bytes.len());
        padded[start..].copy_from_slice(&id_bytes[..id_bytes.len().min(32)]);
        data.extend_from_slice(&padded);
        
        // Call the contract
        let mut tx = eth::TransactionRequest::default();
        tx.to = Some(self.governor_address.into());
        tx.input = data.into();
        let result = self.provider.call(tx, None)
            .map_err(|e| format!("Failed to get proposal votes: {:?}", e))?;
        
        // Decode three uint256 values (againstVotes, forVotes, abstainVotes)
        if result.len() >= 96 {
            let mut against_bytes = [0u8; 32];
            let mut for_bytes = [0u8; 32];
            let mut abstain_bytes = [0u8; 32];
            
            against_bytes.copy_from_slice(&result[0..32]);
            for_bytes.copy_from_slice(&result[32..64]);
            abstain_bytes.copy_from_slice(&result[64..96]);
            
            let against = eth::U256::from_be_bytes(against_bytes).to_string();
            let for_votes = eth::U256::from_be_bytes(for_bytes).to_string();
            let abstain = eth::U256::from_be_bytes(abstain_bytes).to_string();
            
            Ok((against, for_votes, abstain))
        } else {
            Ok(("0".to_string(), "0".to_string(), "0".to_string()))
        }
    }
    
    pub async fn get_quorum(&self, block: u64) -> Result<String, String> {
        // Encode quorum(uint256) call with block number as parameter
        let mut data = vec![0xf8, 0xd3, 0xd4, 0x76]; // quorum selector
        
        // Add block number as uint256 parameter
        let mut block_bytes = [0u8; 32];
        block_bytes[24..].copy_from_slice(&block.to_be_bytes());
        data.extend_from_slice(&block_bytes);
        
        // Call the contract at the specific block
        let mut tx = eth::TransactionRequest::default();
        tx.to = Some(self.governor_address.into());
        tx.input = data.into();
        let result = self.provider.call(tx, Some(eth::BlockId::Number(eth::BlockNumberOrTag::Number(block))))
            .map_err(|e| format!("Failed to get quorum: {:?}", e))?;
        
        // Decode uint256 result
        if result.len() >= 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&result[..32]);
            let value = eth::U256::from_be_bytes(bytes);
            Ok(value.to_string())
        } else {
            // Default to 4% of total supply if call fails
            Ok("40000000000000000000000".to_string()) // 40,000 tokens (4% of 1M)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalEvent {
    Created {
        proposal_id: String,
        proposer: String,
        title: String,
        description: String,
        targets: Vec<String>,
        values: Vec<String>,
        calldatas: Vec<String>,
        start_block: u64,
        end_block: u64,
        block_number: u64,
        tx_hash: String,
    },
    VoteCast {
        proposal_id: String,
        voter: String,
        support: u8, // 0 = Against, 1 = For, 2 = Abstain
        weight: String,
        reason: String,
        block_number: u64,
        tx_hash: String,
    },
    Canceled {
        proposal_id: String,
        block_number: u64,
    },
    Queued {
        proposal_id: String,
        eta: u64,
        block_number: u64,
    },
    Executed {
        proposal_id: String,
        block_number: u64,
    },
}

pub fn process_proposal_event(
    event: ProposalEvent,
    proposals: &mut HashMap<String, OnchainProposal>,
) {
    match event {
        ProposalEvent::Created { 
            proposal_id,
            proposer,
            title,
            description,
            targets,
            values,
            calldatas,
            start_block,
            end_block,
            block_number,
            tx_hash,
        } => {
            proposals.insert(proposal_id.clone(), OnchainProposal {
                id: proposal_id,
                proposer,
                title,
                description,
                targets,
                values,
                calldatas,
                start_block,
                end_block,
                votes_for: "0".to_string(),
                votes_against: "0".to_string(),
                votes_abstain: "0".to_string(),
                status: ProposalStatus::Pending,
                tx_hash,
                block_number,
            });
        },
        ProposalEvent::VoteCast { 
            proposal_id,
            voter: _,
            support,
            weight,
            reason: _,
            block_number: _,
            tx_hash: _,
        } => {
            if let Some(proposal) = proposals.get_mut(&proposal_id) {
                let weight_val: u128 = weight.parse().unwrap_or(0);
                match support {
                    0 => {
                        let current: u128 = proposal.votes_against.parse().unwrap_or(0);
                        proposal.votes_against = (current + weight_val).to_string();
                    },
                    1 => {
                        let current: u128 = proposal.votes_for.parse().unwrap_or(0);
                        proposal.votes_for = (current + weight_val).to_string();
                    },
                    2 => {
                        let current: u128 = proposal.votes_abstain.parse().unwrap_or(0);
                        proposal.votes_abstain = (current + weight_val).to_string();
                    },
                    _ => {}
                }
            }
        },
        ProposalEvent::Canceled { proposal_id, block_number: _ } => {
            if let Some(proposal) = proposals.get_mut(&proposal_id) {
                proposal.status = ProposalStatus::Canceled;
            }
        },
        ProposalEvent::Queued { proposal_id, eta: _, block_number: _ } => {
            if let Some(proposal) = proposals.get_mut(&proposal_id) {
                proposal.status = ProposalStatus::Queued;
            }
        },
        ProposalEvent::Executed { proposal_id, block_number: _ } => {
            if let Some(proposal) = proposals.get_mut(&proposal_id) {
                proposal.status = ProposalStatus::Executed;
            }
        },
    }
}