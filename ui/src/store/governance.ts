import { create } from 'zustand';
import * as api from '../types/api';
import { 
  OnchainProposal, 
  ProposalDraft, 
  Discussion, 
  CommitteeStatus, 
  VotingPowerInfo,
  VoteChoice 
} from '../types/governance';

interface GovernanceStore {
  proposals: OnchainProposal[];
  drafts: ProposalDraft[];
  discussions: Map<string, Discussion[]>;
  committeeStatus: CommitteeStatus | null;
  votingPower: VotingPowerInfo | null;
  isLoading: boolean;
  isSyncing: boolean;
  error: string | null;
  
  fetchProposals: () => Promise<void>;
  createDraft: (title: string, description: string) => Promise<void>;
  addDiscussion: (proposalId: string, content: string, parentId?: string) => Promise<void>;
  fetchCommitteeStatus: () => Promise<void>;
  fetchVotingPower: () => Promise<void>;
  joinCommittee: (targetNodes: string[]) => Promise<void>;
  syncWithCommittee: () => Promise<void>;
  castVote: (proposalId: string, choice: VoteChoice) => Promise<void>;
  setError: (error: string | null) => void;
  clearError: () => void;
}

export const useGovernanceStore = create<GovernanceStore>((set, get) => ({
  proposals: [],
  drafts: [],
  discussions: new Map(),
  committeeStatus: null,
  votingPower: null,
  isLoading: false,
  isSyncing: false,
  error: null,

  fetchProposals: async () => {
    set({ isLoading: true, error: null });
    try {
      const resultStr = await api.getProposals();
      const result = JSON.parse(resultStr);
      set({ 
        proposals: result.onchain || [],
        drafts: result.drafts || [],
        isLoading: false 
      });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to fetch proposals',
        isLoading: false 
      });
    }
  },

  createDraft: async (title: string, description: string) => {
    set({ isLoading: true, error: null });
    try {
      const payload = JSON.stringify({ title, description });
      const resultStr = await api.createDraft(payload);
      const result = JSON.parse(resultStr);
      if (result.success) {
        set(state => ({
          drafts: [...state.drafts, result.draft],
          isLoading: false
        }));
      } else {
        set({ 
          error: 'Failed to create draft',
          isLoading: false 
        });
      }
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to create draft',
        isLoading: false 
      });
    }
  },

  addDiscussion: async (proposalId: string, content: string, parentId?: string) => {
    set({ isLoading: true, error: null });
    try {
      const payload = JSON.stringify({
        proposal_id: proposalId,
        content,
        parent_id: parentId || null
      });
      const resultStr = await api.addDiscussion(payload);
      const result = JSON.parse(resultStr);
      if (result.success) {
        set(state => {
          const discussions = new Map(state.discussions);
          const proposalDiscussions = discussions.get(proposalId) || [];
          proposalDiscussions.push(result.discussion);
          discussions.set(proposalId, proposalDiscussions);
          return {
            discussions,
            isLoading: false
          };
        });
      } else {
        set({ 
          error: 'Failed to add discussion',
          isLoading: false 
        });
      }
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to add discussion',
        isLoading: false 
      });
    }
  },

  fetchCommitteeStatus: async () => {
    set({ isLoading: true, error: null });
    try {
      const statusStr = await api.getCommitteeStatus();
      const status = JSON.parse(statusStr);
      set({ 
        committeeStatus: status,
        isLoading: false 
      });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to fetch committee status',
        isLoading: false 
      });
    }
  },

  fetchVotingPower: async () => {
    set({ isLoading: true, error: null });
    try {
      const powerStr = await api.getVotingPowerInfo();
      const power = JSON.parse(powerStr);
      set({ 
        votingPower: power,
        isLoading: false 
      });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to fetch voting power',
        isLoading: false 
      });
    }
  },

  joinCommittee: async (targetNodes: string[]) => {
    set({ isLoading: true, error: null });
    try {
      const payload = JSON.stringify(targetNodes);
      await api.requestJoinCommittee(payload);
      set({ 
        isLoading: false,
        error: null
      });
      await get().fetchCommitteeStatus();
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to join committee',
        isLoading: false 
      });
    }
  },

  syncWithCommittee: async () => {
    set({ isSyncing: true, error: null });
    try {
      await get().fetchProposals();
      await get().fetchCommitteeStatus();
      set({ isSyncing: false });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to sync',
        isSyncing: false 
      });
    }
  },

  castVote: async (proposalId: string, choice: VoteChoice) => {
    console.log('Voting on proposal', proposalId, 'with choice', choice);
    set({ error: 'Voting not yet implemented' });
  },

  setError: (error: string | null) => set({ error }),
  clearError: () => set({ error: null }),
}));