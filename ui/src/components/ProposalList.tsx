import React from 'react';
import { useGovernanceStore } from '../store/governance';
import { ProposalStatus } from '../types/governance';

export const ProposalList: React.FC = () => {
  const { proposals, drafts, isLoading, error, fetchProposals } = useGovernanceStore();

  React.useEffect(() => {
    fetchProposals();
  }, []);

  if (isLoading) return <div className="loading">Loading proposals...</div>;
  if (error) return <div className="error">Error: {error}</div>;

  const activeProposals = proposals.filter(p => p.status === ProposalStatus.Active);
  const pendingProposals = proposals.filter(p => p.status === ProposalStatus.Pending);
  const completedProposals = proposals.filter(p => 
    [ProposalStatus.Executed, ProposalStatus.Defeated, ProposalStatus.Expired].includes(p.status)
  );

  return (
    <div className="proposal-list">
      <div className="section">
        <h2>Active Proposals ({activeProposals.length})</h2>
        {activeProposals.map(proposal => (
          <ProposalCard key={proposal.id} proposal={proposal} />
        ))}
        {activeProposals.length === 0 && <p>No active proposals</p>}
      </div>

      <div className="section">
        <h2>Drafts ({drafts.length})</h2>
        {drafts.map(draft => (
          <DraftCard key={draft.id} draft={draft} />
        ))}
        {drafts.length === 0 && <p>No drafts</p>}
      </div>

      <div className="section">
        <h2>Pending ({pendingProposals.length})</h2>
        {pendingProposals.map(proposal => (
          <ProposalCard key={proposal.id} proposal={proposal} />
        ))}
        {pendingProposals.length === 0 && <p>No pending proposals</p>}
      </div>

      <div className="section">
        <h2>Completed ({completedProposals.length})</h2>
        {completedProposals.map(proposal => (
          <ProposalCard key={proposal.id} proposal={proposal} />
        ))}
        {completedProposals.length === 0 && <p>No completed proposals</p>}
      </div>
    </div>
  );
};

const ProposalCard: React.FC<{ proposal: any }> = ({ proposal }) => {
  const { castVote } = useGovernanceStore();
  
  return (
    <div className="proposal-card">
      <h3>{proposal.title || `Proposal #${proposal.id}`}</h3>
      <p className="proposer">by {proposal.proposer}</p>
      <p className="description">{proposal.description}</p>
      <div className="status">
        <span className={`status-badge ${proposal.status.toLowerCase()}`}>
          {proposal.status}
        </span>
      </div>
      <div className="voting">
        <div className="vote-counts">
          <span>For: {proposal.votes_for}</span>
          <span>Against: {proposal.votes_against}</span>
          <span>Abstain: {proposal.votes_abstain}</span>
        </div>
        {proposal.status === ProposalStatus.Active && (
          <div className="vote-buttons">
            <button onClick={() => castVote(proposal.id, 'Yes' as any)}>Vote Yes</button>
            <button onClick={() => castVote(proposal.id, 'No' as any)}>Vote No</button>
            <button onClick={() => castVote(proposal.id, 'Abstain' as any)}>Abstain</button>
          </div>
        )}
      </div>
    </div>
  );
};

const DraftCard: React.FC<{ draft: any }> = ({ draft }) => {
  return (
    <div className="draft-card">
      <h3>{draft.title}</h3>
      <p className="author">by {draft.author}</p>
      <p className="description">{draft.description}</p>
      <div className="timestamps">
        <span>Created: {new Date(draft.created_at).toLocaleDateString()}</span>
        <span>Updated: {new Date(draft.updated_at).toLocaleDateString()}</span>
      </div>
      <div className="actions">
        <button>Edit</button>
        <button>Submit to Chain</button>
        <button>Delete</button>
      </div>
    </div>
  );
};