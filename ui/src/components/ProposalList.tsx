import React from 'react';
import { useGovernanceStore } from '../store/governance';
import { ProposalStatus } from '../types/governance';
import { ProposalDiscussion } from './ProposalDiscussion';

interface ProposalListProps {
  onSelectProposal?: (proposalId: string) => void;
}

export const ProposalList: React.FC<ProposalListProps> = ({ onSelectProposal }) => {
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
          <ProposalCard 
            key={proposal.id} 
            proposal={proposal} 
            onSelect={onSelectProposal}
          />
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
          <ProposalCard 
            key={proposal.id} 
            proposal={proposal}
            onSelect={onSelectProposal}
          />
        ))}
        {pendingProposals.length === 0 && <p>No pending proposals</p>}
      </div>

      <div className="section">
        <h2>Completed ({completedProposals.length})</h2>
        {completedProposals.map(proposal => (
          <ProposalCard 
            key={proposal.id} 
            proposal={proposal}
            onSelect={onSelectProposal}
          />
        ))}
        {completedProposals.length === 0 && <p>No completed proposals</p>}
      </div>
    </div>
  );
};

interface ProposalCardProps {
  proposal: any;
  onSelect?: (proposalId: string) => void;
}

const ProposalCard: React.FC<ProposalCardProps> = ({ proposal, onSelect }) => {
  const { castVote } = useGovernanceStore();
  const [showDiscussion, setShowDiscussion] = React.useState(false);
  
  return (
    <div className="proposal-card">
      <div onClick={() => onSelect?.(proposal.id)} style={{ cursor: onSelect ? 'pointer' : 'default' }}>
        <h3>{proposal.title || `Proposal #${proposal.id}`}</h3>
        <p className="proposer">by {proposal.proposer}</p>
        <p className="description">{proposal.description?.slice(0, 200)}...</p>
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
              <button onClick={(e) => {
                e.stopPropagation();
                castVote(proposal.id, 'Yes' as any);
              }}>Vote Yes</button>
              <button onClick={(e) => {
                e.stopPropagation();
                castVote(proposal.id, 'No' as any);
              }}>Vote No</button>
              <button onClick={(e) => {
                e.stopPropagation();
                castVote(proposal.id, 'Abstain' as any);
              }}>Abstain</button>
            </div>
          )}
        </div>
      </div>
      <button 
        className="discussion-toggle"
        onClick={(e) => {
          e.stopPropagation();
          setShowDiscussion(!showDiscussion);
        }}
      >
        {showDiscussion ? 'Hide Discussion' : 'Show Discussion'}
      </button>
      {showDiscussion && (
        <div className="discussion-section">
        <ProposalDiscussion 
          proposalId={proposal.id} 
          proposalTitle={proposal.title}
        />
      </div>
    )}
  </div>
  );
};

const DraftCard: React.FC<{ draft: any }> = ({ draft }) => {
  const { editDraft, deleteDraft } = useGovernanceStore();
  // Get current node from window.our if available
  const currentNode = (window as any).our?.node || '';
  const isAuthor = draft.author === currentNode;
  const [isEditing, setIsEditing] = React.useState(false);
  const [editTitle, setEditTitle] = React.useState(draft.title);
  const [editDescription, setEditDescription] = React.useState(draft.description);
  const [showDiscussion, setShowDiscussion] = React.useState(false);
  
  const handleEdit = async () => {
    if (isEditing) {
      await editDraft(draft.id, editTitle, editDescription);
      setIsEditing(false);
    } else {
      setIsEditing(true);
    }
  };
  
  const handleDelete = async () => {
    if (confirm('Are you sure you want to delete this draft?')) {
      await deleteDraft(draft.id);
    }
  };
  
  const handleSubmit = () => {
    // TODO: Implement submit to chain functionality
    alert('Submit to chain functionality not yet implemented');
  };
  
  return (
    <div className="draft-card">
      {isEditing ? (
        <>
          <input 
            type="text" 
            value={editTitle} 
            onChange={(e) => setEditTitle(e.target.value)}
            placeholder="Title"
            style={{ width: '100%', marginBottom: '10px' }}
          />
          <textarea 
            value={editDescription} 
            onChange={(e) => setEditDescription(e.target.value)}
            placeholder="Description"
            style={{ width: '100%', minHeight: '100px', marginBottom: '10px' }}
          />
        </>
      ) : (
        <>
          <h3>{draft.title}</h3>
          <p className="description">{draft.description}</p>
        </>
      )}
      <p className="author">by {draft.author}</p>
      <div className="timestamps">
        <span>Created: {new Date(draft.created_at).toLocaleDateString()}</span>
        <span>Updated: {new Date(draft.updated_at).toLocaleDateString()}</span>
      </div>
      {isAuthor && (
        <div className="actions">
          <button onClick={handleEdit}>{isEditing ? 'Save' : 'Edit'}</button>
          {isEditing && <button onClick={() => setIsEditing(false)}>Cancel</button>}
          {!isEditing && <button onClick={handleSubmit}>Submit to Chain</button>}
          {!isEditing && <button onClick={handleDelete}>Delete</button>}
        </div>
      )}
      {!isAuthor && (
        <div className="info">
          <small>Only the author can edit or delete this draft</small>
        </div>
      )}
      <button 
        className="discussion-toggle"
        onClick={() => setShowDiscussion(!showDiscussion)}
      >
        {showDiscussion ? 'Hide Discussion' : 'Show Discussion'}
      </button>
      {showDiscussion && (
        <div className="discussion-section">
          <ProposalDiscussion 
            proposalId={draft.id} 
            proposalTitle={draft.title}
          />
        </div>
      )}
    </div>
  );
};