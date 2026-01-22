import React, { useState, useEffect } from 'react';

// Icon components
const FolderIcon = ({ size = 16 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
    <path d="M2 4.5A1.5 1.5 0 013.5 3h2.879a1.5 1.5 0 011.06.44l.622.62a1.5 1.5 0 001.06.44H12.5A1.5 1.5 0 0114 6v5.5a1.5 1.5 0 01-1.5 1.5h-9A1.5 1.5 0 012 11.5v-7z" />
  </svg>
);

const FileIcon = ({ size = 16 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
    <path d="M4 2.5A1.5 1.5 0 015.5 1h3.379a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 01.439 1.061V12.5A1.5 1.5 0 0111 14H5.5A1.5 1.5 0 014 12.5v-10z" />
    <path d="M9 1v3.5a.5.5 0 00.5.5H13" />
  </svg>
);

const ChevronIcon = ({ expanded, size = 12 }) => (
  <svg width={size} height={size} viewBox="0 0 12 12" fill="currentColor" style={{ transform: expanded ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 0.15s ease' }}>
    <path d="M4.5 2.5l4 3.5-4 3.5V2.5z" />
  </svg>
);

const CheckIcon = ({ size = 14 }) => (
  <svg width={size} height={size} viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M3 7.5l2.5 2.5 5.5-6" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const SpinnerIcon = ({ size = 14 }) => (
  <svg width={size} height={size} viewBox="0 0 14 14" style={{ animation: 'spin 1s linear infinite' }}>
    <circle cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.5" opacity="0.25" />
    <path d="M7 1.5A5.5 5.5 0 0112.5 7" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const SendIcon = ({ size = 16 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="currentColor">
    <path d="M2.5 2.5l11 5.5-11 5.5v-4l7-1.5-7-1.5v-4z" />
  </svg>
);

const SidebarLeftIcon = ({ size = 16 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
    <rect x="2" y="3" width="12" height="10" rx="1.5" />
    <path d="M6 3v10" />
  </svg>
);

const SidebarRightIcon = ({ size = 16 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
    <rect x="2" y="3" width="12" height="10" rx="1.5" />
    <path d="M10 3v10" />
  </svg>
);

const BackIcon = ({ size = 16 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
    <path d="M10 3L5 8l5 5" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const TerminalIcon = ({ size = 32 }) => (
  <svg width={size} height={size} viewBox="0 0 32 32" fill="none">
    <rect x="2" y="5" width="28" height="22" rx="3" fill="#1a1a1a" stroke="#333" strokeWidth="1" />
    <rect x="4" y="7" width="24" height="2" rx="1" fill="#333" />
    <circle cx="6" cy="8" r="1" fill="#ff5f57" />
    <circle cx="9" cy="8" r="1" fill="#febc2e" />
    <circle cx="12" cy="8" r="1" fill="#28c840" />
    <path d="M8 15l4 3-4 3" stroke="#3b82f6" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <path d="M14 21h6" stroke="#666" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const SkillIcon = ({ size = 12 }) => (
  <svg width={size} height={size} viewBox="0 0 12 12" fill="currentColor" opacity="0.5">
    <circle cx="6" cy="6" r="2" />
  </svg>
);

// Simulated file tree
const sampleFileTree = [
  {
    name: 'src',
    type: 'folder',
    children: [
      { name: 'main.ts', type: 'file' },
      { name: 'config.ts', type: 'file' },
      {
        name: 'components',
        type: 'folder',
        children: [
          { name: 'App.tsx', type: 'file' },
          { name: 'Button.tsx', type: 'file' },
          { name: 'Input.tsx', type: 'file' },
        ],
      },
      {
        name: 'utils',
        type: 'folder',
        children: [
          { name: 'helpers.ts', type: 'file' },
          { name: 'constants.ts', type: 'file' },
        ],
      },
    ],
  },
  { name: 'package.json', type: 'file' },
  { name: 'tsconfig.json', type: 'file' },
  { name: 'README.md', type: 'file' },
  { name: '.gitignore', type: 'file' },
];

// Recent projects
const recentProjects = [
  { name: 'claude-desktop-app', path: '~/Projects/claude-desktop-app', lastOpened: '2 hours ago' },
  { name: 'api-server', path: '~/Work/api-server', lastOpened: 'Yesterday' },
  { name: 'website-redesign', path: '~/Projects/website-redesign', lastOpened: '3 days ago' },
];

// Sample chat messages
const sampleMessages = [
  { role: 'user', content: 'Can you help me refactor the authentication module to use JWT tokens?' },
  { role: 'assistant', content: 'I\'ll help you refactor the authentication module. Let me first examine your current implementation.\n\nI\'ve found the auth module at `src/auth/`. Here\'s my plan:\n\n1. Update the token generation to use JWT\n2. Add token verification middleware\n3. Update the refresh token logic\n\nShall I proceed with these changes?' },
];

// File Tree Component
const FileTreeItem = ({ item, depth = 0, selectedFile, onSelect }) => {
  const [expanded, setExpanded] = useState(depth < 1);
  const isFolder = item.type === 'folder';
  const isSelected = selectedFile === item.name;

  return (
    <div>
      <div
        onClick={() => {
          if (isFolder) setExpanded(!expanded);
          else onSelect(item.name);
        }}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '3px 8px',
          paddingLeft: `${8 + depth * 12}px`,
          cursor: 'pointer',
          backgroundColor: isSelected ? 'rgba(59, 130, 246, 0.15)' : 'transparent',
          borderRadius: '4px',
          margin: '1px 4px',
          fontSize: '12px',
          color: isSelected ? '#3b82f6' : '#8b8b8b',
          transition: 'background-color 0.1s ease',
        }}
        onMouseEnter={(e) => !isSelected && (e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.04)')}
        onMouseLeave={(e) => !isSelected && (e.currentTarget.style.backgroundColor = 'transparent')}
      >
        {isFolder && (
          <span style={{ width: '12px', display: 'flex', alignItems: 'center', color: '#666' }}>
            <ChevronIcon expanded={expanded} />
          </span>
        )}
        {!isFolder && <span style={{ width: '12px' }} />}
        <span style={{ color: isFolder ? '#a78bfa' : '#8b8b8b', display: 'flex', alignItems: 'center' }}>
          {isFolder ? <FolderIcon size={14} /> : <FileIcon size={14} />}
        </span>
        <span style={{ color: isSelected ? '#3b82f6' : '#c9c9c9' }}>{item.name}</span>
      </div>
      {isFolder && expanded && item.children && (
        <div>
          {item.children.map((child, i) => (
            <FileTreeItem key={i} item={child} depth={depth + 1} selectedFile={selectedFile} onSelect={onSelect} />
          ))}
        </div>
      )}
    </div>
  );
};

// Welcome Screen
const WelcomeScreen = ({ onContinue }) => {
  const [cliInstalled, setCliInstalled] = useState(null);
  const [authenticated, setAuthenticated] = useState(null);

  useEffect(() => {
    setTimeout(() => setCliInstalled(true), 800);
    setTimeout(() => setAuthenticated(true), 1500);
  }, []);

  const allChecked = cliInstalled && authenticated;

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      gap: '32px',
      padding: '40px',
    }}>
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center' }}>
        <TerminalIcon size={48} />
        <h1 style={{ fontSize: '18px', fontWeight: '600', marginTop: '16px', color: '#e5e5e5' }}>
          Claude Code
        </h1>
        <p style={{ fontSize: '12px', color: '#737373', marginTop: '4px' }}>
          AI-powered development environment
        </p>
      </div>

      <div style={{
        background: 'rgba(255,255,255,0.02)',
        border: '1px solid rgba(255,255,255,0.06)',
        borderRadius: '8px',
        padding: '16px 20px',
        width: '280px',
      }}>
        <div style={{ fontSize: '11px', fontWeight: '500', color: '#737373', marginBottom: '12px', textTransform: 'uppercase', letterSpacing: '0.5px' }}>
          System Check
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <span style={{ fontSize: '13px', color: '#a3a3a3' }}>Claude CLI installed</span>
            <span style={{ color: cliInstalled ? '#3b82f6' : '#737373' }}>
              {cliInstalled === null ? <SpinnerIcon /> : cliInstalled ? <CheckIcon /> : '✗'}
            </span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <span style={{ fontSize: '13px', color: '#a3a3a3' }}>Authenticated</span>
            <span style={{ color: authenticated ? '#3b82f6' : '#737373' }}>
              {authenticated === null ? <SpinnerIcon /> : authenticated ? <CheckIcon /> : '✗'}
            </span>
          </div>
        </div>
      </div>

      <button
        onClick={onContinue}
        disabled={!allChecked}
        style={{
          padding: '8px 24px',
          fontSize: '13px',
          fontWeight: '500',
          backgroundColor: allChecked ? '#3b82f6' : '#262626',
          color: allChecked ? '#fff' : '#525252',
          border: 'none',
          borderRadius: '6px',
          cursor: allChecked ? 'pointer' : 'not-allowed',
          transition: 'all 0.15s ease',
          opacity: allChecked ? 1 : 0.6,
        }}
      >
        Continue
      </button>
    </div>
  );
};

// Project Selection Screen
const ProjectScreen = ({ onSelectProject }) => {
  const [isDragOver, setIsDragOver] = useState(false);

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      padding: '24px',
    }}>
      <div style={{ marginBottom: '24px' }}>
        <h2 style={{ fontSize: '14px', fontWeight: '600', color: '#e5e5e5', marginBottom: '4px' }}>
          Open a Project
        </h2>
        <p style={{ fontSize: '12px', color: '#737373' }}>
          Select a folder to start working with Claude
        </p>
      </div>

      {/* Drop zone */}
      <div
        onDragOver={(e) => { e.preventDefault(); setIsDragOver(true); }}
        onDragLeave={() => setIsDragOver(false)}
        onDrop={(e) => { e.preventDefault(); setIsDragOver(false); onSelectProject('dropped-folder'); }}
        onClick={() => onSelectProject('selected-folder')}
        style={{
          border: `1px dashed ${isDragOver ? '#3b82f6' : 'rgba(255,255,255,0.1)'}`,
          borderRadius: '8px',
          padding: '32px',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          cursor: 'pointer',
          backgroundColor: isDragOver ? 'rgba(59, 130, 246, 0.05)' : 'transparent',
          transition: 'all 0.15s ease',
          marginBottom: '24px',
        }}
      >
        <div style={{ color: '#525252', marginBottom: '8px' }}>
          <FolderIcon size={32} />
        </div>
        <p style={{ fontSize: '12px', color: '#a3a3a3', marginBottom: '4px' }}>
          Drop a folder here or click to browse
        </p>
        <p style={{ fontSize: '11px', color: '#525252' }}>
          ⌘O to open folder
        </p>
      </div>

      {/* Recent projects */}
      <div>
        <div style={{
          fontSize: '11px',
          fontWeight: '500',
          color: '#525252',
          marginBottom: '8px',
          textTransform: 'uppercase',
          letterSpacing: '0.5px',
        }}>
          Recent Projects
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
          {recentProjects.map((project, i) => (
            <div
              key={i}
              onClick={() => onSelectProject(project.path)}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '10px 12px',
                borderRadius: '6px',
                cursor: 'pointer',
                transition: 'background-color 0.1s ease',
              }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.03)'}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <span style={{ color: '#a78bfa' }}><FolderIcon size={16} /></span>
                <div>
                  <div style={{ fontSize: '13px', color: '#e5e5e5' }}>{project.name}</div>
                  <div style={{ fontSize: '11px', color: '#525252' }}>{project.path}</div>
                </div>
              </div>
              <span style={{ fontSize: '11px', color: '#525252' }}>{project.lastOpened}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

// Main Chat Interface
const ChatInterface = ({ projectName, onBack }) => {
  const [messages, setMessages] = useState(sampleMessages);
  const [input, setInput] = useState('');
  const [selectedFile, setSelectedFile] = useState(null);
  const [leftSidebarOpen, setLeftSidebarOpen] = useState(false);
  const [rightSidebarOpen, setRightSidebarOpen] = useState(false);

  const handleSend = () => {
    if (!input.trim()) return;
    setMessages([...messages, { role: 'user', content: input }]);
    setInput('');
  };

  const checklist = [
    { text: 'Analyzing codebase structure', done: true },
    { text: 'Reading authentication module', done: true },
    { text: 'Planning refactoring steps', done: true },
    { text: 'Implementing JWT generation', done: false, active: true },
    { text: 'Adding verification middleware', done: false },
    { text: 'Updating refresh token logic', done: false },
  ];

  const activeSkills = [
    'TypeScript',
    'Node.js',
    'Security Best Practices',
    'Code Refactoring',
  ];

  return (
    <div style={{ display: 'flex', height: '100%', position: 'relative' }}>
      {/* Left Sidebar - File Explorer */}
      <div style={{
        width: leftSidebarOpen ? '200px' : '0px',
        borderRight: leftSidebarOpen ? '1px solid rgba(255,255,255,0.06)' : 'none',
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: 'rgba(0,0,0,0.2)',
        overflow: 'hidden',
        transition: 'width 0.2s ease',
        flexShrink: 0,
      }}>
        <div style={{
          padding: '12px',
          borderBottom: '1px solid rgba(255,255,255,0.06)',
          fontSize: '11px',
          fontWeight: '500',
          color: '#737373',
          textTransform: 'uppercase',
          letterSpacing: '0.5px',
        }}>
          Files
        </div>

        <div style={{ flex: 1, overflow: 'auto', paddingTop: '4px' }}>
          {sampleFileTree.map((item, i) => (
            <FileTreeItem
              key={i}
              item={item}
              selectedFile={selectedFile}
              onSelect={setSelectedFile}
            />
          ))}
        </div>

        <div style={{
          padding: '8px 12px',
          borderTop: '1px solid rgba(255,255,255,0.06)',
          fontSize: '11px',
          color: '#525252',
        }}>
          <span style={{ opacity: 0.7 }}>⌘⇧O</span> Quick open
        </div>
      </div>

      {/* Main Chat Area */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        {/* Header */}
        <div style={{
          padding: '8px 12px',
          borderBottom: '1px solid rgba(255,255,255,0.06)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          backgroundColor: 'rgba(0,0,0,0.1)',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            {/* Back button */}
            <button
              onClick={onBack}
              style={{
                background: 'none',
                border: 'none',
                padding: '4px',
                cursor: 'pointer',
                color: '#737373',
                display: 'flex',
                alignItems: 'center',
                borderRadius: '4px',
                transition: 'all 0.1s ease',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.05)'; e.currentTarget.style.color = '#a3a3a3'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = '#737373'; }}
              title="Back to projects"
            >
              <BackIcon size={16} />
            </button>
            
            {/* Left sidebar toggle */}
            <button
              onClick={() => setLeftSidebarOpen(!leftSidebarOpen)}
              style={{
                background: leftSidebarOpen ? 'rgba(255,255,255,0.05)' : 'none',
                border: 'none',
                padding: '4px',
                cursor: 'pointer',
                color: leftSidebarOpen ? '#a3a3a3' : '#525252',
                display: 'flex',
                alignItems: 'center',
                borderRadius: '4px',
                transition: 'all 0.1s ease',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.05)'; e.currentTarget.style.color = '#a3a3a3'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = leftSidebarOpen ? 'rgba(255,255,255,0.05)' : 'transparent'; e.currentTarget.style.color = leftSidebarOpen ? '#a3a3a3' : '#525252'; }}
              title="Toggle file explorer"
            >
              <SidebarLeftIcon size={16} />
            </button>

            <div style={{ width: '1px', height: '16px', backgroundColor: 'rgba(255,255,255,0.08)', margin: '0 4px' }} />
            
            <span style={{ color: '#a78bfa', display: 'flex', alignItems: 'center' }}><FolderIcon size={14} /></span>
            <span style={{ fontSize: '13px', color: '#e5e5e5', fontWeight: '500' }}>{projectName}</span>
          </div>
          
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '11px', color: '#404040' }}>v0.1.0</span>
            
            {/* Right sidebar toggle */}
            <button
              onClick={() => setRightSidebarOpen(!rightSidebarOpen)}
              style={{
                background: rightSidebarOpen ? 'rgba(255,255,255,0.05)' : 'none',
                border: 'none',
                padding: '4px',
                cursor: 'pointer',
                color: rightSidebarOpen ? '#a3a3a3' : '#525252',
                display: 'flex',
                alignItems: 'center',
                borderRadius: '4px',
                transition: 'all 0.1s ease',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.05)'; e.currentTarget.style.color = '#a3a3a3'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = rightSidebarOpen ? 'rgba(255,255,255,0.05)' : 'transparent'; e.currentTarget.style.color = rightSidebarOpen ? '#a3a3a3' : '#525252'; }}
              title="Toggle context panel"
            >
              <SidebarRightIcon size={16} />
            </button>
          </div>
        </div>

        {/* Messages */}
        <div style={{ flex: 1, overflow: 'auto', padding: '16px' }}>
          {messages.length === 0 ? (
            <div style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              height: '100%',
              color: '#525252',
            }}>
              <div style={{ marginBottom: '12px', opacity: 0.15 }}>
                <TerminalIcon size={48} />
              </div>
              <p style={{ fontSize: '13px', marginBottom: '4px', color: '#737373' }}>
                Start a conversation
              </p>
              <p style={{ fontSize: '11px' }}>
                Ask Claude to help with your code
              </p>
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0' }}>
              {messages.map((msg, i) => (
                <div key={i}>
                  {/* Divider between messages */}
                  {i > 0 && (
                    <div style={{
                      height: '1px',
                      backgroundColor: 'rgba(255,255,255,0.04)',
                      margin: '16px 0',
                    }} />
                  )}
                  
                  <div style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '6px',
                  }}>
                    {/* Role label */}
                    <div style={{
                      fontSize: '11px',
                      fontWeight: '500',
                      color: msg.role === 'user' ? '#737373' : '#737373',
                      textTransform: 'uppercase',
                      letterSpacing: '0.5px',
                    }}>
                      {msg.role === 'user' ? 'You' : 'Claude'}
                    </div>
                    
                    {/* Message content */}
                    <div style={{
                      fontSize: '13px',
                      lineHeight: '1.6',
                      color: msg.role === 'user' ? '#a3a3a3' : '#e5e5e5',
                      whiteSpace: 'pre-wrap',
                      paddingLeft: '0',
                    }}>
                      {msg.content}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Input area with footer info */}
        <div style={{
          padding: '12px 16px',
          borderTop: '1px solid rgba(255,255,255,0.06)',
          backgroundColor: 'rgba(0,0,0,0.1)',
        }}>
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            backgroundColor: 'rgba(255,255,255,0.03)',
            border: '1px solid rgba(255,255,255,0.08)',
            borderRadius: '8px',
            padding: '10px 12px',
          }}>
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
              placeholder="Ask Claude something..."
              rows={1}
              style={{
                flex: 1,
                background: 'none',
                border: 'none',
                outline: 'none',
                resize: 'none',
                fontSize: '13px',
                color: '#e5e5e5',
                fontFamily: 'inherit',
                lineHeight: '1.4',
                padding: '0',
                margin: '0',
                verticalAlign: 'middle',
              }}
            />
            <button
              onClick={handleSend}
              disabled={!input.trim()}
              style={{
                background: input.trim() ? '#3b82f6' : 'rgba(255,255,255,0.05)',
                border: 'none',
                borderRadius: '6px',
                padding: '6px 8px',
                cursor: input.trim() ? 'pointer' : 'default',
                color: input.trim() ? '#fff' : '#525252',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                transition: 'all 0.15s ease',
                flexShrink: 0,
              }}
            >
              <SendIcon size={14} />
            </button>
          </div>
          
          {/* Footer row with hints and stats */}
          <div style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginTop: '8px',
            fontSize: '11px',
            color: '#404040',
          }}>
            <span>⇧↵ new line · ⌘↵ send</span>
            <div style={{ display: 'flex', gap: '16px' }}>
              <span>2,847 / 200k tokens</span>
              <span>12m 34s</span>
            </div>
          </div>
        </div>
      </div>

      {/* Right Sidebar - Context Panel */}
      <div style={{
        width: rightSidebarOpen ? '220px' : '0px',
        borderLeft: rightSidebarOpen ? '1px solid rgba(255,255,255,0.06)' : 'none',
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: 'rgba(0,0,0,0.2)',
        overflow: 'hidden',
        transition: 'width 0.2s ease',
        flexShrink: 0,
      }}>
        {/* Current Task */}
        <div style={{
          padding: '12px',
          borderBottom: '1px solid rgba(255,255,255,0.06)',
        }}>
          <div style={{
            fontSize: '11px',
            fontWeight: '500',
            color: '#737373',
            textTransform: 'uppercase',
            letterSpacing: '0.5px',
            marginBottom: '10px',
          }}>
            Current Task
          </div>
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '6px',
          }}>
            {checklist.map((item, i) => (
              <div key={i} style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: '8px',
                fontSize: '12px',
                color: item.done ? '#525252' : item.active ? '#e5e5e5' : '#737373',
              }}>
                <span style={{
                  width: '14px',
                  height: '14px',
                  borderRadius: '4px',
                  border: item.done ? 'none' : '1px solid rgba(255,255,255,0.15)',
                  backgroundColor: item.done ? '#3b82f6' : item.active ? 'rgba(59, 130, 246, 0.2)' : 'transparent',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexShrink: 0,
                  marginTop: '1px',
                }}>
                  {item.done && <CheckIcon size={10} />}
                  {item.active && <span style={{ width: '6px', height: '6px', borderRadius: '50%', backgroundColor: '#3b82f6' }} />}
                </span>
                <span style={{ textDecoration: item.done ? 'line-through' : 'none' }}>{item.text}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Active Skills */}
        <div style={{ padding: '12px' }}>
          <div style={{
            fontSize: '11px',
            fontWeight: '500',
            color: '#737373',
            textTransform: 'uppercase',
            letterSpacing: '0.5px',
            marginBottom: '10px',
          }}>
            Active Skills
          </div>
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '6px',
          }}>
            {activeSkills.map((skill, i) => (
              <div key={i} style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                fontSize: '12px',
                color: '#c9c9c9',
              }}>
                <SkillIcon size={12} />
                <span>{skill}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

// Main App
export default function ClaudeCodeApp() {
  const [screen, setScreen] = useState('welcome');
  const [projectName, setProjectName] = useState('');

  return (
    <div style={{
      width: '100%',
      height: '600px',
      backgroundColor: '#0d0d0d',
      color: '#e5e5e5',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Roboto, sans-serif',
      fontSize: '13px',
      borderRadius: '10px',
      overflow: 'hidden',
      border: '1px solid rgba(255,255,255,0.1)',
      display: 'flex',
      flexDirection: 'column',
    }}>
      {/* Title bar (macOS style) */}
      <div style={{
        height: '38px',
        backgroundColor: 'rgba(30,30,30,0.95)',
        borderBottom: '1px solid rgba(255,255,255,0.06)',
        display: 'flex',
        alignItems: 'center',
        padding: '0 12px',
        WebkitAppRegion: 'drag',
        flexShrink: 0,
      }}>
        <div style={{ display: 'flex', gap: '8px', marginRight: '16px' }}>
          <div style={{ width: '12px', height: '12px', borderRadius: '50%', backgroundColor: '#ff5f57' }} />
          <div style={{ width: '12px', height: '12px', borderRadius: '50%', backgroundColor: '#febc2e' }} />
          <div style={{ width: '12px', height: '12px', borderRadius: '50%', backgroundColor: '#28c840' }} />
        </div>
        <div style={{ flex: 1, textAlign: 'center', fontSize: '12px', color: '#737373', fontWeight: '500' }}>
          {screen === 'chat' ? `Claude Code — ${projectName}` : 'Claude Code'}
        </div>
        <div style={{ width: '68px' }} />
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflow: 'hidden' }}>
        {screen === 'welcome' && (
          <WelcomeScreen onContinue={() => setScreen('project')} />
        )}
        {screen === 'project' && (
          <ProjectScreen onSelectProject={(path) => {
            setProjectName(path.split('/').pop() || 'my-project');
            setScreen('chat');
          }} />
        )}
        {screen === 'chat' && (
          <ChatInterface 
            projectName={projectName} 
            onBack={() => setScreen('project')}
          />
        )}
      </div>

      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        textarea::placeholder {
          color: #525252;
        }
        ::-webkit-scrollbar {
          width: 8px;
          height: 8px;
        }
        ::-webkit-scrollbar-track {
          background: transparent;
        }
        ::-webkit-scrollbar-thumb {
          background: rgba(255,255,255,0.1);
          border-radius: 4px;
        }
        ::-webkit-scrollbar-thumb:hover {
          background: rgba(255,255,255,0.15);
        }
      `}</style>
    </div>
  );
}