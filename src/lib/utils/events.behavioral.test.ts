import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { handleChatEvent, resetEventState, type ChatEvent } from './events';
import { chat } from '$lib/stores/chat.svelte';
import { app } from '$lib/stores/app.svelte';
import {
  makeTextEvent,
  makeCompleteEvent,
  makeToolStartEvent,
  makeAbortCompleteEvent,
} from '../../tests/fixtures/events';

// Mock Tauri invoke (needed by app store's api imports)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('Behavioral Tests', () => {
  beforeEach(() => {
    app.setScreen('chat');
    chat.reset();
    resetEventState();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('Message Queue', () => {
    it('dequeues messages in FIFO order after Complete', () => {
      chat.setStreaming(true);
      chat.enqueueMessage('First queued');
      chat.enqueueMessage('Second queued');

      // Verify queue has 2 messages
      expect(chat.messageQueue).toHaveLength(2);

      // Dequeue after completion
      expect(chat.dequeueMessage()).toBe('First queued');
      expect(chat.dequeueMessage()).toBe('Second queued');
      expect(chat.dequeueMessage()).toBeUndefined();
    });

    it('clearMessageQueue removes all pending messages', () => {
      chat.enqueueMessage('A');
      chat.enqueueMessage('B');
      chat.enqueueMessage('C');

      expect(chat.messageQueue).toHaveLength(3);
      chat.clearMessageQueue();
      expect(chat.messageQueue).toHaveLength(0);
      expect(chat.dequeueMessage()).toBeUndefined();
    });

    it('AbortComplete clears the message queue', () => {
      chat.setStreaming(true);
      chat.enqueueMessage('Queued message');

      handleChatEvent(makeAbortCompleteEvent());
      vi.advanceTimersByTime(200);

      expect(chat.messageQueue).toHaveLength(0);
    });
  });

  describe('Text buffer timing', () => {
    it('does not flush text without newline before 150ms', () => {
      handleChatEvent(makeTextEvent('Hello'));

      // No flush yet
      expect(chat.streamItems).toHaveLength(0);

      // Advance less than 150ms
      vi.advanceTimersByTime(100);
      expect(chat.streamItems).toHaveLength(0);
    });

    it('flushes text without newline after 150ms', () => {
      handleChatEvent(makeTextEvent('Hello'));

      vi.advanceTimersByTime(150);

      expect(chat.streamItems).toHaveLength(1);
      expect(chat.streamItems[0].content).toBe('Hello');
    });

    it('flushes text with newline immediately', () => {
      handleChatEvent(makeTextEvent('Hello\n'));

      // Should be flushed immediately (no timer needed)
      expect(chat.streamItems).toHaveLength(1);
      expect(chat.streamItems[0].content).toBe('Hello\n');
    });

    it('resets flush timer on new content', () => {
      handleChatEvent(makeTextEvent('First'));
      vi.advanceTimersByTime(100);

      handleChatEvent(makeTextEvent(' Second'));
      vi.advanceTimersByTime(100);

      // Still not flushed (timer was reset)
      expect(chat.streamItems).toHaveLength(0);

      vi.advanceTimersByTime(50);
      // Now flushed (150ms since "Second")
      expect(chat.streamItems).toHaveLength(1);
      expect(chat.streamItems[0].content).toBe('First Second');
    });

    it('ToolStart flushes buffered text', () => {
      handleChatEvent(makeTextEvent('Buffered'));

      expect(chat.streamItems).toHaveLength(0);

      handleChatEvent(makeToolStartEvent());

      // Buffer should have been flushed before tool
      expect(chat.streamItems).toHaveLength(2);
      expect(chat.streamItems[0].type).toBe('text');
      expect(chat.streamItems[0].content).toBe('Buffered');
      expect(chat.streamItems[1].type).toBe('tool');
    });

    it('Complete flushes buffered text', () => {
      handleChatEvent(makeTextEvent('Final text'));

      expect(chat.streamItems).toHaveLength(0);

      handleChatEvent(makeCompleteEvent());

      // After finalize, streamItems are cleared, but messages should have the content
      expect(chat.messages).toHaveLength(1);
      expect(chat.messages[0].content).toBe('Final text');
    });
  });

  describe('Rapid events', () => {
    it('handles 50 single-character text events correctly', () => {
      const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWX';

      for (const char of chars) {
        handleChatEvent(makeTextEvent(char));
      }

      // Flush the buffer
      vi.advanceTimersByTime(200);

      // All chars should be concatenated
      const allContent = chat.streamItems
        .filter(item => item.type === 'text')
        .map(item => item.content)
        .join('');
      expect(allContent).toBe(chars);
    });

    it('handles interleaved text and newlines correctly', () => {
      handleChatEvent(makeTextEvent('Line 1'));
      handleChatEvent(makeTextEvent('\n'));
      handleChatEvent(makeTextEvent('Line 2'));
      handleChatEvent(makeTextEvent('\n'));
      handleChatEvent(makeTextEvent('Partial'));

      vi.advanceTimersByTime(200);

      const allContent = chat.streamItems
        .filter(item => item.type === 'text')
        .map(item => item.content)
        .join('');
      expect(allContent).toBe('Line 1\nLine 2\nPartial');
    });
  });

  describe('Abort cleanup', () => {
    it('clears streaming state and permissions on abort', () => {
      chat.setStreaming(true);

      // Add a tool with pending permission
      chat.addTool({
        id: 'tool-1',
        toolType: 'Edit',
        target: 'file.txt',
        status: 'awaiting_permission',
        permissionRequestId: 'perm-1',
        timestamp: Date.now() / 1000,
      });

      expect(chat.tools.size).toBe(1);

      handleChatEvent(makeAbortCompleteEvent());
      vi.advanceTimersByTime(200);

      // After abort: streaming stopped, tools cleared (via finalize + setStreaming(false))
      expect(chat.isStreaming).toBe(false);
      expect(chat.tools.size).toBe(0);
      expect(chat.streamItems).toHaveLength(0);
    });
  });
});
