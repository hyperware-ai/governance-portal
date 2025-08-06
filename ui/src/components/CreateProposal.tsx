import React, { useState } from 'react';
import { useGovernanceStore } from '../store/governance';

export const CreateProposal: React.FC = () => {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const { createDraft, error, clearError } = useGovernanceStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!title || !description) {
      alert('Please fill in all fields');
      return;
    }

    setIsSubmitting(true);
    try {
      await createDraft(title, description);
      setTitle('');
      setDescription('');
      alert('Draft created successfully!');
    } catch (err) {
      console.error('Failed to create draft:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="create-proposal">
      <h2>Create New Proposal Draft</h2>
      {error && (
        <div className="error-message">
          {error}
          <button onClick={clearError}>×</button>
        </div>
      )}
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
            placeholder="Enter detailed proposal description"
            rows={10}
            disabled={isSubmitting}
          />
        </div>

        <div className="form-actions">
          <button type="submit" disabled={isSubmitting}>
            {isSubmitting ? 'Creating...' : 'Create Draft'}
          </button>
          <button type="button" onClick={() => {
            setTitle('');
            setDescription('');
          }}>
            Clear
          </button>
        </div>
      </form>
    </div>
  );
};