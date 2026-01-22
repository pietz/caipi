import { writable, derived, get } from 'svelte/store';

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
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

export interface ChatState {
  messages: Message[];
  activities: ToolActivity[];
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

    addActivity: (activity: ToolActivity) => update(s => ({
      ...s,
      activities: [...s.activities, activity],
    })),

    updateActivityStatus: (id: string, status: ToolActivity['status']) => update(s => ({
      ...s,
      activities: s.activities.map(a =>
        a.id === id ? { ...a, status } : a
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
    })),

    appendStreamingContent: (content: string) => update(s => ({
      ...s,
      streamingContent: s.streamingContent + content,
    })),

    clearStreamingContent: () => update(s => ({
      ...s,
      streamingContent: '',
    })),

    clearActivities: () => update(s => ({
      ...s,
      activities: [],
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

    reset: () => set(initialState),
  };
}

export const chatStore = createChatStore();

// Derived stores
export const messages = derived(chatStore, $chat => $chat.messages);
export const activities = derived(chatStore, $chat => $chat.activities);
export const isStreaming = derived(chatStore, $chat => $chat.isStreaming);
export const pendingPermission = derived(chatStore, $chat => $chat.pendingPermission);
export const tasks = derived(chatStore, $chat => $chat.tasks);
export const activeSkills = derived(chatStore, $chat => $chat.activeSkills);
