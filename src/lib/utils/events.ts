// Event handling utility for Claude events
import { api } from '$lib/api';
import { chat, type ToolState, type ToolStatus } from '$lib/stores/chat.svelte';
import { app, type PermissionMode, type Model } from '$lib/stores/app.svelte';

export interface ChatEvent {
  type: string;
  content?: string;
  toolUseId?: string;
  toolType?: string;
  target?: string;
  status?: string;
  input?: Record<string, unknown>;
  permissionRequestId?: string;
  id?: string;  // For ToolEnd (legacy field name)
  message?: string;
  authType?: string;
  permissionMode?: string;
  model?: string;
  totalTokens?: number;
  sessionId?: string;
  // Thinking events
  thinkingId?: string;
}

export interface EventHandlerOptions {
  onComplete?: () => void;
  onError?: (message: string) => void;
}

// Buffer for line-by-line text streaming (module-level for persistence across events)
let lineBuffer = '';
let flushTimer: ReturnType<typeof setTimeout> | null = null;
const FLUSH_DELAY_MS = 150;

// Callback to notify when content changes (for scrolling)
let onContentChange: (() => void) | null = null;

export function setOnContentChange(callback: (() => void) | null) {
  onContentChange = callback;
}

export function handleClaudeEvent(event: ChatEvent, options: EventHandlerOptions = {}) {
  const { onComplete, onError } = options;

  switch (event.type) {
    case 'Text':
      handleTextEvent(event);
      break;

    case 'ToolStart':
      handleToolStartEvent(event);
      break;

    case 'ToolStatusUpdate':
      handleToolStatusUpdateEvent(event);
      break;

    case 'ToolEnd':
      handleToolEndEvent(event);
      break;

    case 'Complete':
      handleCompleteEvent(onComplete);
      break;

    case 'AbortComplete':
      handleAbortCompleteEvent();
      break;

    case 'SessionInit':
      handleSessionInitEvent(event);
      break;

    case 'StateChanged':
      handleStateChangedEvent(event);
      break;

    case 'TokenUsage':
      handleTokenUsageEvent(event);
      break;

    case 'Error':
      handleErrorEvent(event, onError);
      break;

    case 'ThinkingStart':
      handleThinkingStartEvent(event);
      break;

    case 'ThinkingEnd':
      // Thinking blocks arrive complete, so ThinkingEnd is a no-op
      break;
  }
}

function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + '...';
}

function handleTextEvent(event: ChatEvent) {
  if (!event.content) return;

  lineBuffer += event.content;

  // Split by newlines, keeping incomplete line in buffer
  const lines = lineBuffer.split('\n');
  lineBuffer = lines.pop() || '';

  // Push complete lines (with newline restored)
  if (lines.length > 0) {
    chat.appendText(lines.join('\n') + '\n');
    onContentChange?.();
  }

  // Schedule a timer-based flush for buffered content without newlines
  // This prevents the UI from appearing "stuck" on long lines
  if (lineBuffer) {
    if (flushTimer) clearTimeout(flushTimer);
    flushTimer = setTimeout(() => {
      if (lineBuffer) {
        chat.appendText(lineBuffer);
        lineBuffer = '';
        onContentChange?.();
      }
      flushTimer = null;
    }, FLUSH_DELAY_MS);
  }
}

function handleToolStartEvent(event: ChatEvent) {
  if (!event.toolUseId || !event.toolType) return;

  // Flush buffered text before tool to preserve ordering
  if (lineBuffer) {
    chat.appendText(lineBuffer);
    lineBuffer = '';
  }

  chat.addTool({
    id: event.toolUseId,
    toolType: event.toolType,
    target: event.target || '',
    status: (event.status as ToolStatus) || 'pending',
    input: event.input,
    timestamp: Date.now() / 1000,
  });
}

function handleToolStatusUpdateEvent(event: ChatEvent) {
  if (!event.toolUseId || !event.status) return;

  // Pass null to clear permissionRequestId when not provided (backend sent None)
  chat.updateToolStatus(
    event.toolUseId,
    event.status as ToolStatus,
    { permissionRequestId: event.permissionRequestId ?? null }
  );
}

