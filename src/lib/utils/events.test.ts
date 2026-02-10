import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  handleChatEvent,
  resetEventState,
  respondToPermission,
  setOnContentChange,
  type ChatEvent,
  type EventHandlerOptions,
} from './events';
import { chat, type ToolState, type ToolStatus } from '$lib/stores/chat.svelte';
import { app, type PermissionMode, type Model } from '$lib/stores/app.svelte';
import { invoke } from '@tauri-apps/api/core';

// Mock the stores
vi.mock('$lib/stores/chat.svelte', () => ({
  chat: {
    appendText: vi.fn(),
    addTool: vi.fn(),
    updateToolStatus: vi.fn(),
    getTool: vi.fn(),
    addActiveSkill: vi.fn(),
    setTodos: vi.fn(),
    addTodo: vi.fn(),
    updateTodo: vi.fn(),
    finalize: vi.fn(),
    setStreaming: vi.fn(),
    clearMessageQueue: vi.fn(),
    clearPendingPermissions: vi.fn(),
    addErrorMessage: vi.fn(),
    tokenCount: 0,
    contextWindow: null,
  },
}));

vi.mock('$lib/stores/app.svelte', () => ({
  app: {
    setAuthType: vi.fn(),
    syncState: vi.fn(),
  },
}));

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('handleChatEvent', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetEventState();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    setOnContentChange(null);
  });

  describe('Text events', () => {
    it('should buffer text until newline', () => {
      const event: ChatEvent = {
        type: 'Text',
        content: 'Hello',
      };

      handleChatEvent(event);

      // Should not append yet - waiting for newline
      expect(chat.appendText).not.toHaveBeenCalled();
    });

    it('should flush complete lines immediately', () => {
      const event1: ChatEvent = {
        type: 'Text',
        content: 'Hello\n',
      };

      handleChatEvent(event1);

      expect(chat.appendText).toHaveBeenCalledWith('Hello\n');
      expect(chat.appendText).toHaveBeenCalledTimes(1);
    });

    it('should handle multiple lines in one event', () => {
      const event: ChatEvent = {
        type: 'Text',
        content: 'Line 1\nLine 2\nLine 3\n',
      };

      handleChatEvent(event);

      expect(chat.appendText).toHaveBeenCalledWith('Line 1\nLine 2\nLine 3\n');
      expect(chat.appendText).toHaveBeenCalledTimes(1);
    });

    it('should buffer partial line and flush on next newline', () => {
      const event1: ChatEvent = {
        type: 'Text',
        content: 'Hello ',
      };
      const event2: ChatEvent = {
        type: 'Text',
        content: 'world\n',
      };

      handleChatEvent(event1);
      expect(chat.appendText).not.toHaveBeenCalled();

      handleChatEvent(event2);
      expect(chat.appendText).toHaveBeenCalledWith('Hello world\n');
    });

    it('should flush buffered content after timer delay', () => {
      const event: ChatEvent = {
        type: 'Text',
        content: 'No newline here',
      };

      handleChatEvent(event);

      expect(chat.appendText).not.toHaveBeenCalled();

      // Advance timer by 150ms (FLUSH_DELAY_MS)
      vi.advanceTimersByTime(150);

      expect(chat.appendText).toHaveBeenCalledWith('No newline here');
    });

    it('should reset timer on new buffered content', () => {
      const event1: ChatEvent = {
        type: 'Text',
        content: 'First',
      };
      const event2: ChatEvent = {
        type: 'Text',
        content: ' Second',
      };

      handleChatEvent(event1);
      vi.advanceTimersByTime(100); // Not enough to trigger

      handleChatEvent(event2);
      vi.advanceTimersByTime(100); // Still not enough (timer was reset)

      expect(chat.appendText).not.toHaveBeenCalled();

      vi.advanceTimersByTime(50); // Now 150ms from second event

      expect(chat.appendText).toHaveBeenCalledWith('First Second');
    });

    it('should handle mixed complete and incomplete lines', () => {
      const event: ChatEvent = {
        type: 'Text',
        content: 'Complete line\nIncomplete',
      };

      handleChatEvent(event);

      expect(chat.appendText).toHaveBeenCalledWith('Complete line\n');
      expect(chat.appendText).toHaveBeenCalledTimes(1);

      // The incomplete part should flush after timer
      vi.advanceTimersByTime(150);
      expect(chat.appendText).toHaveBeenCalledWith('Incomplete');
      expect(chat.appendText).toHaveBeenCalledTimes(2);
    });

    // Note: "should ignore events without content" test removed
    // With discriminated union types, Text events require content at compile time

    it('should notify onContentChange when a full line is appended', () => {
      const onChange = vi.fn();
      setOnContentChange(onChange);

      const event: ChatEvent = {
        type: 'Text',
        content: 'Hello\n',
      };

      handleChatEvent(event);

      expect(onChange).toHaveBeenCalledTimes(1);
      setOnContentChange(null);
    });

    it('should notify onContentChange on timer-based flush', () => {
      const onChange = vi.fn();
      setOnContentChange(onChange);

      const event: ChatEvent = {
        type: 'Text',
        content: 'No newline',
      };

      handleChatEvent(event);
      vi.advanceTimersByTime(150);

      expect(onChange).toHaveBeenCalledTimes(1);
      setOnContentChange(null);
    });
  });

  describe('Tool events - ToolStart', () => {
    it('should create tool with pending status and flush text buffer first', () => {
      // Buffer some text first
      const textEvent: ChatEvent = {
        type: 'Text',
        content: 'Buffered text',
      };
      handleChatEvent(textEvent);

      const toolEvent: ChatEvent = {
        type: 'ToolStart',
        toolUseId: 'tool-123',
        toolType: 'Read',
        target: '/path/to/file.txt',
        status: 'pending',
        input: { file_path: '/path/to/file.txt' },
      };

      handleChatEvent(toolEvent);

      // Should flush buffered text first
      expect(chat.appendText).toHaveBeenCalledWith('Buffered text');

      // Then add tool
      expect(chat.addTool).toHaveBeenCalledWith({
        id: 'tool-123',
        toolType: 'Read',
        target: '/path/to/file.txt',
        status: 'pending',
        input: { file_path: '/path/to/file.txt' },
        timestamp: expect.any(Number),
      });
    });

    // Note: Tests for missing fields removed - discriminated union types enforce required fields at compile time
  });

  describe('Tool events - ToolStatusUpdate', () => {
    it('should update tool status and set permissionRequestId', () => {
      const event: ChatEvent = {
        type: 'ToolStatusUpdate',
        toolUseId: 'tool-123',
        status: 'awaiting_permission',
        permissionRequestId: 'perm-456',
      };

      handleChatEvent(event);

      expect(chat.updateToolStatus).toHaveBeenCalledWith(
        'tool-123',
        'awaiting_permission',
        { permissionRequestId: 'perm-456' }
      );
    });

    it('should clear permissionRequestId when not provided', () => {
      const event: ChatEvent = {
        type: 'ToolStatusUpdate',
        toolUseId: 'tool-123',
        status: 'running',
      };

      handleChatEvent(event);

      expect(chat.updateToolStatus).toHaveBeenCalledWith(
        'tool-123',
        'running',
        { permissionRequestId: null }
      );
    });

    // Note: Tests for missing fields removed - discriminated union types enforce required fields at compile time
  });

  describe('Tool events - ToolEnd', () => {
    it('should update tool status on completion', () => {
      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.updateToolStatus).toHaveBeenCalledWith('tool-123', 'completed');
    });

    it('should update tool status on error', () => {
      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'error',
      };

      handleChatEvent(event);

      expect(chat.updateToolStatus).toHaveBeenCalledWith('tool-123', 'error');
    });

    it('should activate skill when Skill tool completes', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'Skill',
        target: 'pdf',
        status: 'completed',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.addActiveSkill).toHaveBeenCalledWith('pdf');
    });

    it('should not activate skill if no target', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'Skill',
        target: '',
        status: 'completed',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.addActiveSkill).not.toHaveBeenCalled();
    });

    it('should handle TodoWrite with todos array', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TodoWrite',
        target: '',
        status: 'completed',
        input: {
          todos: [
            { id: '1', content: 'Task 1', status: 'pending' },
            { id: '2', text: 'Task 2', status: 'completed' },
            { id: '3', subject: 'Task 3', status: 'in_progress' },
          ],
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.setTodos).toHaveBeenCalledWith([
        { id: '1', text: 'Task 1', done: false, active: false },
        { id: '2', text: 'Task 2', done: true, active: false },
        { id: '3', text: 'Task 3', done: false, active: true },
      ]);
    });

    it('should handle TodoWrite with items array', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TodoWrite',
        target: '',
        status: 'completed',
        input: {
          items: [{ content: 'Task 1', status: 'pending' }],
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.setTodos).toHaveBeenCalledWith(
        expect.arrayContaining([
          expect.objectContaining({ text: 'Task 1' }),
        ])
      );
    });

    it('should handle TodoWrite with tasks array', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TodoWrite',
        target: '',
        status: 'completed',
        input: {
          tasks: [{ content: 'Task 1', status: 'pending' }],
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.setTodos).toHaveBeenCalledWith(
        expect.arrayContaining([
          expect.objectContaining({ text: 'Task 1' }),
        ])
      );
    });

    it('should generate UUID for todos without id', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TodoWrite',
        target: '',
        status: 'completed',
        input: {
          todos: [{ content: 'Task without ID' }],
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.setTodos).toHaveBeenCalledWith([
        expect.objectContaining({
          id: expect.any(String),
          text: 'Task without ID',
        }),
      ]);
    });

    it('should handle TaskCreate tool', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TaskCreate',
        target: '',
        status: 'completed',
        input: {
          subject: 'New task from TaskCreate',
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.addTodo).toHaveBeenCalledWith({
        id: 'tool-123',
        text: 'New task from TaskCreate',
        done: false,
        active: true,
      });
    });

    it('should use default text for TaskCreate without subject', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TaskCreate',
        target: '',
        status: 'completed',
        input: {},
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.addTodo).toHaveBeenCalledWith(
        expect.objectContaining({
          text: 'New todo',
        })
      );
    });

    it('should handle TaskUpdate tool - mark completed', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TaskUpdate',
        target: '',
        status: 'completed',
        input: {
          taskId: 'task-456',
          status: 'completed',
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.updateTodo).toHaveBeenCalledWith('task-456', {
        done: true,
        active: false,
      });
    });

    it('should handle TaskUpdate tool - mark in progress', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TaskUpdate',
        target: '',
        status: 'completed',
        input: {
          taskId: 'task-456',
          status: 'in_progress',
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.updateTodo).toHaveBeenCalledWith('task-456', {
        done: false,
        active: true,
      });
    });

    it('should handle TaskUpdate tool - update subject', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TaskUpdate',
        target: '',
        status: 'completed',
        input: {
          taskId: 'task-456',
          status: 'pending',
          subject: 'Updated task text',
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.updateTodo).toHaveBeenCalledWith('task-456', {
        done: false,
        active: false,
        text: 'Updated task text',
      });
    });

    it('should not call updateTodo if taskId is missing', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'TaskUpdate',
        target: '',
        status: 'completed',
        input: {
          status: 'completed',
        },
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'completed',
      };

      handleChatEvent(event);

      expect(chat.updateTodo).not.toHaveBeenCalled();
    });

    it('should only process special tools when status is completed', () => {
      const mockTool: ToolState = {
        id: 'tool-123',
        toolType: 'Skill',
        target: 'pdf',
        status: 'error',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      vi.mocked(chat.getTool).mockReturnValue(mockTool);

      const event: ChatEvent = {
        type: 'ToolEnd',
        id: 'tool-123',
        status: 'error',
      };

      handleChatEvent(event);

      expect(chat.addActiveSkill).not.toHaveBeenCalled();
    });

    // Note: Tests for missing fields removed - discriminated union types enforce required fields at compile time
  });

  describe('Lifecycle events - Complete', () => {
    it('should flush buffer, finalize, and call onComplete callback', () => {
      const onComplete = vi.fn();

      // Buffer some text
      const textEvent: ChatEvent = {
        type: 'Text',
        content: 'Buffered text',
      };
      handleChatEvent(textEvent);

      const completeEvent: ChatEvent = {
        type: 'Complete',
      };

      handleChatEvent(completeEvent, { onComplete });

      expect(chat.appendText).toHaveBeenCalledWith('Buffered text');
      expect(chat.finalize).toHaveBeenCalled();
      expect(onComplete).toHaveBeenCalled();
    });

    it('should work without onComplete callback', () => {
      const event: ChatEvent = {
        type: 'Complete',
      };

      expect(() => handleChatEvent(event)).not.toThrow();
      expect(chat.finalize).toHaveBeenCalled();
    });

    it('should not flush if buffer is empty', () => {
      const event: ChatEvent = {
        type: 'Complete',
      };

      handleChatEvent(event);

      expect(chat.appendText).not.toHaveBeenCalled();
      expect(chat.finalize).toHaveBeenCalled();
    });
  });

  describe('Lifecycle events - AbortComplete', () => {
    it('should flush buffer, finalize, stop streaming, clear queue and permissions', () => {
      // Buffer some text
      const textEvent: ChatEvent = {
        type: 'Text',
        content: 'Buffered text',
      };
      handleChatEvent(textEvent);

      const abortEvent: ChatEvent = {
        type: 'AbortComplete',
        sessionId: 'session-123',
      };

      handleChatEvent(abortEvent);

      expect(chat.appendText).toHaveBeenCalledWith('Buffered text');
      expect(chat.finalize).toHaveBeenCalled();
      expect(chat.setStreaming).toHaveBeenCalledWith(false);
      expect(chat.clearMessageQueue).toHaveBeenCalled();
      expect(chat.clearPendingPermissions).toHaveBeenCalled();
    });

    it('should work with empty buffer', () => {
      const event: ChatEvent = {
        type: 'AbortComplete',
        sessionId: 'session-123',
      };

      handleChatEvent(event);

      expect(chat.appendText).not.toHaveBeenCalled();
      expect(chat.finalize).toHaveBeenCalled();
      expect(chat.setStreaming).toHaveBeenCalledWith(false);
    });
  });

  describe('Lifecycle events - Error', () => {
    it('should add error message, finalize, and call onError callback', () => {
      const onError = vi.fn();

      const event: ChatEvent = {
        type: 'Error',
        message: 'Something went wrong',
      };

      handleChatEvent(event, { onError });

      expect(chat.finalize).toHaveBeenCalled();
      expect(chat.addErrorMessage).toHaveBeenCalledWith('Something went wrong');
      expect(onError).toHaveBeenCalledWith('Something went wrong');
    });

    // Note: "should use default error message if none provided" test removed
    // With discriminated union types, Error events require message at compile time

    it('should work without onError callback', () => {
      const event: ChatEvent = {
        type: 'Error',
        message: 'Error without callback',
      };

      expect(() => handleChatEvent(event)).not.toThrow();
      expect(chat.addErrorMessage).toHaveBeenCalledWith('Error without callback');
    });
  });

  describe('Thinking events', () => {
    it('should flush buffered text before adding thinking tool', () => {
      const textEvent: ChatEvent = {
        type: 'Text',
        content: 'Buffered text',
      };
      handleChatEvent(textEvent);

      const thinkingEvent: ChatEvent = {
        type: 'ThinkingStart',
        thinkingId: 'think-1',
        content: 'This is a long thought',
      };

      handleChatEvent(thinkingEvent);

      expect(chat.appendText).toHaveBeenCalledWith('Buffered text');
      expect(chat.addTool).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'think-1',
          toolType: 'Thinking',
          status: 'completed',
          input: { content: 'This is a long thought' },
        })
      );
    });

    it('should pass full thinking content (CSS handles truncation)', () => {
      const longText = 'a'.repeat(60);
      const thinkingEvent: ChatEvent = {
        type: 'ThinkingStart',
        thinkingId: 'think-2',
        content: longText,
      };

      handleChatEvent(thinkingEvent);

      expect(chat.addTool).toHaveBeenCalledWith(
        expect.objectContaining({
          target: longText,  // Full content passed through, CSS handles truncation
        })
      );
    });
  });

  describe('Other events', () => {
    it('should handle SessionInit event', () => {
      const event: ChatEvent = {
        type: 'SessionInit',
        auth_type: 'api_key',
      };

      handleChatEvent(event);

      expect(app.setAuthType).toHaveBeenCalledWith('api_key');
    });

    it('should handle StateChanged event', () => {
      const event: ChatEvent = {
        type: 'StateChanged',
        permissionMode: 'acceptEdits',
        model: 'opus',
      };

      handleChatEvent(event);

      expect(app.syncState).toHaveBeenCalledWith('acceptEdits', 'opus');
    });

    it('should handle TokenUsage event', () => {
      const event: ChatEvent = {
        type: 'TokenUsage',
        totalTokens: 1234,
      };

      handleChatEvent(event);

      expect(chat.tokenCount).toBe(1234);
      expect(chat.contextWindow).toBeNull();
    });

    it('should prefer context token/window when provided', () => {
      const event: ChatEvent = {
        type: 'TokenUsage',
        totalTokens: 1234,
        contextTokens: 800,
        contextWindow: 200000,
      };

      handleChatEvent(event);

      expect(chat.tokenCount).toBe(800);
      expect(chat.contextWindow).toBe(200000);
    });

    // Note: Tests for missing fields removed - discriminated union types enforce required fields at compile time
  });

  describe('resetEventState', () => {
    it('should clear line buffer', () => {
      // Buffer some text
      const textEvent: ChatEvent = {
        type: 'Text',
        content: 'Buffered text',
      };
      handleChatEvent(textEvent);

      resetEventState();

      // Complete event should not flush anything
      const completeEvent: ChatEvent = {
        type: 'Complete',
      };
      handleChatEvent(completeEvent);

      expect(chat.appendText).not.toHaveBeenCalled();
    });

    it('should clear flush timer', () => {
      // Start a timer
      const textEvent: ChatEvent = {
        type: 'Text',
        content: 'Buffered text',
      };
      handleChatEvent(textEvent);

      resetEventState();

      // Advance timer - should not flush
      vi.advanceTimersByTime(150);

      expect(chat.appendText).not.toHaveBeenCalled();
    });
  });

  describe('respondToPermission', () => {
    it('should call invoke with correct parameters when allowed', async () => {
      const tool: ToolState = {
        id: 'tool-123',
        toolType: 'Edit',
        target: 'file.txt',
        status: 'awaiting_permission',
        permissionRequestId: 'perm-456',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      await respondToPermission('session-789', tool, true);

      expect(invoke).toHaveBeenCalledWith('respond_permission', {
        sessionId: 'session-789',
        requestId: 'perm-456',
        allowed: true,
      });
    });

    it('should call invoke with correct parameters when denied', async () => {
      const tool: ToolState = {
        id: 'tool-123',
        toolType: 'Bash',
        target: 'ls -la',
        status: 'awaiting_permission',
        permissionRequestId: 'perm-456',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      await respondToPermission('session-789', tool, false);

      expect(invoke).toHaveBeenCalledWith('respond_permission', {
        sessionId: 'session-789',
        requestId: 'perm-456',
        allowed: false,
      });
    });

    it('should log error and return early if no permissionRequestId', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const tool: ToolState = {
        id: 'tool-123',
        toolType: 'Edit',
        target: 'file.txt',
        status: 'pending',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      await respondToPermission('session-789', tool, true);

      expect(consoleSpy).toHaveBeenCalledWith(
        'No permission request ID for tool:',
        'tool-123'
      );
      expect(invoke).not.toHaveBeenCalled();

      consoleSpy.mockRestore();
    });

    it('should handle invoke errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      vi.mocked(invoke).mockRejectedValue(new Error('Network error'));

      const tool: ToolState = {
        id: 'tool-123',
        toolType: 'Edit',
        target: 'file.txt',
        status: 'awaiting_permission',
        permissionRequestId: 'perm-456',
        timestamp: Date.now() / 1000,
        insertionIndex: 0,
      };

      await respondToPermission('session-789', tool, true);

      expect(consoleSpy).toHaveBeenCalledWith(
        'Failed to respond to permission:',
        expect.any(Error)
      );

      consoleSpy.mockRestore();
    });
  });
});
