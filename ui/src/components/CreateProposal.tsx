import React, { useState } from 'react';
import { useGovernanceStore } from '../store/governance';

interface CreateProposalProps {
  walletAddress?: `0x${string}`;
  isConnected?: boolean;
}

export const CreateProposal: React.FC<CreateProposalProps> = ({ walletAddress, isConnected }) => {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [targets, setTargets] = useState<string[]>(['']);
  const [values, setValues] = useState<string[]>(['0']);
  const [calldatas, setCalldatas] = useState<string[]>(['0x']);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isDraft, setIsDraft] = useState(true);
  const { createDraft, error, clearError } = useGovernanceStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!title || !description) {
      alert('Please fill in all fields');
      return;
    }

    if (!isDraft && !isConnected) {
      alert('Please connect your wallet to submit to chain');
      return;
    }

    setIsSubmitting(true);
    try {
      if (isDraft) {
        await createDraft(title, description);
        alert('Draft created successfully!');
      } else {
        // TODO: Submit to chain using wagmi
        alert('Chain submission not yet implemented');
      }
      
      setTitle('');
      setDescription('');
      setTargets(['']);
      setValues(['0']);
      setCalldatas(['0x']);
    } catch (err) {
      console.error('Failed to create proposal:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  const addAction = () => {
    setTargets([...targets, '']);
    setValues([...values, '0']);
    setCalldatas([...calldatas, '0x']);
  };

  const removeAction = (index: number) => {
    setTargets(targets.filter((_, i) => i !== index));
    setValues(values.filter((_, i) => i !== index));
    setCalldatas(calldatas.filter((_, i) => i !== index));
  };

  const updateTarget = (index: number, value: string) => {
    const newTargets = [...targets];
    newTargets[index] = value;
    setTargets(newTargets);
  };

  const updateValue = (index: number, value: string) => {
    const newValues = [...values];
    newValues[index] = value;
    setValues(newValues);
  };

  const updateCalldata = (index: number, value: string) => {
    const newCalldatas = [...calldatas];
    newCalldatas[index] = value;
    setCalldatas(newCalldatas);
  };

  return (
    <div className="create-proposal">
      <h2>Create New Proposal</h2>
      {error && (
        <div className="error-message">
          {error}
          <button onClick={clearError}>×</button>
        </div>
      )}
      
      <div className="proposal-type-selector">
        <label>
          <input
            type="radio"
            checked={isDraft}
            onChange={() => setIsDraft(true)}
          />
          Save as Draft
        </label>
        <label>
          <input
            type="radio"
            checked={!isDraft}
            onChange={() => setIsDraft(false)}
            disabled={!isConnected}
          />
          Submit to Chain {!isConnected && '(Connect Wallet)'}
        </label>
      </div>

      <form onSubmit={handleSubmit}>
        <div className="form-group">
          <label htmlFor="title">Title</label>
          <input
            id="title"
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Enter proposal title"
            disabled={isSubmitting}
          />
        </div>
        
        <div className="form-group">
          <label htmlFor="description">Description</label>
          <textarea
            id="description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Enter detailed proposal description (Markdown supported)"
            rows={10}
            disabled={isSubmitting}
          />
        </div>

        {!isDraft && (
          <div className="proposal-actions">
            <h3>Proposal Actions</h3>
            <p className="text-muted">Define the on-chain actions this proposal will execute</p>
            
            {targets.map((target, index) => (
              <div key={index} className="action-group">
                <h4>Action {index + 1}</h4>
                <div className="form-group">
                  <label>Target Contract Address</label>
                  <input
                    type="text"
                    value={target}
                    onChange={(e) => updateTarget(index, e.target.value)}
                    placeholder="0x..."
                    disabled={isSubmitting}
                  />
                </div>
                <div className="form-group">
                  <label>ETH Value (in wei)</label>
                  <input
                    type="text"
                    value={values[index]}
                    onChange={(e) => updateValue(index, e.target.value)}
                    placeholder="0"
                    disabled={isSubmitting}
                  />
                </div>
                <div className="form-group">
                  <label>Calldata</label>
                  <input
                    type="text"
                    value={calldatas[index]}
                    onChange={(e) => updateCalldata(index, e.target.value)}
                    placeholder="0x..."
                    disabled={isSubmitting}
                  />
                </div>
                {targets.length > 1 && (
                  <button
                    type="button"
                    onClick={() => removeAction(index)}
                    className="remove-action"
                  >
                    Remove Action
                  </button>
                )}
              </div>
            ))}
            
            <button type="button" onClick={addAction} className="add-action">
              + Add Action
            </button>
          </div>
        )}

        <div className="form-actions">
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? 'Creating...' : isDraft ? 'Create Draft' : 'Submit to Chain'}
          </button>
          <button type="button" onClick={() => {
            setTitle('');
            setDescription('');
            setTargets(['']);
            setValues(['0']);
            setCalldatas(['0x']);
          }}>
            Clear
          </button>
        </div>

        {walletAddress && !isDraft && (
          <p className="submitting-info">
            Submitting as: {walletAddress.slice(0, 6)}...{walletAddress.slice(-4)}
          </p>
        )}
      </form>
    </div>
  );
};