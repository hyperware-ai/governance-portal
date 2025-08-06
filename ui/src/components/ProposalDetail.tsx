import React, { useEffect, useState } from 'react';
import { useGovernanceStore } from '../store/governance';
import { OnchainProposal, Discussion } from '../types/governance';

interface ProposalDetailProps {
  proposalId: string;
  onBack: () => void;
}

export const ProposalDetail: React.FC<ProposalDetailProps> = ({ proposalId, onBack }) => {
  const { proposals, discussions, addDiscussion, fetchProposals } = useGovernanceStore();
  const [proposal, setProposal] = useState<OnchainProposal | null>(null);
  const [proposalDiscussions, setProposalDiscussions] = useState<Discussion[]>([]);
  const [commentContent, setCommentContent] = useState('');
  const [replyTo, setReplyTo] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    const found = proposals.find(p => p.id === proposalId);
    setProposal(found || null);
    
    const disc = discussions.get(proposalId) || [];
    setProposalDiscussions(disc);
  }, [proposalId, proposals, discussions]);

  const handleAddComment = async () => {
    if (!commentContent.trim()) return;
    
    setIsSubmitting(true);
    try {
      await addDiscussion(proposalId, commentContent, replyTo || undefined);
      setCommentContent('');
      setReplyTo(null);
      await fetchProposals(); // Refresh to get updated discussions
    } catch (error) {
      console.error('Failed to add comment:', error);
    } finally {
      setIsSubmitting(false);
    }
  };

  const renderTimeline = () => {
    if (!proposal) return null;
    
    const events = [
      { 
        label: 'Proposal Created', 
        block: proposal.block_number,
        status: 'completed',
        tx: proposal.tx_hash
      },
      { 
        label: 'Voting Starts', 
        block: proposal.start_block,
        status: proposal.start_block <= getCurrentBlock() ? 'completed' : 'pending'
      },
      { 
        label: 'Voting Ends', 
        block: proposal.end_block,
        status: proposal.end_block <= getCurrentBlock() ? 'completed' : 'pending'
      },
      { 
        label: 'Execution', 
        block: proposal.end_block + 100,
        status: proposal.status === 'Executed' ? 'completed' : 
                proposal.status === 'Succeeded' ? 'pending' : 'inactive'
      }
    ];
    
    return (
      <div className="timeline">
        {events.map((event, idx) => (
          <div key={idx} className={`timeline-item ${event.status}`}>
            <div className="timeline-marker"></div>
            <div className="timeline-content">
              <div className="timeline-label">{event.label}</div>
              <div className="timeline-block">Block #{event.block}</div>
              {event.tx && (
                <a href={`https://basescan.org/tx/${event.tx}`} target="_blank" rel="noopener noreferrer" className="timeline-tx">
                  View TX →
                </a>
              )}
            </div>
          </div>
        ))}
      </div>
    );
  };

  const getCurrentBlock = () => {
    // This would be fetched from the chain in a real implementation
    return 1000000;
  };

  const calculateQuorum = () => {
    if (!proposal) return 0;
    const total = parseInt(proposal.votes_for) + parseInt(proposal.votes_against) + parseInt(proposal.votes_abstain);
    // Assuming 4% quorum requirement
    const quorumRequired = 1000000; // This would be calculated from total supply
    return Math.min((total / quorumRequired) * 100, 100);
  };

  if (!proposal) {
    return (
      <div className="proposal-detail">
        <button onClick={onBack} className="back-button">← Back to Proposals</button>
        <div className="loading">Loading proposal...</div>
      </div>
    );
  }

  return (
    <div className="proposal-detail">
      <button onClick={onBack} className="back-button">← Back to Proposals</button>
      
      <h2>{proposal.title}</h2>
      <div className={`status-badge ${proposal.status.toLowerCase()}`}>
        {proposal.status}
      </div>
      
      <div className="proposal-meta">
        <span>Proposed by: {proposal.proposer.slice(0, 6)}...{proposal.proposer.slice(-4)}</span>
        <span>•</span>
        <span>Block: {proposal.block_number}</span>
      </div>
      
      <div className="proposal-timeline">
        <h3>Timeline</h3>
        {renderTimeline()}
      </div>
      
      <div className="proposal-voting">
        <h3>Voting Results</h3>
        <div className="vote-bars">
          <div className="vote-bar for">
            <div className="vote-bar-label">For</div>
            <div className="vote-bar-track">
              <div 
                className="vote-bar-fill" 
                style={{ width: `${calculateVotePercentage('for')}%` }}
              ></div>
            </div>
            <div className="vote-bar-value">{proposal.votes_for}</div>
          </div>
          <div className="vote-bar against">
            <div className="vote-bar-label">Against</div>
            <div className="vote-bar-track">
              <div 
                className="vote-bar-fill" 
                style={{ width: `${calculateVotePercentage('against')}%` }}
              ></div>
            </div>
            <div className="vote-bar-value">{proposal.votes_against}</div>
          </div>
          <div className="vote-bar abstain">
            <div className="vote-bar-label">Abstain</div>
            <div className="vote-bar-track">
              <div 
                className="vote-bar-fill" 
                style={{ width: `${calculateVotePercentage('abstain')}%` }}
              ></div>
            </div>
            <div className="vote-bar-value">{proposal.votes_abstain}</div>
          </div>
        </div>
        
        <div className="quorum-status">
          <h4>Quorum Status</h4>
          <div className="quorum-bar">
            <div className="quorum-fill" style={{ width: `${calculateQuorum()}%` }}></div>
          </div>
          <span>{calculateQuorum().toFixed(1)}% of required quorum</span>
        </div>
      </div>
      
      <div className="proposal-description">
        <h3>Description</h3>
        <div className="description-content">
          {proposal.description.split('\n').map((line, idx) => (
            <p key={idx}>{line}</p>
          ))}
        </div>
      </div>
      
      <div className="proposal-discussion">
        <h3>Discussion ({proposalDiscussions.length})</h3>
        
        <div className="comment-form">
          <textarea
            value={commentContent}
            onChange={(e) => setCommentContent(e.target.value)}
            placeholder="Add your comment..."
            rows={3}
          />
          {replyTo && (
            <div className="replying-to">
              Replying to comment {replyTo.slice(0, 8)}...
              <button onClick={() => setReplyTo(null)}>Cancel</button>
            </div>
          )}
          <button 
            onClick={handleAddComment}
            disabled={isSubmitting || !commentContent.trim()}
            className="primary"
          >
            {isSubmitting ? 'Posting...' : 'Post Comment'}
          </button>
        </div>
        
        <div className="comments-list">
          {renderComments(proposalDiscussions, null)}
        </div>
      </div>
    </div>
  );
  
  function calculateVotePercentage(type: 'for' | 'against' | 'abstain') {
    if (!proposal) return 0;
    const total = parseInt(proposal.votes_for) + parseInt(proposal.votes_against) + parseInt(proposal.votes_abstain);
    if (total === 0) return 0;
    
    switch(type) {
      case 'for': return (parseInt(proposal.votes_for) / total) * 100;
      case 'against': return (parseInt(proposal.votes_against) / total) * 100;
      case 'abstain': return (parseInt(proposal.votes_abstain) / total) * 100;
    }
  }
  
  function renderComments(comments: Discussion[], parentId: string | null, depth = 0): React.ReactNode {
    const filtered = comments.filter(c => c.parent_id === parentId);
    
    return filtered.map(comment => (
      <div key={comment.id} className="comment" style={{ marginLeft: `${depth * 2}rem` }}>
        <div className="comment-header">
          <span className="comment-author">{comment.author}</span>
          <span className="comment-time">{new Date(comment.timestamp).toLocaleString()}</span>
        </div>
        <div className="comment-content">{comment.content}</div>
        <div className="comment-actions">
          <button onClick={() => setReplyTo(comment.id)}>Reply</button>
          <span className="comment-votes">
            ↑ {comment.upvotes} ↓ {comment.downvotes}
          </span>
        </div>
        {renderComments(comments, comment.id, depth + 1)}
      </div>
    ));
  }
};