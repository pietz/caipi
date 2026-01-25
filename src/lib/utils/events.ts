// Event handling utility for Claude events
import { invoke } from '@tauri-apps/api/core';
import { chat, type ToolActivity, type PermissionRequest } from '$lib/stores/chat.svelte';
import { app, type PermissionMode, type Model } from '$lib/stores/app.svelte';

export interface ChatEvent {
  type: string;
  content?: string;
  activity?: ToolActivity;
  id?: string;
  status?: string;
  tool?: string;
  toolUseId?: string;
  description?: string;
  message?: string;
  authType?: string;
  permissionMode?: string;
  model?: string;
  totalTokens?: number;
}

export interface EventHandlerOptions {
  onComplete?: () => void;
  onError?: (message: string) => void;
}

// Buffer for line-by-line text streaming (module-level for persistence across events)
let lineBuffer = '';
let flushTimer: ReturnType<typeof setTimeout> | null = null;
const FLUSH_DELAY_MS = 150;

export function handleClaudeEvent(event: ChatEvent, options: EventHandlerOptions = {}) {
  const { onComplete, onError } = options;

  switch (event.type) {
    case 'Text':
      handleTextEvent(event);
      break;

    case 'ToolStart':
      handleToolStartEvent(event);
      break;

    case 'ToolEnd':
      handleToolEndEvent(event);
      break;

    case 'PermissionRequest':
      handlePermissionRequestEvent(event);
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
  }
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
  }

  // Schedule a timer-based flush for buffered content without newlines
  // This prevents the UI from appearing "stuck" on long lines
  if (lineBuffer) {
    if (flushTimer) clearTimeout(flushTimer);
    flushTimer = setTimeout(() => {
      if (lineBuffer) {
        chat.appendText(lineBuffer);
        lineBuffer = '';
      }
      flushTimer = null;
    }, FLUSH_DELAY_MS);
  }
}

function handleToolStartEvent(event: ChatEvent) {
  if (!event.activity) return;

  // Flush buffered text before tool to preserve ordering
  if (lineBuffer) {
    chat.appendText(lineBuffer);
    lineBuffer = '';
  }

  const newActivity: ToolActivity = {
    ...event.activity,
    status: 'running',
  };

  chat.addActivity(newActivity);

  // Link unmatched permission requests to this activity
  const pendingPermissions = chat.pendingPermissions;
  for (const [key, permission] of Object.entries(pendingPermissions)) {
    if (permission.activityId === null && permission.tool === newActivity.toolType) {
      chat.removePermissionRequest(key);
      chat.addPermissionRequest({
        ...permission,
        activityId: newActivity.id,
      });
      break;
    }
  }
}

function handleToolEndEvent(event: ChatEvent) {
  if (!event.id || !event.status) return;

  chat.updateActivityStatus(event.id, event.status as ToolActivity['status']);

  // Track skill activation and handle special tools
  if (event.status === 'completed') {
    const activities = chat.getActivities();
    const activity = activities.find(a => a.id === event.id);

    if (activity?.toolType === 'Skill' && activity.target) {
      chat.addActiveSkill(activity.target);
    }

    // Handle TodoWrite - sync entire todo list
    if (activity?.toolType === 'TodoWrite' && activity.input) {
      try {
        const input = activity.input;
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
    if (activity?.toolType === 'TaskCreate' && activity.input) {
      const input = activity.input as { subject?: string };
      chat.addTodo({
        id: activity.id,
        text: input.subject || 'New todo',
        done: false,
        active: true,
      });
    }

    // Handle TaskUpdate
    if (activity?.toolType === 'TaskUpdate' && activity.input) {
      const input = activity.input as { taskId?: string; status?: string; subject?: string };
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

function handlePermissionRequestEvent(event: ChatEvent) {
  if (!event.id || !event.tool || !event.description) return;

  let matchingActivityId: string | null = null;
  const activities = chat.getActivities();
  const pendingPermissions = chat.pendingPermissions;

  if (event.toolUseId) {
    const exactMatch = activities.find(a => a.id === event.toolUseId);
    matchingActivityId = exactMatch?.id || null;
  }

  if (!matchingActivityId) {
    const matchingActivity = activities.find(
      a => a.status === 'running' && a.toolType === event.tool && !pendingPermissions[a.id]
    );
    matchingActivityId = matchingActivity?.id || null;
  }

  chat.addPermissionRequest({
    id: event.id,
    activityId: matchingActivityId,
    tool: event.tool,
    description: event.description,
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
  chat.clearPermissionRequests();
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
  console.error('Claude error:', event.message);
  chat.addErrorMessage(event.message || 'An unknown error occurred');
  chat.setStreaming(false);
  onError?.(event.message || 'Unknown error');
}

// Permission response helper
export async function respondToPermission(
  sessionId: string,
  permission: PermissionRequest,
  allowed: boolean
): Promise<void> {
  const key = permission.activityId || permission.id;

  try {
    await invoke('respond_permission', {
      sessionId,
      requestId: permission.id,
      allowed,
    });
    // Only remove permission request on success
    chat.removePermissionRequest(key);
  } catch (e) {
    console.error('Failed to respond to permission:', e);
    // Keep the permission request pending so user can retry
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
