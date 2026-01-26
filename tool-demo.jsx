import React, { useState, useEffect } from 'react';

const TOOLS = {
  web_search: {
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
        <circle cx="11" cy="11" r="8"/>
        <path d="m21 21-4.35-4.35"/>
      </svg>
    ),
    label: 'Web Search',
    color: '#64748b',
    bgColor: '#f1f5f9',
  },
  web_fetch: {
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
        <circle cx="12" cy="12" r="10"/>
        <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
      </svg>
    ),
    label: 'Fetch',
    color: '#64748b',
    bgColor: '#f1f5f9',
  },
  create_document: {
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14,2 14,8 20,8"/>
        <line x1="12" y1="18" x2="12" y2="12"/>
        <line x1="9" y1="15" x2="15" y2="15"/>
      </svg>
    ),
    label: 'Create',
    color: '#64748b',
    bgColor: '#f1f5f9',
  },
  read_document: {
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14,2 14,8 20,8"/>
        <line x1="16" y1="13" x2="8" y2="13"/>
        <line x1="16" y1="17" x2="8" y2="17"/>
      </svg>
    ),
    label: 'Read',
    color: '#64748b',
    bgColor: '#f1f5f9',
  },
  bash_command: {
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
        <polyline points="4,17 10,11 4,5"/>
        <line x1="12" y1="19" x2="20" y2="19"/>
      </svg>
    ),
    label: 'Terminal',
    color: '#64748b',
    bgColor: '#f1f5f9',
  },
  code_edit: {
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4">
        <polyline points="16,18 22,12 16,6"/>
        <polyline points="8,6 2,12 8,18"/>
      </svg>
    ),
    label: 'Edit',
    color: '#64748b',
    bgColor: '#f1f5f9',
  },
};

const ToolIcon = ({ tool, index, total, isExpanded }) => {
  const toolConfig = TOOLS[tool.type];
  const offset = isExpanded ? index * 28 : index * 16;
  const zIndex = index + 1;
  
  return (
    <div
      className="absolute transition-all duration-400 ease-out"
      style={{
        left: `${offset}px`,
        zIndex,
        animation: 'iconSlideIn 0.35s ease-out both',
      }}
    >
      <div
        className="w-7 h-7 rounded-full flex items-center justify-center border border-slate-200 bg-white"
        style={{ color: toolConfig.color }}
      >
        {toolConfig.icon}
      </div>
    </div>
  );
};

