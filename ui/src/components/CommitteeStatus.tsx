import React, { useEffect } from 'react';
import { useGovernanceStore } from '../store/governance';

export const CommitteeStatus: React.FC = () => {
  const { 
    committeeStatus, 
    fetchCommitteeStatus, 
    joinCommittee,
    isLoading 
  } = useGovernanceStore();

  useEffect(() => {
    fetchCommitteeStatus();
    const interval = setInterval(fetchCommitteeStatus, 30000);
    return () => clearInterval(interval);
  }, []);

  const handleJoinCommittee = async () => {
    const targetNode = prompt('Enter target node address to join committee:');
    if (targetNode) {
      await joinCommittee([targetNode]);
      await fetchCommitteeStatus();
    }
  };

  if (isLoading && !committeeStatus) {
    return <div className="loading">Loading committee status...</div>;
  }

  return (
    <div className="committee-status">
      <h3>Committee Status</h3>
      {committeeStatus ? (
        <div className="status-info">
          <div className="status-item">
            <span className="label">Committee Members:</span>
            <span className="value">{committeeStatus.committee_count || committeeStatus.members.length}</span>
          </div>
          <div className="status-item">
            <span className="label">Subscribers:</span>
            <span className="value">{committeeStatus.subscriber_count || 0}</span>
          </div>
          <div className="status-item">
            <span className="label">Online Members:</span>
            <span className="value">{committeeStatus.online_count}</span>
          </div>
          <div className="status-item">
            <span className="label">Total Participants:</span>
            <span className="value">{committeeStatus.total_participants || (committeeStatus.members.length + (committeeStatus.subscriber_count || 0))}</span>
          </div>
          <div className="status-item">
            <span className="label">Your Status:</span>
            <span className={`value ${committeeStatus.is_member ? 'member' : 'non-member'}`}>
              {committeeStatus.is_member ? 'Committee Member' : 'Not a Member'}
            </span>
          </div>
          
          {!committeeStatus.is_member && (
            <button 
              onClick={handleJoinCommittee}
              className="join-button"
              disabled={isLoading}
            >
              Join Committee
            </button>
          )}

          {committeeStatus.members.length > 0 && (
            <div className="members-list">
              <h4>Committee Members:</h4>
              <ul>
                {committeeStatus.members.slice(0, 5).map((member, idx) => (
                  <li key={idx}>{member}</li>
                ))}
                {committeeStatus.members.length > 5 && (
                  <li>... and {committeeStatus.members.length - 5} more</li>
                )}
              </ul>
            </div>
          )}
        </div>
      ) : (
        <p>No committee data available</p>
      )}
    </div>
  );
};