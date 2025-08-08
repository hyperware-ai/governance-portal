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
  editDraft: (id: string, title?: string, description?: string) => Promise<void>;
  deleteDraft: (id: string) => Promise<void>;
  addDiscussion: (proposalId: string, content: string, parentId?: string) => Promise<void>;
  fetchDiscussions: (proposalId: string) => Promise<void>;
  fetchCommitteeStatus: () => Promise<void>;
  fetchVotingPower: (address?: string) => Promise<void>;
  joinCommittee: (targetNodes: string[]) => Promise<void>;
  syncWithCommittee: () => Promise<void>;
  castVote: (proposalId: string, choice: VoteChoice, voter?: string) => Promise<void>;
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

  editDraft: async (id: string, title?: string, description?: string) => {
    set({ isLoading: true, error: null });
    try {
      const payload = JSON.stringify({ id, title, description });
      const response = await api.editDraft(payload);
      const result = parseApiResponse<{ success: boolean, draft?: any }>(response);
      if (result.success) {
        await get().fetchProposals();
      }
      set({ isLoading: false });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to edit draft',
        isLoading: false 
      });
    }
  },

  deleteDraft: async (id: string) => {
    set({ isLoading: true, error: null });
    try {
      const payload = JSON.stringify({ id });
      const response = await api.deleteDraft(payload);
      const result = parseApiResponse<{ success: boolean }>(response);
      if (result.success) {
        await get().fetchProposals();
      }
      set({ isLoading: false });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to delete draft',
        isLoading: false 
      });
    }
  },

  fetchDiscussions: async (proposalId: string) => {
    set({ isLoading: true, error: null });
    try {
      const payload = JSON.stringify({ proposal_id: proposalId });
      const response = await fetch('/api', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ GetDiscussions: payload })
      });
      const result = await response.json();
      if (result.Ok) {
        const data = JSON.parse(result.Ok);
        if (data.success) {
          set(state => {
            const discussions = new Map(state.discussions);
            discussions.set(proposalId, data.discussions || []);
            return { discussions, isLoading: false };
          });
        }
      }
      set({ isLoading: false });
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to fetch discussions',
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

  fetchVotingPower: async (address?: string) => {
    set({ isLoading: true, error: null });
    try {
      // Pass the wallet address if provided, otherwise empty object
      const request = address ? JSON.stringify({ address }) : '{}';
      const response = await api.getVotingPowerInfo(request);
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

  castVote: async (proposalId: string, choice: VoteChoice, voter?: string) => {
    set({ isLoading: true, error: null });
    try {
      const support = choice === VoteChoice.Yes ? 1 : choice === VoteChoice.No ? 0 : 2;
      const payload = JSON.stringify({
        proposal_id: proposalId,
        support,
        voter: voter || '0x0000000000000000000000000000000000000000',
        reason: ''
      });
      const response = await api.castVote(payload);
      const result = parseApiResponse<{ success: boolean, message?: string }>(response);
      
      if (result.success) {
        // Refresh proposals to get updated vote counts
        await get().fetchProposals();
        set({ isLoading: false });
      } else {
        set({ 
          error: result.message || 'Failed to cast vote',
          isLoading: false 
        });
      }
    } catch (error) {
      set({ 
        error: error instanceof Error ? error.message : 'Failed to cast vote',
        isLoading: false 
      });
    }
  },

  setError: (error: string | null) => set({ error }),
  clearError: () => set({ error: null }),
}));