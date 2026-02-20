// Chat state store using Svelte 5 runes
import { SvelteMap } from 'svelte/reactivity';
import { debug } from '$lib/utils/logger';

export type ToolStatus = 'pending' | 'awaiting_permission' | 'running' | 'completed' | 'error' | 'denied' | 'aborted' | 'history';

export interface ToolState {
  id: string;  // tool_use_id - the canonical identifier
  toolType: string;
  target: string;
  status: ToolStatus;
  permissionRequestId?: string;
  input?: Record<string, unknown>;
  timestamp: number;
  insertionIndex: number;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'error';
  content: string;
  timestamp: number;
  tools?: ToolState[];
}

export interface StreamItem {
  id: string;
  type: 'text' | 'tool';
  content?: string;
  toolId?: string;  // Reference to tool in tools Map
  timestamp: number;
  insertionIndex: number;
}

export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  active: boolean;
}

function ensureUniqueToolIds(tools: ToolState[]): ToolState[] {
  const seen = new Map<string, number>();
  return tools.map((tool) => {
    const count = seen.get(tool.id) ?? 0;
    seen.set(tool.id, count + 1);
    if (count === 0) return tool;
    return { ...tool, id: `${tool.id}__dup_${count}` };
  });
}

class ChatState {
  // Messages (finalized)
  messages = $state<Message[]>([]);

  // Streaming state
  isStreaming = $state(false);
  activeTurnId = $state<string | null>(null);
  streamItems = $state<StreamItem[]>([]);
  private streamItemCounter = $state(0);

  // Tools (keyed by tool_use_id)
  tools = $state(new SvelteMap<string, ToolState>());

  // UI state: tool stack expanded/collapsed by stable stack key.
  // We key stacks by the first tool id in the group so this survives the stream -> finalized remount.
  toolStackExpanded = $state<Record<string, boolean>>({});

  // Message queue (for sending while streaming)
  messageQueue = $state<string[]>([]);

