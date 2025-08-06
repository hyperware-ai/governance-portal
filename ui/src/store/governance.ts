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

// Helper to handle API responses that might be wrapped in Result or direct JSON
function parseApiResponse<T>(response: any): T {
  // If it's already parsed JSON, return it
  if (typeof response === 'object' && response !== null) {
    return response as T;
  }
  // If it's a string, parse it
  if (typeof response === 'string') {
    return JSON.parse(response) as T;
  }
  throw new Error('Invalid API response format');
}

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
      const response = await api.getProposals();
      const result = parseApiResponse<{ onchain: any[], drafts: any[] }>(response);
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
      const response = await api.createDraft(payload);
      const result = parseApiResponse<{ success: boolean, draft?: any, draft_id?: string }>(response);
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
      const response = await api.addDiscussion(payload);
      const result = parseApiResponse<{ success: boolean, discussion?: any }>(response);
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
      const response = await api.getCommitteeStatus();
      const status = parseApiResponse<CommitteeStatus>(response);
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
      const response = await api.getVotingPowerInfo();
      const power = parseApiResponse<VotingPowerInfo>(response);
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
      const response = await api.requestJoinCommittee(payload);
      // requestJoinCommittee returns a Result, check if it's successful
      if (typeof response === 'string' && response.includes('Successfully')) {
        set({ 
          isLoading: false,
          error: null
        });
        await get().fetchCommitteeStatus();
      } else {
        set({ 
          error: 'Failed to join committee',
          isLoading: false 
        });
      }
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