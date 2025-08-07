use hyperware_app_common::hyperware_process_lib::{vfs, our};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{OnchainProposal, ProposalDraft, Discussion, GovernanceCRDT};

const STORAGE_DRIVE: &str = "governance_data";

#[derive(Clone)]
pub struct FileStorage {
    drive_path: String,
}

impl FileStorage {
    pub fn new() -> Result<Self, String> {
        let our = our();
        let drive_path = vfs::create_drive(our.package_id(), STORAGE_DRIVE, None)
            .map_err(|e| format!("Failed to create storage drive: {:?}", e))?;
            
        Ok(Self { drive_path })
    }
    
    // Save onchain proposals to file
    pub fn save_proposals(&self, proposals: &HashMap<String, OnchainProposal>) -> Result<(), String> {
        let path = format!("{}/proposals.json", self.drive_path);
        let data = serde_json::to_vec(proposals)
            .map_err(|e| format!("Failed to serialize proposals: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open proposals file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write proposals: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn load_proposals(&self) -> Result<HashMap<String, OnchainProposal>, String> {
        let path = format!("{}/proposals.json", self.drive_path);
        
        match vfs::open_file(&path, false, None) {
            Ok(file) => {
                let data = file.read()
                    .map_err(|e| format!("Failed to read proposals: {:?}", e))?;
                    
                serde_json::from_slice(&data)
                    .map_err(|e| format!("Failed to deserialize proposals: {}", e))
            },
            Err(_) => Ok(HashMap::new()), // File doesn't exist yet
        }
    }
    
    // Save proposal drafts to file
    pub fn save_drafts(&self, drafts: &HashMap<String, ProposalDraft>) -> Result<(), String> {
        let path = format!("{}/drafts.json", self.drive_path);
        let data = serde_json::to_vec(drafts)
            .map_err(|e| format!("Failed to serialize drafts: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open drafts file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write drafts: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn load_drafts(&self) -> Result<HashMap<String, ProposalDraft>, String> {
        let path = format!("{}/drafts.json", self.drive_path);
        
        match vfs::open_file(&path, false, None) {
            Ok(file) => {
                let data = file.read()
                    .map_err(|e| format!("Failed to read drafts: {:?}", e))?;
                    
                serde_json::from_slice(&data)
                    .map_err(|e| format!("Failed to deserialize drafts: {}", e))
            },
            Err(_) => Ok(HashMap::new()),
        }
    }
    
    // Save discussions to file
    pub fn save_discussions(&self, discussions: &HashMap<String, Vec<Discussion>>) -> Result<(), String> {
        let path = format!("{}/discussions.json", self.drive_path);
        let data = serde_json::to_vec(discussions)
            .map_err(|e| format!("Failed to serialize discussions: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open discussions file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write discussions: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn load_discussions(&self) -> Result<HashMap<String, Vec<Discussion>>, String> {
        let path = format!("{}/discussions.json", self.drive_path);
        
        match vfs::open_file(&path, false, None) {
            Ok(file) => {
                let data = file.read()
                    .map_err(|e| format!("Failed to read discussions: {:?}", e))?;
                    
                serde_json::from_slice(&data)
                    .map_err(|e| format!("Failed to deserialize discussions: {}", e))
            },
            Err(_) => Ok(HashMap::new()),
        }
    }
    
    // Save CRDT state to file
    pub fn save_crdt_state(&self, crdt: &GovernanceCRDT) -> Result<(), String> {
        let path = format!("{}/crdt_state.bin", self.drive_path);
        let data = bincode::serialize(crdt)
            .map_err(|e| format!("Failed to serialize CRDT state: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open CRDT file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write CRDT state: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn load_crdt_state(&self) -> Result<GovernanceCRDT, String> {
        let path = format!("{}/crdt_state.bin", self.drive_path);
        
        match vfs::open_file(&path, false, None) {
            Ok(file) => {
                let data = file.read()
                    .map_err(|e| format!("Failed to read CRDT state: {:?}", e))?;
                    
                bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to deserialize CRDT state: {}", e))
            },
            Err(_) => Ok(GovernanceCRDT::default()),
        }
    }
    
    // Save indexing metadata
    pub fn save_metadata(&self, last_block: u64) -> Result<(), String> {
        let path = format!("{}/metadata.json", self.drive_path);
        let metadata = IndexingMetadata { last_indexed_block: last_block };
        let data = serde_json::to_vec(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open metadata file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write metadata: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn load_metadata(&self) -> Result<u64, String> {
        let path = format!("{}/metadata.json", self.drive_path);
        
        match vfs::open_file(&path, false, None) {
            Ok(file) => {
                let data = file.read()
                    .map_err(|e| format!("Failed to read metadata: {:?}", e))?;
                    
                let metadata: IndexingMetadata = serde_json::from_slice(&data)
                    .map_err(|e| format!("Failed to deserialize metadata: {}", e))?;
                    
                Ok(metadata.last_indexed_block)
            },
            Err(_) => Ok(0), // Start from block 0 if no metadata exists
        }
    }
    
    // Save chain event logs
    pub fn save_event_log(&self, block_number: u64, events: &[crate::chain_indexer::ProposalEvent]) -> Result<(), String> {
        let path = format!("{}/events/block_{}.json", self.drive_path, block_number);
        let data = serde_json::to_vec(events)
            .map_err(|e| format!("Failed to serialize events: {}", e))?;
        
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open events file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write events: {:?}", e))?;
            
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct IndexingMetadata {
    last_indexed_block: u64,
}