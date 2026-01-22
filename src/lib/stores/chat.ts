import { writable, derived, get } from 'svelte/store';

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  activities?: ToolActivity[];  // Tool calls associated with this message
}

export interface ToolActivity {
  id: string;
  toolType: string;
  target: string;
  status: 'running' | 'completed' | 'error';
  timestamp: number;
}

export interface PermissionRequest {
  id: string;
  tool: string;
  description: string;
  timestamp: number;
}

export interface TaskItem {
  id: string;
  text: string;
  done: boolean;
  active: boolean;
}

// StreamItem for unified timeline during streaming
export interface StreamItem {
  id: string;
  type: 'text' | 'tool';
  content?: string;
  activity?: ToolActivity;
  timestamp: number;
}

export interface ChatState {
  messages: Message[];
  activities: ToolActivity[];
  streamItems: StreamItem[];
  pendingPermission: PermissionRequest | null;
  isStreaming: boolean;
  streamingContent: string;
  tasks: TaskItem[];
  activeSkills: string[];
  tokenCount: number;
  sessionDuration: number;
}

const initialState: ChatState = {
  messages: [],
  activities: [],
  streamItems: [],
  pendingPermission: null,
  isStreaming: false,
  streamingContent: '',
  tasks: [],
  activeSkills: [],
  tokenCount: 0,
  sessionDuration: 0,
};

function createChatStore() {
  const { subscribe, set, update } = writable<ChatState>(initialState);

  return {
    subscribe,

    addMessage: (message: Message) => update(s => ({
      ...s,
      messages: [...s.messages, message],
    })),

    updateMessage: (id: string, content: string) => update(s => ({
      ...s,
      messages: s.messages.map(m =>
        m.id === id ? { ...m, content } : m
      ),
    })),

    addActivity: (activity: ToolActivity) => update(s => {
      // Add to both activities (for backward compat) and streamItems (for timeline)
      const streamItem: StreamItem = {
        id: `stream-tool-${activity.id}`,
        type: 'tool',
        activity,
        timestamp: activity.timestamp,
      };
      return {
        ...s,
        activities: [...s.activities, activity],
        streamItems: [...s.streamItems, streamItem],
      };
    }),

    updateActivityStatus: (id: string, status: ToolActivity['status']) => update(s => ({
      ...s,
      activities: s.activities.map(a =>
        a.id === id ? { ...a, status } : a
      ),
      streamItems: s.streamItems.map(item =>
        item.type === 'tool' && item.activity?.id === id
          ? { ...item, activity: { ...item.activity!, status } }
          : item
      ),
    })),

    setPermissionRequest: (request: PermissionRequest | null) => update(s => ({
      ...s,
      pendingPermission: request,
    })),

    setStreaming: (isStreaming: boolean) => update(s => ({
      ...s,
      isStreaming,
      streamingContent: isStreaming ? s.streamingContent : '',
      streamItems: isStreaming ? s.streamItems : [],
    })),

    appendStreamingContent: (content: string) => update(s => {
      // Find the last text item in streamItems, or create a new one
      const lastItem = s.streamItems[s.streamItems.length - 1];

      if (lastItem && lastItem.type === 'text') {
        // Append to existing text item
        return {
          ...s,
          streamingContent: s.streamingContent + content,
          streamItems: s.streamItems.map((item, i) =>
            i === s.streamItems.length - 1
              ? { ...item, content: (item.content || '') + content }
              : item
          ),
        };
      } else {
        // Create new text item
        const newItem: StreamItem = {
          id: `stream-text-${Date.now()}`,
          type: 'text',
          content,
          timestamp: Date.now() / 1000,
        };
        return {
          ...s,
          streamingContent: s.streamingContent + content,
          streamItems: [...s.streamItems, newItem],
        };
      }
    }),

    clearStreamingContent: () => update(s => ({
      ...s,
      streamingContent: '',
    })),

    clearActivities: () => update(s => ({
      ...s,
      activities: [],
    })),

    removeActivity: (id: string) => update(s => ({
      ...s,
      activities: s.activities.filter(a => a.id !== id),
      streamItems: s.streamItems.filter(item =>
        !(item.type === 'tool' && item.activity?.id === id)
      ),
    })),

    clearStreamItems: () => update(s => ({
      ...s,
      streamItems: [],
    })),

    // Task management
    setTasks: (tasks: TaskItem[]) => update(s => ({
      ...s,
      tasks,
    })),

    addTask: (task: TaskItem) => update(s => ({
      ...s,
      tasks: [...s.tasks, task],
    })),

    updateTask: (id: string, updates: Partial<TaskItem>) => update(s => ({
      ...s,
      tasks: s.tasks.map(t =>
        t.id === id ? { ...t, ...updates } : t
      ),
    })),

    // Skills management
    setActiveSkills: (skills: string[]) => update(s => ({
      ...s,
      activeSkills: skills,
    })),

    // Stats
    setTokenCount: (count: number) => update(s => ({
      ...s,
      tokenCount: count,
    })),

    setSessionDuration: (duration: number) => update(s => ({
      ...s,
      sessionDuration: duration,
    })),

    // Finalize the current stream - convert streamItems to messages preserving order
    finalizeStream: () => update(s => {
      const newMessages = [...s.messages];

      // Group consecutive items: text segments become messages, tools get attached to preceding text
      let currentText = '';
      let currentActivities: ToolActivity[] = [];

      for (const item of s.streamItems) {
        if (item.type === 'text' && item.content) {
          // If we have pending activities, finalize previous message first
          if (currentActivities.length > 0 && currentText) {
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
          // Ensure completed status if still running (timing safeguard)
          const activity = item.activity.status === 'running'
            ? { ...item.activity, status: 'completed' as const }
            : item.activity;
          currentActivities.push(activity);
        }
      }

      // Finalize any remaining content
      if (currentText || currentActivities.length > 0) {
        newMessages.push({
          id: crypto.randomUUID(),
          role: 'assistant',
          content: currentText,
          timestamp: Date.now() / 1000,
          activities: currentActivities.length > 0 ? currentActivities : undefined,
        });
      }

      return {
        ...s,
        messages: newMessages,
        streamItems: [],
        activities: [],
        streamingContent: '',
        isStreaming: false,
      };
    }),

    reset: () => set(initialState),
  };
}

export const chatStore = createChatStore();

// Derived stores
export const messages = derived(chatStore, $chat => $chat.messages);
export const activities = derived(chatStore, $chat => $chat.activities);
export const streamItems = derived(chatStore, $chat => $chat.streamItems);
export const isStreaming = derived(chatStore, $chat => $chat.isStreaming);
export const pendingPermission = derived(chatStore, $chat => $chat.pendingPermission);
export const tasks = derived(chatStore, $chat => $chat.tasks);
export const activeSkills = derived(chatStore, $chat => $chat.activeSkills);
