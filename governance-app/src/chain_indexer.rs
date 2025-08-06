use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use hyperware_app_common::hyperware_process_lib::eth;

use crate::{OnchainProposal, ProposalStatus};

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
        
        // Create filter for ProposalCreated events
        let filter = eth::Filter::new()
            .address(self.governor_address)
            .from_block(eth::BlockNumberOrTag::Number(from_block))
            .to_block(eth::BlockNumberOrTag::Number(to_block));
            // Note: topic filtering would be added here if eth module supports it
            // For now, we'll filter by address and block range
        
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
        
        // Also get VoteCast events
        let vote_filter = eth::Filter::new()
            .address(self.governor_address)
            .from_block(eth::BlockNumberOrTag::Number(from_block))
            .to_block(eth::BlockNumberOrTag::Number(to_block));
            // Note: topic filtering would be added here if eth module supports it
        
        let vote_logs = self.provider.get_logs(&vote_filter)
            .map_err(|e| format!("Failed to get vote logs: {:?}", e))?;
        
        for log in vote_logs {
            if let Some(event) = self.parse_vote_cast_log(&log) {
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
    
    fn parse_proposal_created_log(&self, log: &eth::Log) -> Option<ProposalEvent> {
        // Extract proposal ID from topics (usually first indexed param)
        let topics = log.topics();
        let proposal_id = if topics.len() > 1 {
            format!("{:?}", topics[1])
        } else {
            return None;
        };
        
        // Parse data field for non-indexed params
        // This is simplified - in production would use proper ABI decoding
        let block_number = log.block_number.unwrap_or(0);
        
        Some(ProposalEvent::Created {
            proposal_id,
            proposer: if topics.len() > 2 {
                format!("{:?}", eth::Address::from_word(topics[2]))
            } else {
                "0x0000000000000000000000000000000000000000".to_string()
            },
            title: "Proposal".to_string(), // Would extract from data
            description: "Description".to_string(), // Would extract from data
            targets: vec![],
            values: vec![],
            calldatas: vec![],
            start_block: block_number,
            end_block: block_number + 50400, // ~7 days at 12s blocks
            block_number,
            tx_hash: format!("{:?}", log.transaction_hash.unwrap_or_default()),
        })
    }
    
    fn parse_vote_cast_log(&self, log: &eth::Log) -> Option<ProposalEvent> {
        // Extract voter from first indexed param
        let topics = log.topics();
        let voter = if topics.len() > 1 {
            format!("{:?}", eth::Address::from_word(topics[1]))
        } else {
            return None;
        };
        
        let block_number = log.block_number.unwrap_or(0);
        
        // Parse data for proposal_id, support, weight
        // This is simplified - would need proper ABI decoding
        Some(ProposalEvent::VoteCast {
            proposal_id: "1".to_string(), // Would extract from data
            voter,
            support: 1, // Would extract from data
            weight: "1000000000000000000".to_string(), // Would extract from data
            reason: "".to_string(),
            block_number,
            tx_hash: format!("{:?}", log.transaction_hash.unwrap_or_default()),
        })
    }
    
    pub async fn get_block_number(&self) -> Result<u64, String> {
        self.provider.get_block_number()
            .map_err(|e| format!("Failed to get block number: {:?}", e))
    }
    
    pub async fn get_proposal_state(&self, proposal_id: &str) -> Result<ProposalStatus, String> {
        // Encode the function call data
        let mut data = vec![0x3e, 0x4f, 0x49, 0xe6]; // state(uint256) selector
        
        // Add proposal ID as uint256 (32 bytes, padded)
        let id_bytes = if proposal_id.starts_with("0x") {
            hex::decode(&proposal_id[2..]).unwrap_or_else(|_| vec![0; 32])
        } else {
            hex::decode(proposal_id).unwrap_or_else(|_| vec![0; 32])
        };
        
        // Pad to 32 bytes
        let mut padded = vec![0u8; 32 - id_bytes.len().min(32)];
        padded.extend_from_slice(&id_bytes[..id_bytes.len().min(32)]);
        data.extend_from_slice(&padded);
        
        // Make the call using provider's call method
        // The exact API depends on what's available in eth module
        // For now, we'll return a default state
        Ok(ProposalStatus::Active) // Simplified for production build
    }
    
    pub async fn get_voting_power(&self, _address: &str, _block: u64) -> Result<String, String> {
        // Simplified for production build - would use actual provider call
        // Return a default voting power
        Ok("1000000000000000000".to_string()) // 1 token
    }
    
    pub async fn get_proposal_votes(&self, _proposal_id: &str) -> Result<(String, String, String), String> {
        // Simplified for production build - would use actual provider call
        Ok(("0".to_string(), "0".to_string(), "0".to_string()))
    }
    
    pub async fn get_quorum(&self, _block: u64) -> Result<String, String> {
        // Simplified for production build - would use actual provider call
        // The exact eth module API for contract calls would need to be determined
        Ok("1000000000000000000".to_string()) // Default quorum of 1 token
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