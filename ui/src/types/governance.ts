export interface OnchainProposal {
  id: string;
  proposer: string;
  title: string;
  description: string;
  targets: string[];
  values: string[];
  calldatas: string[];
  start_block: number;
  end_block: number;
  votes_for: string;
  votes_against: string;
  votes_abstain: string;
  status: ProposalStatus;
  tx_hash: string;
  block_number: number;
}

export enum ProposalStatus {
  Pending = "Pending",
  Active = "Active",
  Canceled = "Canceled",
  Defeated = "Defeated",
  Succeeded = "Succeeded",
  Queued = "Queued",
  Expired = "Expired",
  Executed = "Executed",
  Rejected = "Rejected"
}

export interface ProposalDraft {
  id: string;
  author: string;
  title: string;
  description: string;
  created_at: string;
  updated_at: string;
  signatures: NodeSignature[];
}

export interface NodeSignature {
  node_id: string;
  signature: number[];
  timestamp: number;
}

export interface Discussion {
  id: string;
  proposal_id: string;
  parent_id: string | null;
  author: string;
  content: string;
  timestamp: string;
  upvotes: number;
  downvotes: number;
  signatures: NodeSignature[];
}

export interface CommitteeStatus {
  members: string[];
  is_member: boolean;
  online_count: number;
}

export interface VotingPowerInfo {
  voting_power: string;
  delegated_power: string;
  total_supply: string;
}

export interface CreateDraftRequest {
  title: string;
  description: string;
}

export interface AddDiscussionRequest {
  proposal_id: string;
  content: string;
  parent_id?: string;
}

export enum VoteChoice {
  Yes = "Yes",
  No = "No",
  Abstain = "Abstain"
}

export interface GovernanceResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}