import { describe, it, expect, beforeEach, vi } from 'vitest';
import { chat, type ToolState, type Message } from './chat.svelte';

describe('ChatState', () => {
  beforeEach(() => {
    chat.reset();
    vi.clearAllMocks();
  });

  describe('Message methods', () => {
    it('addUserMessage creates correct structure with id, role=user, content, timestamp', () => {
      const mockUUID = '00000000-0000-0000-0000-000000000001' as `${string}-${string}-${string}-${string}-${string}`;
      const mockTimestamp = 1234567890;
      vi.spyOn(crypto, 'randomUUID').mockReturnValue(mockUUID);
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.addUserMessage('Hello, Claude!');

      expect(chat.messages).toHaveLength(1);
      expect(chat.messages[0]).toEqual({
        id: mockUUID,
        role: 'user',
        content: 'Hello, Claude!',
        timestamp: mockTimestamp,
      });
    });

    it('addErrorMessage creates correct structure with role=error', () => {
      const mockUUID = '00000000-0000-0000-0000-000000000002' as `${string}-${string}-${string}-${string}-${string}`;
      const mockTimestamp = 9876543210;
      vi.spyOn(crypto, 'randomUUID').mockReturnValue(mockUUID);
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.addErrorMessage('Something went wrong');

      expect(chat.messages).toHaveLength(1);
      expect(chat.messages[0]).toEqual({
        id: mockUUID,
        role: 'error',
        content: 'Something went wrong',
        timestamp: mockTimestamp,
      });
    });
  });

  describe('Streaming methods', () => {
    it('appendText creates new item when streamItems is empty', () => {
      const mockTimestamp = 1111111111;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.appendText('First chunk');

      expect(chat.streamItems).toHaveLength(1);
      expect(chat.streamItems[0]).toMatchObject({
        type: 'text',
        content: 'First chunk',
        timestamp: mockTimestamp,
        insertionIndex: 0,
      });
      expect(chat.streamItems[0].id).toMatch(/^stream-text-/);
    });

    it('appendText appends to existing text item when last item is text', () => {
      const mockTimestamp = 2222222222;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.appendText('First ');
      chat.appendText('second ');
      chat.appendText('third');

      expect(chat.streamItems).toHaveLength(1);
      expect(chat.streamItems[0]).toMatchObject({
        type: 'text',
        content: 'First second third',
        insertionIndex: 0,
      });
    });

    it('appendText creates new item after a tool item', () => {
      const mockTimestamp1 = 3333333333;
      const mockTimestamp2 = 4444444444;
      vi.spyOn(Date, 'now')
        .mockReturnValueOnce(mockTimestamp1 * 1000)
        .mockReturnValueOnce(mockTimestamp2 * 1000);

      // Add initial text
      chat.appendText('Before tool');

      // Add a tool
      chat.addTool({
        id: 'tool-1',
        toolType: 'Read',
        target: 'file.txt',
        status: 'pending',
        timestamp: mockTimestamp1,
      });

      // Add text after tool
      chat.appendText('After tool');

      expect(chat.streamItems).toHaveLength(3);
      expect(chat.streamItems[0].type).toBe('text');
      expect(chat.streamItems[0].content).toBe('Before tool');
      expect(chat.streamItems[1].type).toBe('tool');
      expect(chat.streamItems[2].type).toBe('text');
      expect(chat.streamItems[2].content).toBe('After tool');
    });

    it('setStreaming(false) clears stream state (streamItems, tools)', () => {
      // Set up some stream state
      chat.appendText('Some text');
      chat.addTool({
        id: 'tool-1',
        toolType: 'Write',
        target: 'file.txt',
        status: 'running',
        timestamp: Date.now() / 1000,
      });

      expect(chat.streamItems.length).toBeGreaterThan(0);
      expect(chat.tools.size).toBeGreaterThan(0);

      // End streaming
      chat.setStreaming(false);

      expect(chat.isStreaming).toBe(false);
      expect(chat.streamItems).toHaveLength(0);
      expect(chat.tools.size).toBe(0);
    });

    it('setStreaming(true) sets isStreaming to true', () => {
      expect(chat.isStreaming).toBe(false);

      chat.setStreaming(true);

      expect(chat.isStreaming).toBe(true);
    });
  });

  describe('Tool methods', () => {
    it('addTool creates tool with insertionIndex and adds stream item reference', () => {
      const mockTimestamp = 5555555555;

      chat.addTool({
        id: 'tool-123',
        toolType: 'Edit',
        target: 'src/main.ts',
        status: 'pending',
        timestamp: mockTimestamp,
      });

      // Check tool was added to map
      expect(chat.tools.size).toBe(1);
      const tool = chat.tools.get('tool-123');
      expect(tool).toMatchObject({
        id: 'tool-123',
        toolType: 'Edit',
        target: 'src/main.ts',
        status: 'pending',
        timestamp: mockTimestamp,
        insertionIndex: 0,
      });

      // Check stream item was added
      expect(chat.streamItems).toHaveLength(1);
      expect(chat.streamItems[0]).toMatchObject({
        type: 'tool',
        toolId: 'tool-123',
        timestamp: mockTimestamp,
        insertionIndex: 0,
      });
      expect(chat.streamItems[0].id).toBe('stream-tool-tool-123');
    });

    it('updateToolStatus updates existing tool status', () => {
      chat.addTool({
        id: 'tool-456',
        toolType: 'Bash',
        target: 'npm install',
        status: 'pending',
        timestamp: Date.now() / 1000,
      });

      chat.updateToolStatus('tool-456', 'running');

      const tool = chat.tools.get('tool-456');
      expect(tool?.status).toBe('running');
    });

    it('updateToolStatus ignores unknown tool id', () => {
      const initialSize = chat.tools.size;

      chat.updateToolStatus('nonexistent-tool', 'completed');

      expect(chat.tools.size).toBe(initialSize);
    });

    it('updateToolStatus clears permissionRequestId when passed null', () => {
      chat.addTool({
        id: 'tool-789',
        toolType: 'Write',
        target: 'output.txt',
        status: 'awaiting_permission',
        timestamp: Date.now() / 1000,
        permissionRequestId: 'perm-123',
      });

      expect(chat.tools.get('tool-789')?.permissionRequestId).toBe('perm-123');

      chat.updateToolStatus('tool-789', 'running', { permissionRequestId: null });

      const tool = chat.tools.get('tool-789');
      expect(tool?.status).toBe('running');
      expect(tool?.permissionRequestId).toBeUndefined();
    });

    it('updateToolStatus sets permissionRequestId when passed value', () => {
      chat.addTool({
        id: 'tool-999',
        toolType: 'Edit',
        target: 'config.json',
        status: 'pending',
        timestamp: Date.now() / 1000,
      });

      chat.updateToolStatus('tool-999', 'awaiting_permission', { permissionRequestId: 'perm-456' });

      const tool = chat.tools.get('tool-999');
      expect(tool?.status).toBe('awaiting_permission');
      expect(tool?.permissionRequestId).toBe('perm-456');
    });

    it('updateToolStatus keeps permissionRequestId when extras is undefined', () => {
      chat.addTool({
        id: 'tool-keep',
        toolType: 'Write',
        target: 'file.txt',
        status: 'awaiting_permission',
        timestamp: Date.now() / 1000,
        permissionRequestId: 'perm-keep',
      });

      chat.updateToolStatus('tool-keep', 'running');

      const tool = chat.tools.get('tool-keep');
      expect(tool?.permissionRequestId).toBe('perm-keep');
    });

    it('getToolsAwaitingPermission filters only awaiting_permission status and sorts by insertionIndex', () => {
      // Add tools in mixed order with different statuses
      chat.addTool({
        id: 'tool-3',
        toolType: 'Write',
        target: 'third.txt',
        status: 'awaiting_permission',
        timestamp: Date.now() / 1000,
      });

      chat.addTool({
        id: 'tool-1',
        toolType: 'Edit',
        target: 'first.txt',
        status: 'completed',
        timestamp: Date.now() / 1000,
      });

      chat.addTool({
        id: 'tool-2',
        toolType: 'Bash',
        target: 'ls',
        status: 'awaiting_permission',
        timestamp: Date.now() / 1000,
      });

      chat.addTool({
        id: 'tool-4',
        toolType: 'Read',
        target: 'fourth.txt',
        status: 'running',
        timestamp: Date.now() / 1000,
      });

      const awaitingTools = chat.getToolsAwaitingPermission();

      expect(awaitingTools).toHaveLength(2);
      expect(awaitingTools[0].id).toBe('tool-3');
      expect(awaitingTools[0].insertionIndex).toBe(0);
      expect(awaitingTools[1].id).toBe('tool-2');
      expect(awaitingTools[1].insertionIndex).toBe(2);
    });

    it('clearPendingPermissions marks all awaiting tools as denied', () => {
      chat.addTool({
        id: 'tool-a',
        toolType: 'Write',
        target: 'a.txt',
        status: 'awaiting_permission',
        timestamp: Date.now() / 1000,
        permissionRequestId: 'perm-a',
      });

      chat.addTool({
        id: 'tool-b',
        toolType: 'Edit',
        target: 'b.txt',
        status: 'running',
        timestamp: Date.now() / 1000,
      });

      chat.addTool({
        id: 'tool-c',
        toolType: 'Bash',
        target: 'ls',
        status: 'awaiting_permission',
        timestamp: Date.now() / 1000,
        permissionRequestId: 'perm-c',
      });

      chat.clearPendingPermissions();

      const toolA = chat.tools.get('tool-a');
      const toolB = chat.tools.get('tool-b');
      const toolC = chat.tools.get('tool-c');

      expect(toolA?.status).toBe('denied');
      expect(toolA?.permissionRequestId).toBeUndefined();
      expect(toolB?.status).toBe('running'); // unchanged
      expect(toolC?.status).toBe('denied');
      expect(toolC?.permissionRequestId).toBeUndefined();
    });
  });

  describe('Finalize', () => {
    beforeEach(() => {
      vi.spyOn(crypto, 'randomUUID')
        .mockReturnValueOnce('00000000-0000-0000-0000-000000000011' as `${string}-${string}-${string}-${string}-${string}`)
        .mockReturnValueOnce('00000000-0000-0000-0000-000000000012' as `${string}-${string}-${string}-${string}-${string}`)
        .mockReturnValueOnce('00000000-0000-0000-0000-000000000013' as `${string}-${string}-${string}-${string}-${string}`)
        .mockReturnValueOnce('00000000-0000-0000-0000-000000000014' as `${string}-${string}-${string}-${string}-${string}`);
    });

    it('converts stream items to messages in correct order', () => {
      const mockTimestamp = 6666666666;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.appendText('Hello ');
      chat.appendText('world');

      chat.finalize();

      expect(chat.messages).toHaveLength(1);
      expect(chat.messages[0]).toMatchObject({
        role: 'assistant',
        content: 'Hello world',
      });
    });

    it('marks running/pending/awaiting_permission tools as aborted', () => {
      const mockTimestamp = 7777777777;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.addTool({
        id: 'tool-pending',
        toolType: 'Read',
        target: 'file1.txt',
        status: 'pending',
        timestamp: mockTimestamp,
      });

      chat.addTool({
        id: 'tool-running',
        toolType: 'Write',
        target: 'file2.txt',
        status: 'running',
        timestamp: mockTimestamp,
      });

      chat.addTool({
        id: 'tool-awaiting',
        toolType: 'Edit',
        target: 'file3.txt',
        status: 'awaiting_permission',
        timestamp: mockTimestamp,
      });

      chat.addTool({
        id: 'tool-completed',
        toolType: 'Bash',
        target: 'ls',
        status: 'completed',
        timestamp: mockTimestamp,
      });

      chat.finalize();

      expect(chat.messages).toHaveLength(1);
      const tools = chat.messages[0].tools!;

      expect(tools).toHaveLength(4);
      expect(tools.find(t => t.id === 'tool-pending')?.status).toBe('aborted');
      expect(tools.find(t => t.id === 'tool-running')?.status).toBe('aborted');
      expect(tools.find(t => t.id === 'tool-awaiting')?.status).toBe('aborted');
      expect(tools.find(t => t.id === 'tool-completed')?.status).toBe('completed');
    });

    it('groups tools with text correctly', () => {
      const mockTimestamp = 8888888888;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.appendText('First message');

      chat.addTool({
        id: 'tool-1',
        toolType: 'Read',
        target: 'file1.txt',
        status: 'completed',
        timestamp: mockTimestamp,
      });

      chat.addTool({
        id: 'tool-2',
        toolType: 'Write',
        target: 'file2.txt',
        status: 'completed',
        timestamp: mockTimestamp,
      });

      chat.appendText('Second message');

      chat.addTool({
        id: 'tool-3',
        toolType: 'Edit',
        target: 'file3.txt',
        status: 'completed',
        timestamp: mockTimestamp,
      });

      chat.finalize();

      // finalize creates messages when text appears after tools (flushes tools)
      // Sequence: text -> tool -> tool -> text (flush!) -> tool -> end (flush!)
      // Result: 2 messages
      expect(chat.messages).toHaveLength(2);

      // First message: text with 2 tools (flushed when second text appeared)
      expect(chat.messages[0].content).toBe('First message');
      expect(chat.messages[0].tools).toHaveLength(2);
      expect(chat.messages[0].tools![0].id).toBe('tool-1');
      expect(chat.messages[0].tools![1].id).toBe('tool-2');

      // Second message: text with 1 tool (flushed at end)
      expect(chat.messages[1].content).toBe('Second message');
      expect(chat.messages[1].tools).toHaveLength(1);
      expect(chat.messages[1].tools![0].id).toBe('tool-3');
    });

    it('clears all streaming state after finalization', () => {
      const mockTimestamp = 9999999999;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.setStreaming(true);
      chat.appendText('Some text');
      chat.addTool({
        id: 'tool-final',
        toolType: 'Read',
        target: 'file.txt',
        status: 'completed',
        timestamp: mockTimestamp,
      });

      expect(chat.isStreaming).toBe(true);
      expect(chat.streamItems.length).toBeGreaterThan(0);
      expect(chat.tools.size).toBeGreaterThan(0);

      chat.finalize();

      expect(chat.isStreaming).toBe(false);
      expect(chat.streamItems).toHaveLength(0);
      expect(chat.tools.size).toBe(0);
    });

    it('handles empty stream gracefully', () => {
      const initialMessages = chat.messages.length;

      chat.finalize();

      expect(chat.messages).toHaveLength(initialMessages);
    });

    it('creates message with tools but no text', () => {
      const mockTimestamp = 1010101010;
      vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp * 1000);

      chat.addTool({
        id: 'tool-only',
        toolType: 'Bash',
        target: 'echo test',
        status: 'completed',
        timestamp: mockTimestamp,
      });

      chat.finalize();

      expect(chat.messages).toHaveLength(1);
      expect(chat.messages[0].content).toBe('');
      expect(chat.messages[0].tools).toHaveLength(1);
      expect(chat.messages[0].tools![0].id).toBe('tool-only');
    });
  });

  describe('Queue methods', () => {
    it('enqueueMessage / dequeueMessage maintains FIFO order', () => {
      chat.enqueueMessage('First');
      chat.enqueueMessage('Second');
      chat.enqueueMessage('Third');

      expect(chat.dequeueMessage()).toBe('First');
      expect(chat.dequeueMessage()).toBe('Second');
      expect(chat.dequeueMessage()).toBe('Third');
    });

    it('dequeueMessage returns undefined when empty', () => {
      expect(chat.dequeueMessage()).toBeUndefined();
    });

    it('dequeueMessage empties the queue correctly', () => {
      chat.enqueueMessage('Only message');

      expect(chat.messageQueue).toHaveLength(1);
      expect(chat.dequeueMessage()).toBe('Only message');
      expect(chat.messageQueue).toHaveLength(0);
      expect(chat.dequeueMessage()).toBeUndefined();
    });

    it('clearMessageQueue removes all queued messages', () => {
      chat.enqueueMessage('First');
      chat.enqueueMessage('Second');
      chat.enqueueMessage('Third');

      expect(chat.messageQueue).toHaveLength(3);

      chat.clearMessageQueue();

      expect(chat.messageQueue).toHaveLength(0);
      expect(chat.dequeueMessage()).toBeUndefined();
    });
  });

  describe('Reset', () => {
    it('resets all state to initial values', () => {
      // Populate all state
      chat.addUserMessage('Test message');
      chat.setStreaming(true);
      chat.appendText('Stream text');
      chat.addTool({
        id: 'tool-reset',
        toolType: 'Read',
        target: 'file.txt',
        status: 'running',
        timestamp: Date.now() / 1000,
      });
      chat.enqueueMessage('Queued message');
      chat.addTodo({ id: 'todo-1', text: 'Do something', done: false, active: true });
      chat.addActiveSkill('test-skill');
      chat.tokenCount = 1000;
      chat.sessionDuration = 60;

      // Reset
      chat.reset();

      // Verify all state is cleared
      expect(chat.messages).toHaveLength(0);
      expect(chat.isStreaming).toBe(false);
      expect(chat.streamItems).toHaveLength(0);
      expect(chat.tools.size).toBe(0);
      expect(chat.messageQueue).toHaveLength(0);
      expect(chat.todos).toHaveLength(0);
      expect(chat.activeSkills).toHaveLength(0);
      expect(chat.tokenCount).toBe(0);
      expect(chat.sessionDuration).toBe(0);
    });
  });
});
