import { describe, it, expect, beforeEach } from 'vitest';
import { chatStore } from './chat';
import { get } from 'svelte/store';

describe('chatStore', () => {
  beforeEach(() => {
    chatStore.reset();
  });

  it('should add a message', () => {
    const message = {
      id: '1',
      role: 'user' as const,
      content: 'Hello',
      timestamp: Date.now() / 1000,
    };

    chatStore.addMessage(message);
    const state = get(chatStore);

    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]).toEqual(message);
  });

  it('should set streaming state', () => {
    chatStore.setStreaming(true);
    expect(get(chatStore).isStreaming).toBe(true);

    chatStore.setStreaming(false);
    expect(get(chatStore).isStreaming).toBe(false);
  });

  it('should append streaming content', () => {
    chatStore.setStreaming(true);
    chatStore.appendStreamingContent('Hello');
    chatStore.appendStreamingContent(' World');

    const state = get(chatStore);
    expect(state.streamingContent).toBe('Hello World');
    expect(state.streamItems).toHaveLength(1);
    expect(state.streamItems[0].content).toBe('Hello World');
  });

  it('should manage message queue', () => {
    chatStore.enqueueMessage('First');
    chatStore.enqueueMessage('Second');

    expect(get(chatStore).messageQueue).toEqual(['First', 'Second']);

    const first = chatStore.dequeueMessage();
    expect(first).toBe('First');
    expect(get(chatStore).messageQueue).toEqual(['Second']);

    chatStore.clearMessageQueue();
    expect(get(chatStore).messageQueue).toEqual([]);
  });

  it('should add and update activities', () => {
    const activity = {
      id: 'tool-1',
      toolType: 'Read',
      target: 'test.ts',
      status: 'running' as const,
      timestamp: Date.now() / 1000,
    };

    chatStore.addActivity(activity);
    expect(get(chatStore).activities).toHaveLength(1);
    expect(get(chatStore).streamItems).toHaveLength(1);

    chatStore.updateActivityStatus('tool-1', 'completed');
    const state = get(chatStore);
    expect(state.activities[0].status).toBe('completed');
    expect(state.streamItems[0].activity?.status).toBe('completed');
  });

  it('should reset to initial state', () => {
    chatStore.addMessage({
      id: '1',
      role: 'user',
      content: 'Test',
      timestamp: Date.now() / 1000,
    });
    chatStore.setTokenCount(100);

    chatStore.reset();
    const state = get(chatStore);

    expect(state.messages).toEqual([]);
    expect(state.tokenCount).toBe(0);
    expect(state.isStreaming).toBe(false);
  });
});
