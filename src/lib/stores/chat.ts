import { writable, derived, get } from 'svelte/store';

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'error';
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
  activityId: string | null;  // ID of the activity awaiting permission
  tool: string;
  description: string;
  timestamp: number;
}

export interface PlanRequest {
  id: string;
  activityId: string | null;  // ID of the ExitPlanMode activity
  planContent: string;
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
  insertionIndex: number;  // Stable position assigned at creation, never changes
}

export interface ChatState {
  messages: Message[];
  activities: ToolActivity[];
  streamItems: StreamItem[];
  streamItemCounter: number;  // Counter for stable insertion ordering
  pendingPermissions: Record<string, PermissionRequest>;  // Keyed by activityId for parallel tools
  pendingPlan: PlanRequest | null;
  isStreaming: boolean;
  streamingContent: string;
  messageQueue: string[];  // Queue of messages to send after current turn completes
  tasks: TaskItem[];
  activeSkills: string[];
  tokenCount: number;
  sessionDuration: number;
}

const initialState: ChatState = {
  messages: [],
  activities: [],
  streamItems: [],
  streamItemCounter: 0,
  pendingPermissions: {},
  pendingPlan: null,
  isStreaming: false,
  streamingContent: '',
  messageQueue: [],
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
        insertionIndex: s.streamItemCounter,
      };
      return {
        ...s,
        activities: [...s.activities, activity],
        streamItems: [...s.streamItems, streamItem],
        streamItemCounter: s.streamItemCounter + 1,
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

    addPermissionRequest: (request: PermissionRequest) => update(s => {
      // Use activityId as key (or request.id if no activityId)
      const key = request.activityId || request.id;
      return {
        ...s,
        pendingPermissions: {
          ...s.pendingPermissions,
          [key]: request,
        },
      };
    }),

    removePermissionRequest: (activityIdOrRequestId: string) => update(s => {
      const { [activityIdOrRequestId]: removed, ...rest } = s.pendingPermissions;
      return {
        ...s,
        pendingPermissions: rest,
      };
    }),

    clearPermissionRequests: () => update(s => ({
      ...s,
      pendingPermissions: {},
    })),

    setPlanRequest: (request: PlanRequest | null) => update(s => ({
      ...s,
      pendingPlan: request,
    })),

    setStreaming: (isStreaming: boolean) => update(s => ({
      ...s,
      isStreaming,
      streamingContent: isStreaming ? s.streamingContent : '',
      streamItems: isStreaming ? s.streamItems : [],
      streamItemCounter: isStreaming ? s.streamItemCounter : 0,
    })),

    appendStreamingContent: (content: string) => update(s => {
      // Find the last text item in streamItems, or create a new one
      const lastItem = s.streamItems[s.streamItems.length - 1];

      if (lastItem && lastItem.type === 'text') {
        // Append to existing text item (preserve insertionIndex)
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
        // Create new text item with stable insertion index
        const newItem: StreamItem = {
          id: `stream-text-${Date.now()}`,
          type: 'text',
          content,
          timestamp: Date.now() / 1000,
          insertionIndex: s.streamItemCounter,
        };
        return {
          ...s,
          streamingContent: s.streamingContent + content,
          streamItems: [...s.streamItems, newItem],
          streamItemCounter: s.streamItemCounter + 1,
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
      streamItemCounter: 0,
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

    // Message queue management
    enqueueMessage: (message: string) => update(s => ({
      ...s,
      messageQueue: [...s.messageQueue, message],
    })),

    dequeueMessage: (): string | undefined => {
      const state = get({ subscribe });
      const [first, ...rest] = state.messageQueue;
      if (first !== undefined) {
        update(s => ({ ...s, messageQueue: rest }));
      }
      return first;
    },

    clearMessageQueue: () => update(s => ({
      ...s,
      messageQueue: [],
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
        streamItemCounter: 0,
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
export const pendingPermissions = derived(chatStore, $chat => $chat.pendingPermissions);
export const pendingPlan = derived(chatStore, $chat => $chat.pendingPlan);
export const tasks = derived(chatStore, $chat => $chat.tasks);
export const activeSkills = derived(chatStore, $chat => $chat.activeSkills);
export const messageQueue = derived(chatStore, $chat => $chat.messageQueue);
