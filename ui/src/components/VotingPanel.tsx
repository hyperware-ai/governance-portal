import React, { useState } from 'react';
import { useGovernanceStore } from '../store/governance';
import { VoteChoice } from '../types/governance';

interface VotingPanelProps {
  proposalId: string;
  walletAddress?: `0x${string}`;
  isConnected?: boolean;
}

export const VotingPanel: React.FC<VotingPanelProps> = ({ 
  proposalId, 
  walletAddress, 
  isConnected 
}) => {
  const { proposals, castVote } = useGovernanceStore();
  const [selectedChoice, setSelectedChoice] = useState<VoteChoice | null>(null);
  const [isVoting, setIsVoting] = useState(false);
  const [votingReason, setVotingReason] = useState('');
  
  const proposal = proposals.find(p => p.id === proposalId);
  
  if (!proposal) return null;
  
  const handleVote = async () => {
    if (!selectedChoice || !isConnected || !walletAddress) return;
    
    setIsVoting(true);
    try {
      await castVote(proposalId, selectedChoice, walletAddress);
      // Vote submitted to backend, which will submit to chain
      alert(`Vote cast: ${selectedChoice} for proposal ${proposalId}`);
      setSelectedChoice(null);
      setVotingReason('');
    } catch (error) {
      console.error('Failed to cast vote:', error);
      alert('Failed to cast vote. Please try again.');
    } finally {
      setIsVoting(false);
    }
  };
  
  const isVotingActive = proposal.status === 'Active';
  
  return (
    <div className="voting-panel">
      <h3>Cast Your Vote</h3>
      
      {!isConnected ? (
        <p className="text-muted">Connect your wallet to vote</p>
      ) : !isVotingActive ? (
        <p className="text-muted">Voting is not active for this proposal</p>
      ) : (
        <>
          <div className="vote-options">
            <button
              className={`vote-option for ${selectedChoice === VoteChoice.Yes ? 'selected' : ''}`}
              onClick={() => setSelectedChoice(VoteChoice.Yes)}
              disabled={isVoting}
            >
                  <span className="vote-icon">✓</span>
              <span>Vote For</span>
            </button>
            
            <button
              className={`vote-option against ${selectedChoice === VoteChoice.No ? 'selected' : ''}`}
              onClick={() => setSelectedChoice(VoteChoice.No)}
              disabled={isVoting}
            >
                  <span className="vote-icon">✗</span>
              <span>Vote Against</span>
            </button>
            
            <button
              className={`vote-option abstain ${selectedChoice === VoteChoice.Abstain ? 'selected' : ''}`}
              onClick={() => setSelectedChoice(VoteChoice.Abstain)}
              disabled={isVoting}
            >
                  <span className="vote-icon">−</span>
              <span>Abstain</span>
            </button>
          </div>
          
          <div className="vote-reason">
            <label>Reason (optional)</label>
            <textarea
              value={votingReason}
              onChange={(e) => setVotingReason(e.target.value)}
              placeholder="Explain your vote..."
              rows={3}
              disabled={isVoting}
            />
          </div>
          
          <button
            className="submit-vote primary"
            onClick={handleVote}
            disabled={!selectedChoice || isVoting}
          >
            {isVoting ? 'Submitting Vote...' : 'Submit Vote'}
          </button>
          
          {walletAddress && (
            <p className="voting-info">
              Voting as: {walletAddress.slice(0, 6)}...{walletAddress.slice(-4)}
            </p>
          )}
        </>
      )}
    </div>
  );
};