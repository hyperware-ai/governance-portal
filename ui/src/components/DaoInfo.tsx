import React, { useEffect } from 'react';
import { useGovernanceStore } from '../store/governance';

interface DaoInfoProps {
  walletAddress?: `0x${string}`;
  isConnected?: boolean;
}

export const DaoInfo: React.FC<DaoInfoProps> = ({ walletAddress, isConnected }) => {
  const { votingPower, fetchVotingPower, isLoading } = useGovernanceStore();

  useEffect(() => {
    if (isConnected && walletAddress) {
      fetchVotingPower(walletAddress);
    }
  }, [isConnected, walletAddress, fetchVotingPower]);

  return (
    <div className="dao-info">
      <h3>DAO Information</h3>
      {isLoading && !votingPower ? (
        <div className="loading">Loading DAO info...</div>
      ) : (
        <div className="info-grid">
          {isConnected && walletAddress && (
            <div className="info-item">
              <span className="label">Wallet:</span>
              <span className="value" style={{ fontSize: '0.85rem', fontFamily: 'monospace' }}>
                {walletAddress.slice(0, 6)}...{walletAddress.slice(-4)}
              </span>
            </div>
          )}
          <div className="info-item">
            <span className="label">Your Voting Power:</span>
            <span className="value">
              {isConnected ? (votingPower?.voting_power || '0') : 'Connect Wallet'}
            </span>
          </div>
          <div className="info-item">
            <span className="label">Delegated Power:</span>
            <span className="value">
              {isConnected ? (votingPower?.delegated_power || '0') : '-'}
            </span>
          </div>
          <div className="info-item">
            <span className="label">Total Supply:</span>
            <span className="value">{votingPower?.total_supply || '0'}</span>
          </div>
          <div className="info-item">
            <span className="label">Governor:</span>
            <span className="value" style={{ fontSize: '0.75rem', fontFamily: 'monospace' }}>
              0x1234...7890
            </span>
          </div>
          <div className="info-item">
            <span className="label">Token:</span>
            <span className="value" style={{ fontSize: '0.75rem', fontFamily: 'monospace' }}>
              0x2345...8901
            </span>
          </div>
        </div>
      )}
    </div>
  );
};