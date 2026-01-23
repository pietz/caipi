import { writable, derived, get } from 'svelte/store';
import { messageStore, type Message } from './messageStore';
import { activityStore, type ToolActivity } from './activityStore';
import { permissionStore, type PermissionRequest } from './permissionStore';

// Re-export types
export type { Message, ToolActivity, PermissionRequest };
export type { StreamItem } from './messageStore';

// Todo management (keeping in chat.ts for now)
export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  active: boolean;
}

// Stats and metadata
export interface ChatMetadata {
  todos: TodoItem[];
  activeSkills: string[];
  tokenCount: number;
  sessionDuration: number;
}

const initialMetadata: ChatMetadata = {
  todos: [],
  activeSkills: [],
  tokenCount: 0,
  sessionDuration: 0,
};

function createChatMetadataStore() {
  const { subscribe, set, update } = writable<ChatMetadata>(initialMetadata);

  return {
    subscribe,

    // Todo management
    setTodos: (todos: TodoItem[]) => update(s => ({
      ...s,
      todos,
    })),

    addTodo: (todo: TodoItem) => update(s => ({
      ...s,
      todos: [...s.todos, todo],
    })),

    updateTodo: (id: string, updates: Partial<TodoItem>) => update(s => ({
      ...s,
      todos: s.todos.map(t =>
        t.id === id ? { ...t, ...updates } : t
      ),
    })),

    // Skills management
    setActiveSkills: (skills: string[]) => update(s => ({
      ...s,
      activeSkills: skills,
    })),

    addActiveSkill: (skill: string) => update(s => ({
      ...s,
      activeSkills: s.activeSkills.includes(skill)
        ? s.activeSkills
        : [...s.activeSkills, skill],
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

    reset: () => set(initialMetadata),
  };
}

const metadataStore = createChatMetadataStore();

// Combined store for backward compatibility - created once at module level
const combinedStore = derived(
  [messageStore, activityStore, permissionStore, metadataStore],
  ([$messages, $activities, $permissions, $metadata]) => ({
    ...$messages,
    ...$activities,
    ...$permissions,
    ...$metadata,
  })
);

// Unified chat store interface for backward compatibility
export const chatStore = {
  subscribe: combinedStore.subscribe,

  // Message operations (delegated to messageStore)
  addMessage: messageStore.addMessage,
  updateMessage: messageStore.updateMessage,
  setStreaming: messageStore.setStreaming,
  appendStreamingContent: messageStore.appendStreamingContent,
  clearStreamingContent: messageStore.clearStreamingContent,
  clearStreamItems: messageStore.clearStreamItems,
  enqueueMessage: messageStore.enqueueMessage,
  dequeueMessage: messageStore.dequeueMessage,
  clearMessageQueue: messageStore.clearMessageQueue,

  // Activity operations (delegated to activityStore)
  addActivity: (activity: ToolActivity) => {
    activityStore.addActivity(activity);
    messageStore.addStreamItem(activity);
  },
  updateActivityStatus: (id: string, status: ToolActivity['status']) => {
    activityStore.updateActivityStatus(id, status);
    messageStore.updateStreamItemActivity(id, status);
  },
  removeActivity: (id: string) => {
    activityStore.removeActivity(id);
    messageStore.removeStreamItem(id);
  },
  clearActivities: activityStore.clearActivities,

  // Permission operations (delegated to permissionStore)
  addPermissionRequest: permissionStore.addPermissionRequest,
  removePermissionRequest: permissionStore.removePermissionRequest,
  clearPermissionRequests: permissionStore.clearPermissionRequests,

  // Getter methods for accessing current state without subscribing
  getActivities: () => get(activityStore).activities,
  getPendingPermissions: () => get(permissionStore).pendingPermissions,
  getStreamItems: () => get(messageStore).streamItems,

  // Finalize stream (coordinated across stores)
  finalizeStream: () => {
    messageStore.finalizeStream();
    activityStore.clearActivities();
  },

  // Metadata operations
  setTodos: metadataStore.setTodos,
  addTodo: metadataStore.addTodo,
  updateTodo: metadataStore.updateTodo,
  setActiveSkills: metadataStore.setActiveSkills,
  addActiveSkill: metadataStore.addActiveSkill,
  setTokenCount: metadataStore.setTokenCount,
  setSessionDuration: metadataStore.setSessionDuration,

  // Reset all stores
  reset: () => {
    messageStore.reset();
    activityStore.reset();
    permissionStore.reset();
    metadataStore.reset();
  },
};

// Re-export derived stores from individual stores
export { messages, streamItems, isStreaming, messageQueue } from './messageStore';
export { activities } from './activityStore';
export { pendingPermissions } from './permissionStore';

// Export todos and skills as derived stores
export const todos = derived(metadataStore, $store => $store.todos);
export const activeSkills = derived(metadataStore, $store => $store.activeSkills);

// Export individual stores for direct access if needed
export { messageStore } from './messageStore';
export { activityStore } from './activityStore';
export { permissionStore } from './permissionStore';
