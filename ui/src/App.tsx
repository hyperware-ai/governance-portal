import { useEffect, useState } from 'react';
import './App.css';
import { useGovernanceStore } from './store/governance';
import { ProposalList } from './components/ProposalList';
import { CreateProposal } from './components/CreateProposal';
import { CommitteeStatus } from './components/CommitteeStatus';
import { DaoInfo } from './components/DaoInfo';

function App() {
  const [activeTab, setActiveTab] = useState<'proposals' | 'create' | 'committee'>('proposals');
  const { error, clearError, syncWithCommittee, isSyncing } = useGovernanceStore();
  const [nodeId, setNodeId] = useState<string>('');

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
          <DaoInfo />
          <CommitteeStatus />
        </div>

        <div className="content">
          <nav className="tabs">
            <button 
              className={`tab ${activeTab === 'proposals' ? 'active' : ''}`}
              onClick={() => setActiveTab('proposals')}
            >
              Proposals
            </button>
            <button 
              className={`tab ${activeTab === 'create' ? 'active' : ''}`}
              onClick={() => setActiveTab('create')}
            >
              Create Proposal
            </button>
            <button 
              className={`tab ${activeTab === 'committee' ? 'active' : ''}`}
              onClick={() => setActiveTab('committee')}
            >
              Committee
            </button>
          </nav>

          <div className="tab-content">
            {activeTab === 'proposals' && <ProposalList />}
            {activeTab === 'create' && <CreateProposal />}
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

export default App;