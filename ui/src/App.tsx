import { useEffect, useState } from 'react';
import './App.css';
import '@rainbow-me/rainbowkit/styles.css';
import { RainbowKitProvider, ConnectButton } from '@rainbow-me/rainbowkit';
import { WagmiProvider, useAccount } from 'wagmi';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { config } from './config/wallet';
import { useGovernanceStore } from './store/governance';
import { ProposalList } from './components/ProposalList';
import { CreateProposal } from './components/CreateProposal';
import { CommitteeStatus } from './components/CommitteeStatus';
import { DaoInfo } from './components/DaoInfo';
import { ProposalDetail } from './components/ProposalDetail';
import { VotingPanel } from './components/VotingPanel';

const queryClient = new QueryClient();

function AppContent() {
  const [activeTab, setActiveTab] = useState<'proposals' | 'create' | 'committee'>('proposals');
  const [selectedProposal, setSelectedProposal] = useState<string | null>(null);
  const { error, clearError, syncWithCommittee, isSyncing } = useGovernanceStore();
  const [nodeId, setNodeId] = useState<string>('');
  const { address, isConnected } = useAccount();

  useEffect(() => {
    if (typeof window !== 'undefined' && (window as any).our) {
      setNodeId((window as any).our.node);
    }
    
    syncWithCommittee();
    
    const syncInterval = setInterval(() => {
      syncWithCommittee();
    }, 60000);
    
    return () => clearInterval(syncInterval);
  }, []);

  return (
    <div className="app">
      <header className="app-header">
        <h1 className="app-title">🏛️ DAO Governance Portal</h1>
        <div className="node-info">
          {nodeId ? (
            <>Connected as <span className="badge info">{nodeId}</span></>
          ) : (
            <span className="text-muted">Connecting to Hyperware...</span>
          )}
        </div>
        <div style={{ marginTop: '1rem' }}>
          <ConnectButton />
        </div>
        {isSyncing && <span className="badge warning">Syncing...</span>}
      </header>

      {error && (
        <div className="error error-message">
          {error}
          <button onClick={clearError} style={{ marginLeft: '1rem' }}>
            Dismiss
          </button>
        </div>
      )}

      <div className="main-container">
        <div className="sidebar">
          <DaoInfo walletAddress={address} isConnected={isConnected} />
          <CommitteeStatus />
          {selectedProposal && (
            <VotingPanel 
              proposalId={selectedProposal}
              walletAddress={address}
              isConnected={isConnected}
            />
          )}
        </div>

        <div className="content">
          <nav className="tabs">
            <button 
              className={`tab ${activeTab === 'proposals' ? 'active' : ''}`}
              onClick={() => {
                setActiveTab('proposals');
                setSelectedProposal(null);
              }}
            >
              Proposals
            </button>
            <button 
              className={`tab ${activeTab === 'create' ? 'active' : ''}`}
              onClick={() => {
                setActiveTab('create');
                setSelectedProposal(null);
              }}
            >
              Create Proposal
            </button>
            <button 
              className={`tab ${activeTab === 'committee' ? 'active' : ''}`}
              onClick={() => {
                setActiveTab('committee');
                setSelectedProposal(null);
              }}
            >
              Committee
            </button>
          </nav>

          <div className="tab-content">
            {activeTab === 'proposals' && !selectedProposal && (
              <ProposalList onSelectProposal={setSelectedProposal} />
            )}
            {activeTab === 'proposals' && selectedProposal && (
              <ProposalDetail 
                proposalId={selectedProposal}
                onBack={() => setSelectedProposal(null)}
              />
            )}
            {activeTab === 'create' && (
              <CreateProposal walletAddress={address} isConnected={isConnected} />
            )}
            {activeTab === 'committee' && (
              <div className="committee-tab">
                <h2>Committee Management</h2>
                <CommitteeStatus />
                <div className="committee-actions mt-3">
                  <button className="primary" onClick={syncWithCommittee} disabled={isSyncing}>
                    {isSyncing ? 'Syncing...' : 'Force Sync'}
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function App() {
  return (
    <WagmiProvider config={config}>
      <QueryClientProvider client={queryClient}>
        <RainbowKitProvider>
          <AppContent />
        </RainbowKitProvider>
      </QueryClientProvider>
    </WagmiProvider>
  );
}

export default App;