const ToolCallStack = ({ tools, isAnimating, currentLabel }) => {
  const [isExpanded, setIsExpanded] = useState(false);
  
  const stackWidth = isExpanded 
    ? tools.length * 28 + 4
    : Math.min(tools.length, 6) * 16 + 12;
  
  return (
    <div 
      className="relative bg-slate-50 rounded-xl border border-slate-200 overflow-hidden transition-all duration-400 cursor-pointer"
      style={{ width: '100%', maxWidth: '320px' }}
      onClick={() => setIsExpanded(!isExpanded)}
    >
      <div className="flex items-center h-11 px-3 gap-3">
        {/* Stacked icons */}
        <div 
          className="relative transition-all duration-400 flex-shrink-0"
          style={{ 
            width: `${Math.max(isExpanded ? tools.length * 28 : Math.min(tools.length, 6) * 16, 28)}px`,
            height: '28px',
          }}
        >
          {tools.map((tool, index) => (
            <ToolIcon
              key={tool.id}
              tool={tool}
              index={index}
              total={tools.length}
              isExpanded={isExpanded}
            />
          ))}
        </div>
        
        {/* Current action label */}
        <div className="flex-1 min-w-0 overflow-hidden">
          {currentLabel && (
            <div 
              className="text-xs text-slate-500 truncate"
              style={{ animation: 'labelFadeIn 0.3s ease-out' }}
              key={currentLabel}
            >
              {currentLabel}
            </div>
          )}
        </div>
        
        {/* Expand indicator */}
        {tools.length > 1 && (
          <div className="flex-shrink-0 flex items-center gap-1 text-xs text-slate-400">
            <span>{tools.length}</span>
            <svg 
              className={`w-3 h-3 transition-transform duration-300 ${isExpanded ? 'rotate-180' : ''}`}
              viewBox="0 0 24 24" 
              fill="none" 
              stroke="currentColor" 
              strokeWidth="2"
            >
              <polyline points="6,9 12,15 18,9"/>
            </svg>
          </div>
        )}
      </div>
      
      {/* Expanded detail list */}
      <div className={`overflow-hidden transition-all duration-400 ${isExpanded ? 'max-h-80' : 'max-h-0'}`}>
        <div className="px-3 pb-3 space-y-1">
          <div className="h-px bg-slate-200 mb-2" />
          {tools.map((tool) => {
            const config = TOOLS[tool.type];
            return (
              <div 
                key={tool.id}
                className="flex items-center gap-2 py-1.5 px-2 rounded-lg hover:bg-slate-100 transition-colors"
              >
                <div 
                  className="w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0 bg-white border border-slate-200"
                  style={{ color: config.color }}
                >
                  {config.icon}
                </div>
                <div className="flex-1 min-w-0">
                  <span className="text-xs text-slate-600">{tool.detail}</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};

const ChatMessage = ({ message, isUser }) => (
  <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-3`}>
    <div 
      className={`max-w-[280px] px-3 py-2 rounded-2xl text-sm ${
        isUser 
          ? 'bg-slate-800 text-white rounded-br-sm' 
          : 'bg-white border border-slate-200 text-slate-700 rounded-bl-sm'
      }`}
    >
      {message}
    </div>
  </div>
);

export default function ToolCallsDemo() {
  const [tools, setTools] = useState([]);
  const [isAnimating, setIsAnimating] = useState(false);
  const [demoPhase, setDemoPhase] = useState(0);
  const [currentLabel, setCurrentLabel] = useState('');
  
  const toolSequence = [
    { type: 'web_search', detail: 'Searching "React animation libraries"' },
    { type: 'web_fetch', detail: 'Fetching framer.com/motion' },
    { type: 'web_fetch', detail: 'Fetching react-spring.io' },
    { type: 'read_document', detail: 'Reading requirements.md' },
    { type: 'create_document', detail: 'Creating AnimatedStack.jsx' },
    { type: 'code_edit', detail: 'Editing components/index.js' },
    { type: 'bash_command', detail: 'Running npm install' },
  ];
  
  const runDemo = () => {
    setTools([]);
    setCurrentLabel('');
    setIsAnimating(true);
    setDemoPhase(1);
    
    // Each tool: icon appears, then label fades in, holds for ~1.5s, then next
    const stepDuration = 1800; // Total time per step
    const labelDelay = 200; // Delay before label appears after icon
    
    toolSequence.forEach((tool, index) => {
      // Add the icon
      setTimeout(() => {
        setTools(prev => [...prev, { ...tool, id: Date.now() + index }]);
      }, index * stepDuration);
      
      // Show the label shortly after
      setTimeout(() => {
        setCurrentLabel(tool.detail);
      }, index * stepDuration + labelDelay);
      
      // Clear animation state at the end
      if (index === toolSequence.length - 1) {
        setTimeout(() => {
          setIsAnimating(false);
          setCurrentLabel('');
          setDemoPhase(2);
        }, (index + 1) * stepDuration);
      }
    });
  };
  
  return (
    <div className="min-h-screen bg-slate-100 p-4">
      <style>{`
        @keyframes iconSlideIn {
          0% { opacity: 0; transform: translateX(-12px) scale(0.9); }
          100% { opacity: 1; transform: translateX(0) scale(1); }
        }
        @keyframes labelFadeIn {
          0% { opacity: 0; transform: translateX(-4px); }
          100% { opacity: 1; transform: translateX(0); }
        }
      `}</style>
      
      <div className="max-w-sm mx-auto">
        <h1 className="text-lg font-semibold text-slate-800 mb-1">Tool Call Stack</h1>
        <p className="text-slate-500 text-xs mb-4">Tap the stack to expand details</p>
        
        <div className="bg-white rounded-2xl border border-slate-200 shadow-sm overflow-hidden">
          <div className="px-4 py-3 border-b border-slate-100 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className="w-7 h-7 rounded-full bg-slate-800 flex items-center justify-center text-white text-xs font-medium">
                AI
              </div>
              <span className="font-medium text-slate-700 text-sm">Claude</span>
            </div>
            <button
              onClick={runDemo}
              disabled={isAnimating}
              className="px-3 py-1.5 bg-slate-800 text-white text-xs font-medium rounded-lg disabled:opacity-50 transition-opacity"
            >
              {isAnimating ? 'Running...' : demoPhase > 0 ? 'Replay' : 'Run Demo'}
            </button>
          </div>
          
          <div className="p-4 min-h-[320px]">
            <ChatMessage 
              message="Help me set up React animations for my project" 
              isUser={true}
            />
            
            {demoPhase >= 1 && (
              <div className="mb-3">
                <div className="flex items-start gap-2">
                  <div className="w-5 h-5 rounded-full bg-slate-800 flex items-center justify-center text-white text-xs font-medium flex-shrink-0 mt-0.5">
                    AI
                  </div>
                  <div className="flex-1">
                    <ToolCallStack 
                      tools={tools} 
                      isAnimating={isAnimating} 
                      currentLabel={currentLabel}
                    />
                  </div>
                </div>
              </div>
            )}
            
            {demoPhase >= 2 && (
              <ChatMessage 
                message="Done! I've set up Framer Motion for your project with a custom animated component." 
                isUser={false}
              />
            )}
            
            {demoPhase === 0 && (
              <div className="flex flex-col items-center justify-center py-10 text-center">
                <div className="w-10 h-10 rounded-xl bg-slate-100 flex items-center justify-center mb-3">
                  <svg className="w-5 h-5 text-slate-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <polygon points="5,3 19,12 5,21"/>
                  </svg>
                </div>
                <p className="text-slate-400 text-xs">Tap "Run Demo" to start</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}