  // Metadata
  todos = $state<TodoItem[]>([]);
  activeSkills = $state<string[]>([]);
  tokenCount = $state(0);
  contextWindow = $state<number | null>(null);
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
    debug(`Streaming: ${streaming}, items=${this.streamItems.length}, tools=${this.tools.size}`);
    this.isStreaming = streaming;
    if (streaming) {
      // Starting stream
    } else {
      // Ending stream - clear streaming state
      this.streamItems = [];
      this.streamItemCounter = 0;
      this.tools = new SvelteMap();
      this.activeTurnId = null;
    }
  }

  setActiveTurnId(turnId: string | null) {
    this.activeTurnId = turnId;
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

  // --- Tool methods ---

  addTool(tool: Omit<ToolState, 'insertionIndex'>) {
    const toolState: ToolState = {
      ...tool,
      insertionIndex: this.streamItemCounter,
    };

    // Add to tools map
    const newTools = new SvelteMap(this.tools);
    newTools.set(tool.id, toolState);
    this.tools = newTools;

    // Add stream item reference
    this.streamItems = [...this.streamItems, {
      id: `stream-tool-${tool.id}`,
      type: 'tool',
      toolId: tool.id,
      timestamp: tool.timestamp,
      insertionIndex: this.streamItemCounter,
    }];
    this.streamItemCounter++;
  }

  updateToolStatus(id: string, status: ToolStatus, extras?: { permissionRequestId?: string | null }) {
    const tool = this.tools.get(id);
    if (!tool) {
      return;
    }

    const newTools = new SvelteMap(this.tools);
    const updatedTool = { ...tool, status };

    // Handle permissionRequestId: set if provided, clear if explicitly null, keep if undefined
    if (extras?.permissionRequestId !== undefined) {
      if (extras.permissionRequestId === null) {
        delete updatedTool.permissionRequestId;
      } else {
        updatedTool.permissionRequestId = extras.permissionRequestId;
      }
    }

    newTools.set(id, updatedTool);
    this.tools = newTools;
  }

  getTool(id: string): ToolState | undefined {
    return this.tools.get(id);
  }

  getToolStackExpanded(key: string): boolean {
    return this.toolStackExpanded[key] ?? false;
  }

  setToolStackExpanded(key: string, expanded: boolean) {
    this.toolStackExpanded = { ...this.toolStackExpanded, [key]: expanded };
  }

  getToolsAwaitingPermission(): ToolState[] {
    return [...this.tools.values()]
      .filter(t => t.status === 'awaiting_permission')
      .sort((a, b) => a.insertionIndex - b.insertionIndex);
  }

  clearPendingPermissions() {
    // Update all awaiting_permission tools to denied
    const newTools = new SvelteMap(this.tools);
    for (const [id, tool] of newTools) {
      if (tool.status === 'awaiting_permission') {
        newTools.set(id, { ...tool, status: 'denied', permissionRequestId: undefined });
      }
    }
    this.tools = newTools;
  }

  // Finalize the current stream - convert streamItems to messages preserving order
  finalize() {
    debug(`Finalize: ${this.streamItems.length} stream items, ${this.tools.size} tools → messages`);
    const newMessages = [...this.messages];

    let currentText = '';
    let currentTools: ToolState[] = [];

    for (const item of this.streamItems) {
      if (item.type === 'text' && item.content) {
        // Flush pending tools before text
        if (currentTools.length > 0) {
          newMessages.push({
            id: crypto.randomUUID(),
            role: 'assistant',
            content: currentText,
            timestamp: Date.now() / 1000,
            tools: currentTools,
          });
          currentText = '';
          currentTools = [];
        }
        currentText += item.content;
      } else if (item.type === 'tool' && item.toolId) {
        const tool = this.tools.get(item.toolId);
        if (tool) {
          // Mark running tools as 'aborted' since they were interrupted
          const finalTool = tool.status === 'running' || tool.status === 'pending' || tool.status === 'awaiting_permission'
            ? { ...tool, status: 'aborted' as const }
            : tool;
          currentTools.push(finalTool);
        }
      }
    }

    // Finalize remaining content
    if (currentText || currentTools.length > 0) {
      newMessages.push({
        id: crypto.randomUUID(),
        role: 'assistant',
        content: currentText,
        timestamp: Date.now() / 1000,
        tools: currentTools.length > 0 ? currentTools : undefined,
      });
    }

    this.messages = newMessages;
    this.isStreaming = false;
    this.activeTurnId = null;
    this.streamItems = [];
    this.streamItemCounter = 0;
    this.tools = new SvelteMap();
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
    this.activeTurnId = null;
    this.streamItems = [];
    this.streamItemCounter = 0;
    this.tools = new SvelteMap();
    this.toolStackExpanded = {};
    this.messageQueue = [];
    this.todos = [];
    this.activeSkills = [];
    this.tokenCount = 0;
    this.contextWindow = null;
    this.sessionDuration = 0;
  }

  // --- History ---

  loadHistory(historyMessages: Array<{ id: string; role: string; content: string; timestamp: number; tools: Array<{ id: string; toolType: string; target: string }> }>) {
    // Merge consecutive assistant messages that have tools but no text content
    const mergedMessages: Message[] = [];
    let toolCounter = 0;

    for (const msg of historyMessages) {
      const tools: ToolState[] = msg.tools.map((tool) => ({
        id: tool.id,
        toolType: tool.toolType,
        target: tool.target,
        status: 'history' as const,
        timestamp: msg.timestamp,
        insertionIndex: toolCounter++,
      }));

      const lastMessage = mergedMessages[mergedMessages.length - 1];
      const canMerge =
        msg.role === 'assistant' &&
        !msg.content.trim() &&
        tools.length > 0 &&
        lastMessage?.role === 'assistant';

      if (canMerge) {
        // Merge tools into previous assistant message
        lastMessage.tools = ensureUniqueToolIds([...(lastMessage.tools || []), ...tools]);
      } else {
        // Create new message
        mergedMessages.push({
          id: msg.id,
          role: msg.role as 'user' | 'assistant',
          content: msg.content,
          timestamp: msg.timestamp,
          tools: tools.length > 0 ? ensureUniqueToolIds(tools) : undefined,
        });
      }
    }

    this.messages = mergedMessages;
  }
}

export const chat = new ChatState();
