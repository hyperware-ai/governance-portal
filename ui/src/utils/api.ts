import { BASE_URL } from '../types/global';

export async function makeApiCall<TResponse>(
  call: Record<string, any>
): Promise<TResponse> {
  try {
    // For Hyperware apps, use a relative path 'api' (without leading slash)
    // This ensures the API call is relative to the current app's base path
    const apiUrl = BASE_URL || 'api';
    const response = await fetch(apiUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(call),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`API call failed: ${response.status} - ${errorText}`);
    }

    const result = await response.json();
    return result;
  } catch (error) {
    console.error('API call error:', error);
    throw error;
  }
}

export class GovernanceAPI {
  static async ready(): Promise<{ ready: boolean }> {
    return makeApiCall({ Ready: null });
  }

  static async getProposals(): Promise<{
    onchain: any[];
    drafts: any[];
  }> {
    return makeApiCall({ GetProposals: null });
  }

  static async createDraft(title: string, description: string): Promise<{
    success: boolean;
    draft_id: string;
    draft: any;
  }> {
    return makeApiCall({ CreateDraft: { title, description } });
  }

  static async addDiscussion(
    proposal_id: string,
    content: string,
    parent_id?: string
  ): Promise<{
    success: boolean;
    discussion: any;
  }> {
    return makeApiCall({ 
      AddDiscussion: {
        proposal_id,
        content,
        parent_id: parent_id || null,
      }
    });
  }

  static async getCommitteeStatus(): Promise<{
    members: string[];
    is_member: boolean;
    online_count: number;
  }> {
    return makeApiCall({ GetCommitteeStatus: null });
  }

  static async getVotingPowerInfo(): Promise<{
    voting_power: string;
    delegated_power: string;
    total_supply: string;
  }> {
    return makeApiCall({ GetVotingPowerInfo: null });
  }

  static async requestJoinCommittee(targetNodes: string[]): Promise<string> {
    return makeApiCall({ RequestJoinCommittee: targetNodes });
  }
}

export function isApiError(error: unknown): error is Error {
  return error instanceof Error;
}

export function getErrorMessage(error: unknown): string {
  if (isApiError(error)) {
    return error.message;
  }
  return 'An unknown error occurred';
}