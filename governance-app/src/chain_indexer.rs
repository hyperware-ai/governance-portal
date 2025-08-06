use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{OnchainProposal, ProposalStatus};

// Simplified chain indexer that will be expanded later with actual Ethereum integration
#[derive(Clone)]
pub struct ChainIndexer {
    pub last_indexed_block: u64,
}

impl ChainIndexer {
    pub async fn new() -> Result<Self, String> {
        Ok(Self {
            last_indexed_block: 0,
        })
    }
    
    pub async fn index_proposals(
        &mut self, 
        from_block: u64, 
        to_block: Option<u64>
    ) -> Result<Vec<ProposalEvent>, String> {
        // For now, return empty events
        // This will be expanded with actual chain indexing
        let to = to_block.unwrap_or(from_block + 100);
        self.last_indexed_block = to;
        Ok(vec![])
    }
    
    pub async fn get_block_number(&self) -> Result<u64, String> {
        // Placeholder - return a simulated block number
        Ok(self.last_indexed_block + 1000)
    }
    
    pub async fn get_proposal_state(&self, _proposal_id: &str) -> Result<ProposalStatus, String> {
        Ok(ProposalStatus::Active)
    }
    
    pub async fn get_voting_power(&self, _address: &str, _block: u64) -> Result<String, String> {
        Ok("0".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalEvent {
    Created {
        proposal_id: String,
        proposer: String,
        targets: Vec<String>,
        values: Vec<String>,
        calldatas: Vec<String>,
        start_block: u64,
        end_block: u64,
        description: String,
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
    },
    Canceled {
        proposal_id: String,
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
            targets,
            values,
            calldatas,
            start_block,
            end_block,
            description,
            block_number,
            tx_hash,
        } => {
            let title = description.lines().next().unwrap_or("Untitled").to_string();
            
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
        ProposalEvent::Executed { proposal_id, block_number: _ } => {
            if let Some(proposal) = proposals.get_mut(&proposal_id) {
                proposal.status = ProposalStatus::Executed;
            }
        },
    }
}