// Chat state store using Svelte 5 runes

export interface ToolActivity {
  id: string;
  toolType: string;
  target: string;
  status: 'running' | 'completed' | 'error' | 'aborted';
  timestamp: number;
  input?: Record<string, unknown>;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'error';
  content: string;
  timestamp: number;
  activities?: ToolActivity[];
}

export interface StreamItem {
  id: string;
  type: 'text' | 'tool';
  content?: string;
  activity?: ToolActivity;
  timestamp: number;
  insertionIndex: number;
}

export interface PermissionRequest {
  id: string;
  activityId: string | null;
  tool: string;
  description: string;
  timestamp: number;
}

export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  active: boolean;
}

class ChatState {
  // Messages (finalized)
  messages = $state<Message[]>([]);

  // Streaming state
  isStreaming = $state(false);
  streamItems = $state<StreamItem[]>([]);
  private streamItemCounter = $state(0);

  // Permissions (keyed by activityId)
  pendingPermissions = $state<Record<string, PermissionRequest>>({});

  // Message queue (for sending while streaming)
  messageQueue = $state<string[]>([]);

  // Metadata
  todos = $state<TodoItem[]>([]);
  activeSkills = $state<string[]>([]);
  tokenCount = $state(0);
  sessionDuration = $state(0);

  // --- Message methods ---

  addMessage(message: Message) {
    this.messages = [...this.messages, message];
  }

  addUserMessage(content: string) {
    this.addMessage({
      id: crypto.randomUUID(),
      role: 'user',
      content,
      timestamp: Date.now() / 1000,
    });
  }

  addErrorMessage(content: string) {
    this.addMessage({
      id: crypto.randomUUID(),
      role: 'error',
      content,
      timestamp: Date.now() / 1000,
    });
  }

  // --- Streaming methods ---

  setStreaming(streaming: boolean) {
    this.isStreaming = streaming;
    if (streaming) {
      // Starting stream
    } else {
      // Ending stream - clear streaming state
      this.streamItems = [];
      this.streamItemCounter = 0;
    }
  }

  appendText(content: string) {
    const lastItem = this.streamItems[this.streamItems.length - 1];

    if (lastItem && lastItem.type === 'text') {
      // Append to existing text item
      this.streamItems = this.streamItems.map((item, i) =>
        i === this.streamItems.length - 1
          ? { ...item, content: (item.content || '') + content }
          : item
      );
    } else {
      // Create new text item
      this.streamItems = [...this.streamItems, {
        id: `stream-text-${Date.now()}`,
        type: 'text',
        content,
        timestamp: Date.now() / 1000,
        insertionIndex: this.streamItemCounter,
      }];
      this.streamItemCounter++;
    }
  }

  addActivity(activity: ToolActivity) {
    const streamItem: StreamItem = {
      id: `stream-tool-${activity.id}`,
      type: 'tool',
      activity,
      timestamp: activity.timestamp,
      insertionIndex: this.streamItemCounter,
    };
    this.streamItems = [...this.streamItems, streamItem];
    this.streamItemCounter++;
  }

  updateActivityStatus(id: string, status: ToolActivity['status']) {
    this.streamItems = this.streamItems.map(item =>
      item.type === 'tool' && item.activity?.id === id
        ? { ...item, activity: { ...item.activity!, status } }
        : item
    );
  }

  getActivities(): ToolActivity[] {
    return this.streamItems
      .filter(item => item.type === 'tool' && item.activity)
      .map(item => item.activity!);
  }

  // Finalize the current stream - convert streamItems to messages preserving order
  finalize() {
    const newMessages = [...this.messages];

    let currentText = '';
    let currentActivities: ToolActivity[] = [];

    for (const item of this.streamItems) {
      if (item.type === 'text' && item.content) {
        // Flush pending activities before text
        if (currentActivities.length > 0) {
          newMessages.push({
            id: crypto.randomUUID(),
            role: 'assistant',
            content: currentText,
            timestamp: Date.now() / 1000,
            activities: currentActivities,
          });
          currentText = '';
          currentActivities = [];
        }
        currentText += item.content;
      } else if (item.type === 'tool' && item.activity) {
        // Mark running tools as 'aborted' since they were interrupted
        const activity = item.activity.status === 'running'
          ? { ...item.activity, status: 'aborted' as const }
          : item.activity;
        currentActivities.push(activity);
      }
    }

    // Finalize remaining content
    if (currentText || currentActivities.length > 0) {
      newMessages.push({
        id: crypto.randomUUID(),
        role: 'assistant',
        content: currentText,
        timestamp: Date.now() / 1000,
        activities: currentActivities.length > 0 ? currentActivities : undefined,
      });
    }

    this.messages = newMessages;
    this.isStreaming = false;
    this.streamItems = [];
    this.streamItemCounter = 0;
  }

  // --- Permission methods ---

  addPermissionRequest(request: PermissionRequest) {
    const key = request.activityId || request.id;
    this.pendingPermissions = { ...this.pendingPermissions, [key]: request };
  }

  removePermissionRequest(key: string) {
    const { [key]: _, ...rest } = this.pendingPermissions;
    this.pendingPermissions = rest;
  }

  clearPermissionRequests() {
    this.pendingPermissions = {};
  }

  // --- Queue methods ---

  enqueueMessage(message: string) {
    this.messageQueue = [...this.messageQueue, message];
  }

  dequeueMessage(): string | undefined {
    const [first, ...rest] = this.messageQueue;
    if (first !== undefined) {
      this.messageQueue = rest;
    }
    return first;
  }

  clearMessageQueue() {
    this.messageQueue = [];
  }

  // --- Metadata methods ---

  setTodos(todos: TodoItem[]) {
    this.todos = todos;
  }

  addTodo(todo: TodoItem) {
    this.todos = [...this.todos, todo];
  }

  updateTodo(id: string, updates: Partial<TodoItem>) {
    this.todos = this.todos.map(t =>
      t.id === id ? { ...t, ...updates } : t
    );
  }

  addActiveSkill(skill: string) {
    if (!this.activeSkills.includes(skill)) {
      this.activeSkills = [...this.activeSkills, skill];
    }
  }

  // --- Reset ---

  reset() {
    this.messages = [];
    this.isStreaming = false;
    this.streamItems = [];
    this.streamItemCounter = 0;
    this.pendingPermissions = {};
    this.messageQueue = [];
    this.todos = [];
    this.activeSkills = [];
    this.tokenCount = 0;
    this.sessionDuration = 0;
  }
}

export const chat = new ChatState();
