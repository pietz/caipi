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

export interface ChatState {
  messages: Message[];
  activities: ToolActivity[];
  pendingPermission: PermissionRequest | null;
  isStreaming: boolean;
  streamingContent: string;
}

const initialState: ChatState = {
  messages: [],
  activities: [],
  pendingPermission: null,
  isStreaming: false,
  streamingContent: '',
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

    reset: () => set(initialState),
  };
}

export const chatStore = createChatStore();

// Derived stores
export const messages = derived(chatStore, $chat => $chat.messages);
export const activities = derived(chatStore, $chat => $chat.activities);
export const isStreaming = derived(chatStore, $chat => $chat.isStreaming);
export const pendingPermission = derived(chatStore, $chat => $chat.pendingPermission);
