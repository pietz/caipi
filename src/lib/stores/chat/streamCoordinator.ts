/**
 * Stream Coordinator
 *
 * Handles Claude event stream processing, coordinating between the chat store
 * and app store as events arrive from the backend.
 */

import {
  chatStore,
  appStore,
  type ToolActivity,
  type PermissionRequest,
  type PermissionMode,
  type ModelType,
} from '$lib/stores';

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
  planContent?: string;
  permissionMode?: string;
  model?: string;
}

export interface StreamCoordinatorOptions {
  onComplete?: () => void;
  onError?: (message: string) => void;
}

/**
 * Create a stream coordinator for handling Claude events
 */
export function createStreamCoordinator(options: StreamCoordinatorOptions = {}) {
  const { onComplete, onError } = options;

  // Buffer for line-by-line text streaming
  let lineBuffer = '';

  /**
   * Handle a Claude event from the backend
   */
  function handleEvent(event: ChatEvent): void {
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
        handleCompleteEvent();
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

      case 'Error':
        handleErrorEvent(event);
        break;
    }
  }

  function handleTextEvent(event: ChatEvent): void {
    if (!event.content) return;

    lineBuffer += event.content;

    // Split by newlines, keeping incomplete line in buffer
    const lines = lineBuffer.split('\n');
    lineBuffer = lines.pop() || '';

    // Push complete lines (with newline restored)
    if (lines.length > 0) {
      chatStore.appendStreamingContent(lines.join('\n') + '\n');
    }
  }

  function handleToolStartEvent(event: ChatEvent): void {
    if (!event.activity) return;

    // Flush buffered text before tool to preserve ordering
    if (lineBuffer) {
      chatStore.appendStreamingContent(lineBuffer);
      lineBuffer = '';
    }

    const newActivity = {
      ...event.activity,
      toolType: event.activity.toolType,
      status: 'running' as const,
    };

    chatStore.addActivity(newActivity);

    // If there's a pending permission request with matching tool type but no activityId,
    // link it to this activity (handles case where permission request arrives before ToolStart)
    const pendingPermissions = chatStore.getPendingPermissions();
    for (const [key, permission] of Object.entries(pendingPermissions) as [string, PermissionRequest][]) {
      if (permission.activityId === null && permission.tool === newActivity.toolType) {
        chatStore.removePermissionRequest(key);
        chatStore.addPermissionRequest({
          ...permission,
          activityId: newActivity.id,
        });
        break; // Only update one permission per ToolStart
      }
    }
  }

  function handleToolEndEvent(event: ChatEvent): void {
    if (event.id && event.status) {
      chatStore.updateActivityStatus(
        event.id,
        event.status as ToolActivity['status']
      );

      // Track skill activation only after successful completion (user approved)
      if (event.status === 'completed') {
        const activities = chatStore.getActivities();
        const activity = activities.find((a: ToolActivity) => a.id === event.id);
        if (activity?.toolType === 'Skill' && activity.target) {
          chatStore.addActiveSkill(activity.target);
        }

        // Handle TodoWrite - sync entire todo list
        if (activity?.toolType === 'TodoWrite' && activity.input) {
          try {
            const input = activity.input as Record<string, unknown>;

            // Try different possible field names for the todos array
            const todosArray = input.todos ?? input.items ?? input.tasks ??
              (Array.isArray(input) ? input : null);

            if (todosArray && Array.isArray(todosArray)) {
              const todos = todosArray.map((todo: Record<string, unknown>) => ({
                id: String(todo.id ?? crypto.randomUUID()),
                text: String(todo.content ?? todo.text ?? todo.subject ?? 'Todo'),
                done: todo.status === 'completed',
                active: todo.status === 'in_progress',
              }));
              chatStore.setTodos(todos);
            }
          } catch (err) {
            console.error('[TodoWrite] Error processing input:', err);
          }
        }

        // Handle TaskCreate - add todo to list (legacy/alternative tool)
        if (activity?.toolType === 'TaskCreate' && activity.input) {
          const input = activity.input as { subject?: string; description?: string; activeForm?: string };
          chatStore.addTodo({
            id: activity.id,
            text: input.subject || 'New todo',
            done: false,
            active: true,
          });
        }

        // Handle TaskUpdate - update existing todo (legacy/alternative tool)
        if (activity?.toolType === 'TaskUpdate' && activity.input) {
          const input = activity.input as { taskId?: string; status?: string; subject?: string };
          if (input.taskId) {
            chatStore.updateTodo(input.taskId, {
              done: input.status === 'completed',
              active: input.status === 'in_progress',
              ...(input.subject && { text: input.subject }),
            });
          }
        }
      }
    }
  }

  function handlePermissionRequestEvent(event: ChatEvent): void {
    if (!event.id || !event.tool || !event.description) return;

    // Use toolUseId for exact matching when available (handles parallel tools)
    // Fall back to finding by tool type for backwards compatibility
    let matchingActivityId: string | null = null;

    const activities = chatStore.getActivities();
    const pendingPermissions = chatStore.getPendingPermissions();

    if (event.toolUseId) {
      // Exact match by tool_use_id
      const exactMatch = activities.find((a: ToolActivity) => a.id === event.toolUseId);
      matchingActivityId = exactMatch?.id || null;
    }

    if (!matchingActivityId) {
      // Fallback: find by tool type that doesn't already have a pending permission
      const matchingActivity = activities.find(
        (a: ToolActivity) => a.status === 'running' && a.toolType === event.tool && !pendingPermissions[a.id]
      );
      matchingActivityId = matchingActivity?.id || null;
    }

    chatStore.addPermissionRequest({
      id: event.id,
      activityId: matchingActivityId,
      tool: event.tool,
      description: event.description,
      timestamp: Date.now() / 1000,
    });
  }

  function handleCompleteEvent(): void {
    // Flush any remaining buffered text
    if (lineBuffer) {
      chatStore.appendStreamingContent(lineBuffer);
      lineBuffer = '';
    }
    chatStore.finalizeStream();
    onComplete?.();
  }

  function handleAbortCompleteEvent(): void {
    // Flush any remaining buffered text
    if (lineBuffer) {
      chatStore.appendStreamingContent(lineBuffer);
      lineBuffer = '';
    }
    chatStore.finalizeStream();
    chatStore.setStreaming(false);
    chatStore.clearMessageQueue();
    chatStore.clearPermissionRequests();
  }

  function handleSessionInitEvent(event: ChatEvent): void {
    if (event.authType) {
      appStore.setAuthType(event.authType);
    }
  }

  function handleStateChangedEvent(event: ChatEvent): void {
    if (event.permissionMode && event.model) {
      appStore.syncState(
        event.permissionMode as PermissionMode,
        event.model as ModelType
      );
    }
  }

  function handleErrorEvent(event: ChatEvent): void {
    console.error('Claude error:', event.message);
    chatStore.addMessage({
      id: crypto.randomUUID(),
      role: 'error',
      content: event.message || 'An unknown error occurred',
      timestamp: Date.now() / 1000,
    });
    chatStore.setStreaming(false);
    onError?.(event.message || 'Unknown error');
  }

  return {
    handleEvent,
  };
}
