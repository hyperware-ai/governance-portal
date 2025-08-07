use hyperware_app_common::hyperware_process_lib::{vfs, our};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use sha3::{Digest, Sha3_256};
use chrono::Utc;

use crate::{OnchainProposal, ProposalDraft, Discussion, GovernanceCRDT};
use crate::p2p_committee::{CommitteeState, CommitteeCRDT, CRDTSnapshot};

const STORAGE_DRIVE: &str = "governance_data";
const SNAPSHOTS_DIR: &str = "snapshots";
const CRDT_CHECKPOINTS_DIR: &str = "crdt_checkpoints";

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
    
    // Save CRDT state to file (accepts both GovernanceCRDT and CommitteeCRDT)
    pub fn save_crdt_state<T: serde::Serialize>(&self, crdt: &T) -> Result<(), String> {
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
    
    // ===== CRDT Snapshot Management =====
    
    pub fn save_crdt_snapshot(&self, crdt: &CommitteeCRDT) -> Result<String, String> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let snapshot_id = format!("snapshot_{}", timestamp);
        let path = format!("{}/{}/{}.bin", self.drive_path, SNAPSHOTS_DIR, snapshot_id);
        
        // Create a snapshot from the CRDT
        let snapshot = crdt.create_snapshot();
        
        let data = bincode::serialize(&snapshot)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;
        
        // Calculate checksum
        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let checksum = hex::encode(hasher.finalize());
        
        // Save snapshot data
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to create snapshot file: {:?}", e))?;
        file.write(&data)
            .map_err(|e| format!("Failed to write snapshot: {:?}", e))?;
        
        // Save checksum
        let checksum_path = format!("{}/{}/{}.checksum", self.drive_path, SNAPSHOTS_DIR, snapshot_id);
        let checksum_file = vfs::open_file(&checksum_path, true, None)
            .map_err(|e| format!("Failed to create checksum file: {:?}", e))?;
        checksum_file.write(checksum.as_bytes())
            .map_err(|e| format!("Failed to write checksum: {:?}", e))?;
        
        Ok(snapshot_id)
    }
    
    pub fn load_crdt_snapshot(&self, snapshot_id: &str) -> Result<CommitteeCRDT, String> {
        let path = format!("{}/{}/{}.bin", self.drive_path, SNAPSHOTS_DIR, snapshot_id);
        let checksum_path = format!("{}/{}/{}.checksum", self.drive_path, SNAPSHOTS_DIR, snapshot_id);
        
        // Read snapshot data
        let file = vfs::open_file(&path, false, None)
            .map_err(|e| format!("Failed to open snapshot file: {:?}", e))?;
        let data = file.read()
            .map_err(|e| format!("Failed to read snapshot: {:?}", e))?;
        
        // Read and verify checksum
        let checksum_file = vfs::open_file(&checksum_path, false, None)
            .map_err(|e| format!("Failed to open checksum file: {:?}", e))?;
        let stored_checksum = checksum_file.read()
            .map_err(|e| format!("Failed to read checksum: {:?}", e))?;
        let stored_checksum = String::from_utf8(stored_checksum)
            .map_err(|e| format!("Invalid checksum format: {}", e))?;
        
        // Calculate actual checksum
        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let actual_checksum = hex::encode(hasher.finalize());
        
        if actual_checksum != stored_checksum {
            return Err("Snapshot checksum verification failed".to_string());
        }
        
        // Deserialize snapshot
        let snapshot: CRDTSnapshot = bincode::deserialize(&data)
            .map_err(|e| format!("Failed to deserialize snapshot: {}", e))?;
        
        // Create a new CRDT and restore from snapshot
        let mut crdt = CommitteeCRDT::new(String::new());
        crdt.restore_from_snapshot(snapshot);
        Ok(crdt)
    }
    
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>, String> {
        let snapshots_path = format!("{}/{}", self.drive_path, SNAPSHOTS_DIR);
        
        // List all files in snapshots directory
        match vfs::metadata(&snapshots_path, None) {
            Ok(_metadata) => {
                // Parse directory listing to extract snapshot info
                // This is a simplified version - actual implementation would parse the directory properly
                Ok(Vec::new())
            },
            Err(_) => Ok(Vec::new()), // Directory doesn't exist yet
        }
    }
    
    pub fn save_state(&self, state: &crate::GovernanceState) -> Result<(), String> {
        let path = format!("{}/state.bin", self.drive_path);
        let data = bincode::serialize(state)
            .map_err(|e| format!("Failed to serialize state: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open state file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write state: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn load_state(&self) -> Result<crate::GovernanceState, String> {
        let path = format!("{}/state.bin", self.drive_path);
        
        match vfs::open_file(&path, false, None) {
            Ok(file) => {
                let data = file.read()
                    .map_err(|e| format!("Failed to read state: {:?}", e))?;
                    
                bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to deserialize state: {}", e))
            },
            Err(e) => Err(format!("Failed to open state file: {:?}", e)),
        }
    }
    
    pub fn save_committee_members(&self, members: &HashSet<String>) -> Result<(), String> {
        let path = format!("{}/committee_members.json", self.drive_path);
        let data = serde_json::to_vec(members)
            .map_err(|e| format!("Failed to serialize committee members: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open committee members file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write committee members: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn save_committee_state(&self, state: &crate::p2p_committee::CommitteeState) -> Result<(), String> {
        let path = format!("{}/committee_state.bin", self.drive_path);
        let data = bincode::serialize(state)
            .map_err(|e| format!("Failed to serialize committee state: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open committee state file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write committee state: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn save_proposals(&self, proposals: &HashMap<String, crate::OnchainProposal>) -> Result<(), String> {
        let path = format!("{}/proposals.json", self.drive_path);
        let data = serde_json::to_vec(proposals)
            .map_err(|e| format!("Failed to serialize proposals: {}", e))?;
            
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to open proposals file: {:?}", e))?;
            
        file.write(&data)
            .map_err(|e| format!("Failed to write proposals: {:?}", e))?;
            
        Ok(())
    }
    
    pub fn cleanup_old_snapshots(&self, keep_count: usize) -> Result<(), String> {
        let snapshots = self.list_snapshots()?;
        
        if snapshots.len() <= keep_count {
            return Ok(());
        }
        
        // Sort by timestamp and remove oldest
        let mut sorted = snapshots;
        sorted.sort_by_key(|s| s.timestamp);
        
        for snapshot in sorted.iter().take(sorted.len() - keep_count) {
            let path = format!("{}/{}/{}.bin", self.drive_path, SNAPSHOTS_DIR, snapshot.id);
            let checksum_path = format!("{}/{}/{}.checksum", self.drive_path, SNAPSHOTS_DIR, snapshot.id);
            
            // Remove snapshot files
            let _ = vfs::remove_file(&path, None);
            let _ = vfs::remove_file(&checksum_path, None);
        }
        
        Ok(())
    }
    
    // ===== CRDT Checkpoints for Efficient Sync =====
    
    pub fn save_crdt_checkpoint(&self, crdt: &CommitteeCRDT, checkpoint_id: u64) -> Result<(), String> {
        let path = format!("{}/{}/checkpoint_{}.bin", self.drive_path, CRDT_CHECKPOINTS_DIR, checkpoint_id);
        
        let checkpoint = CRDTCheckpoint {
            id: checkpoint_id,
            crdt: crdt.clone(),
            timestamp: Utc::now().timestamp() as u64,
        };
        
        let data = bincode::serialize(&checkpoint)
            .map_err(|e| format!("Failed to serialize checkpoint: {}", e))?;
        
        let file = vfs::open_file(&path, true, None)
            .map_err(|e| format!("Failed to create checkpoint file: {:?}", e))?;
        file.write(&data)
            .map_err(|e| format!("Failed to write checkpoint: {:?}", e))?;
        
        Ok(())
    }
    
    pub fn load_crdt_checkpoint(&self, checkpoint_id: u64) -> Result<CommitteeCRDT, String> {
        let path = format!("{}/{}/checkpoint_{}.bin", self.drive_path, CRDT_CHECKPOINTS_DIR, checkpoint_id);
        
        let file = vfs::open_file(&path, false, None)
            .map_err(|e| format!("Failed to open checkpoint file: {:?}", e))?;
        let data = file.read()
            .map_err(|e| format!("Failed to read checkpoint: {:?}", e))?;
        
        let checkpoint: CRDTCheckpoint = bincode::deserialize(&data)
            .map_err(|e| format!("Failed to deserialize checkpoint: {}", e))?;
        
        Ok(checkpoint.crdt)
    }
}

#[derive(Serialize, Deserialize)]
struct IndexingMetadata {
    last_indexed_block: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct CRDTCheckpoint {
    id: u64,
    crdt: CommitteeCRDT,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SnapshotInfo {
    pub id: String,
    pub timestamp: u64,
    pub merkle_root: String,
}