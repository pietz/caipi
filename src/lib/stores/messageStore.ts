import { writable, derived, get } from 'svelte/store';
import type { ToolActivity } from './activityStore';

export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'error';
  content: string;
  timestamp: number;
  activities?: ToolActivity[];  // Tool calls associated with this message
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

export interface MessageState {
  messages: Message[];
  streamItems: StreamItem[];
  streamItemCounter: number;  // Counter for stable insertion ordering
  isStreaming: boolean;
  streamingContent: string;
  messageQueue: string[];  // Queue of messages to send after current turn completes
}

const initialState: MessageState = {
  messages: [],
  streamItems: [],
  streamItemCounter: 0,
  isStreaming: false,
  streamingContent: '',
  messageQueue: [],
};

function createMessageStore() {
  const { subscribe, set, update } = writable<MessageState>(initialState);

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

    addStreamItem: (activity: ToolActivity) => update(s => {
      const streamItem: StreamItem = {
        id: `stream-tool-${activity.id}`,
        type: 'tool',
        activity,
        timestamp: activity.timestamp,
        insertionIndex: s.streamItemCounter,
      };
      return {
        ...s,
        streamItems: [...s.streamItems, streamItem],
        streamItemCounter: s.streamItemCounter + 1,
      };
    }),

    updateStreamItemActivity: (id: string, status: ToolActivity['status']) => update(s => ({
      ...s,
      streamItems: s.streamItems.map(item =>
        item.type === 'tool' && item.activity?.id === id
          ? { ...item, activity: { ...item.activity!, status } }
          : item
      ),
    })),

    removeStreamItem: (id: string) => update(s => ({
      ...s,
      streamItems: s.streamItems.filter(item =>
        !(item.type === 'tool' && item.activity?.id === id)
      ),
    })),

    clearStreamingContent: () => update(s => ({
      ...s,
      streamingContent: '',
    })),

    clearStreamItems: () => update(s => ({
      ...s,
      streamItems: [],
      streamItemCounter: 0,
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
          // Mark running tools as 'aborted' since they were interrupted
          const activity = item.activity.status === 'running'
            ? { ...item.activity, status: 'aborted' as const }
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
        streamingContent: '',
        isStreaming: false,
      };
    }),

    reset: () => set(initialState),
  };
}

export const messageStore = createMessageStore();

// Derived stores
export const messages = derived(messageStore, $store => $store.messages);
export const streamItems = derived(messageStore, $store => $store.streamItems);
export const isStreaming = derived(messageStore, $store => $store.isStreaming);
export const messageQueue = derived(messageStore, $store => $store.messageQueue);
