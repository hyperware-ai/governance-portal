import React, { useState, useEffect } from 'react';
import { useGovernanceStore } from '../store/governance';

interface ProposalDiscussionProps {
  proposalId: string;
  proposalTitle?: string;
}

interface DiscussionMessage {
  id: string;
  author: string;
  content: string;
  timestamp: any;
  parent_id?: string;
  replies?: DiscussionMessage[];
}

const DiscussionThread: React.FC<{ 
  message: DiscussionMessage; 
  proposalId: string;
  depth?: number;
}> = ({ message, proposalId, depth = 0 }) => {
  const [expanded, setExpanded] = useState(true);
  const [replyText, setReplyText] = useState('');
  const [showReplyBox, setShowReplyBox] = useState(false);
  const { addDiscussion } = useGovernanceStore();

  const handleReply = async () => {
    if (replyText.trim()) {
      await addDiscussion(proposalId, replyText, message.id);
      setReplyText('');
      setShowReplyBox(false);
    }
  };

  return (
    <div style={{ marginLeft: `${depth * 20}px`, marginBottom: '10px' }}>
      <div style={{ 
        padding: '10px', 
        border: '1px solid #ddd', 
        borderRadius: '4px',
        backgroundColor: depth > 0 ? '#f9f9f9' : '#fff' 
      }}>
        <div style={{ marginBottom: '5px' }}>
          <span style={{ fontWeight: 'bold' }}>{message.author}</span>
          <span style={{ color: '#666', marginLeft: '10px', fontSize: '0.9em' }}>
            {new Date(message.timestamp?.wallTime || 0).toLocaleString()}
          </span>
        </div>
        <div style={{ marginBottom: '10px' }}>{message.content}</div>
        
        <div style={{ display: 'flex', gap: '10px' }}>
          <button 
            onClick={() => setShowReplyBox(!showReplyBox)}
            style={{ fontSize: '0.9em' }}
          >
            Reply
          </button>
          {message.replies && message.replies.length > 0 && (
            <button 
              onClick={() => setExpanded(!expanded)}
              style={{ fontSize: '0.9em' }}
            >
              {expanded ? 'Hide' : 'Show'} Replies ({message.replies.length})
            </button>
          )}
        </div>

        {showReplyBox && (
          <div style={{ marginTop: '10px', display: 'flex', gap: '5px' }}>
            <textarea
              value={replyText}
              onChange={(e) => setReplyText(e.target.value)}
              onKeyPress={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleReply();
                }
              }}
              placeholder="Write a reply..."
              style={{ 
                flex: 1,
                minHeight: '50px',
                padding: '5px',
                borderRadius: '4px',
                border: '1px solid #ddd'
              }}
            />
            <button
              onClick={handleReply}
              disabled={!replyText.trim()}
            >
              Send
            </button>
          </div>
        )}
      </div>

      {expanded && message.replies && message.replies.length > 0 && (
        <div style={{ marginTop: '5px' }}>
          {message.replies.map((reply) => (
            <DiscussionThread
              key={reply.id}
              message={reply}
              proposalId={proposalId}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export const ProposalDiscussion: React.FC<ProposalDiscussionProps> = ({ 
  proposalId, 
  proposalTitle 
}) => {
  const [newComment, setNewComment] = useState('');
  const { discussions, addDiscussion, fetchDiscussions } = useGovernanceStore();
  const proposalDiscussions = discussions.get(proposalId) || [];

  useEffect(() => {
    if (proposalId) {
      fetchDiscussions(proposalId);
    }
  }, [proposalId, fetchDiscussions]);

  const handleSubmitComment = async () => {
    if (newComment.trim()) {
      await addDiscussion(proposalId, newComment);
      setNewComment('');
      // Refresh discussions
      await fetchDiscussions(proposalId);
    }
  };

  // Organize discussions into threads
  const organizeThreads = (messages: any[]): DiscussionMessage[] => {
    const messageMap = new Map<string, DiscussionMessage>();
    const rootMessages: DiscussionMessage[] = [];

    // First pass: create all messages
    messages.forEach(msg => {
      messageMap.set(msg.id, { ...msg, replies: [] });
    });

    // Second pass: organize into tree structure
    messages.forEach(msg => {
      const message = messageMap.get(msg.id)!;
      if (msg.parent_id) {
        const parent = messageMap.get(msg.parent_id);
        if (parent) {
          parent.replies = parent.replies || [];
          parent.replies.push(message);
        } else {
          rootMessages.push(message);
        }
      } else {
        rootMessages.push(message);
      }
    });

    return rootMessages;
  };

  const threads = organizeThreads(proposalDiscussions);

  return (
    <div style={{ padding: '20px' }}>
      {proposalTitle && (
        <h3>Discussion: {proposalTitle}</h3>
      )}
      
      <div style={{ marginBottom: '20px' }}>
        <textarea
          value={newComment}
          onChange={(e) => setNewComment(e.target.value)}
          placeholder="Share your thoughts on this proposal..."
          style={{ 
            width: '100%',
            minHeight: '80px',
            padding: '10px',
            borderRadius: '4px',
            border: '1px solid #ddd'
          }}
        />
        <button
          onClick={handleSubmitComment}
          disabled={!newComment.trim()}
          style={{ marginTop: '10px' }}
        >
          Post Comment
        </button>
      </div>

      <hr style={{ margin: '20px 0' }} />

      {threads.length === 0 ? (
        <p style={{ textAlign: 'center', color: '#666' }}>
          No comments yet. Be the first to share your thoughts!
        </p>
      ) : (
        <div>
          {threads.map((message) => (
            <DiscussionThread
              key={message.id}
              message={message}
              proposalId={proposalId}
            />
          ))}
        </div>
      )}
    </div>
  );
};