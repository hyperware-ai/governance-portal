import React, { useEffect } from 'react';
import { useGovernanceStore } from '../store/governance';

export const DaoInfo: React.FC = () => {
  const { votingPower, fetchVotingPower, isLoading } = useGovernanceStore();

  useEffect(() => {
    fetchVotingPower();
  }, []);

  return (
    <div className="dao-info">
      <h3>DAO Information</h3>
      {isLoading && !votingPower ? (
        <div className="loading">Loading DAO info...</div>
      ) : votingPower ? (
        <div className="info-grid">
          <div className="info-item">
            <span className="label">Your Voting Power:</span>
            <span className="value">{votingPower.voting_power}</span>
          </div>
          <div className="info-item">
            <span className="label">Delegated Power:</span>
            <span className="value">{votingPower.delegated_power}</span>
          </div>
          <div className="info-item">
            <span className="label">Total Supply:</span>
            <span className="value">{votingPower.total_supply}</span>
          </div>
        </div>
      ) : (
        <p>Connect wallet to see voting power</p>
      )}
    </div>
  );
};