function handleToolEndEvent(event: ChatEvent) {
  // ToolEnd uses 'id' field (legacy from PostToolUse hook)
  const toolId = event.id;
  if (!toolId || !event.status) return;

  chat.updateToolStatus(toolId, event.status as ToolStatus);

  // Track skill activation and handle special tools
  if (event.status === 'completed') {
    const tool = chat.getTool(toolId);

    if (tool?.toolType === 'Skill' && tool.target) {
      chat.addActiveSkill(tool.target);
    }

    // Handle TodoWrite - sync entire todo list
    if (tool?.toolType === 'TodoWrite' && tool.input) {
      try {
        const input = tool.input;
        const todosArray = input.todos ?? input.items ?? input.tasks ??
          (Array.isArray(input) ? input : null);

        if (todosArray && Array.isArray(todosArray)) {
          const todos = todosArray.map((todo: Record<string, unknown>) => ({
            id: String(todo.id ?? crypto.randomUUID()),
            text: String(todo.content ?? todo.text ?? todo.subject ?? 'Todo'),
            done: todo.status === 'completed',
            active: todo.status === 'in_progress',
          }));
          chat.setTodos(todos);
        }
      } catch (err) {
        console.error('[TodoWrite] Error processing input:', err);
      }
    }

    // Handle TaskCreate
    if (tool?.toolType === 'TaskCreate' && tool.input) {
      const input = tool.input as { subject?: string };
      chat.addTodo({
        id: tool.id,
        text: input.subject || 'New todo',
        done: false,
        active: true,
      });
    }

    // Handle TaskUpdate
    if (tool?.toolType === 'TaskUpdate' && tool.input) {
      const input = tool.input as { taskId?: string; status?: string; subject?: string };
      if (input.taskId) {
        chat.updateTodo(input.taskId, {
          done: input.status === 'completed',
          active: input.status === 'in_progress',
          ...(input.subject && { text: input.subject }),
        });
      }
    }
  }
}

function handleThinkingStartEvent(event: ChatEvent) {
  if (!event.thinkingId || !event.content) return;

  // Flush buffered text before thinking to preserve ordering
  if (lineBuffer) {
    chat.appendText(lineBuffer);
    lineBuffer = '';
  }

  chat.addTool({
    id: event.thinkingId,
    toolType: 'Thinking',
    target: truncate(event.content, 50),  // Preview of thinking
    status: 'completed',  // Thinking arrives complete
    input: { content: event.content },  // Store full content
    timestamp: Date.now() / 1000,
  });
}

function handleCompleteEvent(onComplete?: () => void) {
  // Flush any remaining buffered text
  if (lineBuffer) {
    chat.appendText(lineBuffer);
    lineBuffer = '';
  }
  chat.finalize();
  onComplete?.();
}

function handleAbortCompleteEvent() {
  // Flush any remaining buffered text
  if (lineBuffer) {
    chat.appendText(lineBuffer);
    lineBuffer = '';
  }
  chat.finalize();
  chat.setStreaming(false);
  chat.clearMessageQueue();
  chat.clearPendingPermissions();
}

function handleSessionInitEvent(event: ChatEvent) {
  if (event.authType) {
    app.setAuthType(event.authType);
  }
}

function handleStateChangedEvent(event: ChatEvent) {
  if (event.permissionMode && event.model) {
    app.syncState(
      event.permissionMode as PermissionMode,
      event.model as Model
    );
  }
}

function handleTokenUsageEvent(event: ChatEvent) {
  if (event.totalTokens !== undefined) {
    chat.tokenCount = event.totalTokens;
  }
}

function handleErrorEvent(event: ChatEvent, onError?: (message: string) => void) {
  // Flush buffered text and finalize before clearing
  if (lineBuffer) {
    chat.appendText(lineBuffer);
    lineBuffer = '';
  }
  chat.finalize();

  chat.addErrorMessage(event.message || 'An unknown error occurred');
  onError?.(event.message || 'Unknown error');
}

// Permission response helper
export async function respondToPermission(
  sessionId: string,
  tool: ToolState,
  allowed: boolean
): Promise<void> {
  if (!tool.permissionRequestId) {
    console.error('No permission request ID for tool:', tool.id);
    return;
  }

  try {
    await api.respondPermission(sessionId, tool.permissionRequestId, allowed);
    // Status will be updated via ToolStatusUpdate event from backend
  } catch (e) {
    console.error('Failed to respond to permission:', e);
  }
}

// Reset event state (for session reset)
export function resetEventState() {
  lineBuffer = '';
  if (flushTimer) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
}
