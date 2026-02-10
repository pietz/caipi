import { describe, it, expect, beforeEach, vi } from 'vitest';
import { chat } from './chat.svelte';

describe('chat.loadHistory - edge cases', () => {
  beforeEach(() => {
    chat.reset();
  });

  it('loads basic user + assistant messages', () => {
    chat.loadHistory([
      { id: 'msg-1', role: 'user', content: 'Hello', timestamp: 1000, tools: [] },
      { id: 'msg-2', role: 'assistant', content: 'Hi there!', timestamp: 1001, tools: [] },
    ]);

    expect(chat.messages).toHaveLength(2);
    expect(chat.messages[0].role).toBe('user');
    expect(chat.messages[0].content).toBe('Hello');
    expect(chat.messages[1].role).toBe('assistant');
    expect(chat.messages[1].content).toBe('Hi there!');
  });

  it('merges consecutive empty assistant messages with tools into previous assistant', () => {
    chat.loadHistory([
      { id: 'msg-1', role: 'assistant', content: 'Working on it', timestamp: 1000, tools: [] },
      {
        id: 'msg-2', role: 'assistant', content: '', timestamp: 1001,
        tools: [{ id: 'tool-1', toolType: 'Read', target: 'file.txt' }],
      },
      {
        id: 'msg-3', role: 'assistant', content: '  ', timestamp: 1002,
        tools: [{ id: 'tool-2', toolType: 'Write', target: 'out.txt' }],
      },
    ]);

    expect(chat.messages).toHaveLength(1);
    expect(chat.messages[0].content).toBe('Working on it');
    expect(chat.messages[0].tools).toHaveLength(2);
    expect(chat.messages[0].tools![0].id).toBe('tool-1');
    expect(chat.messages[0].tools![1].id).toBe('tool-2');
  });

  it('does not merge tool-only assistant messages after a user message', () => {
    chat.loadHistory([
      { id: 'msg-1', role: 'user', content: 'Do something', timestamp: 1000, tools: [] },
      {
        id: 'msg-2', role: 'assistant', content: '', timestamp: 1001,
        tools: [{ id: 'tool-1', toolType: 'Bash', target: 'ls' }],
      },
    ]);

    expect(chat.messages).toHaveLength(2);
    expect(chat.messages[1].role).toBe('assistant');
    expect(chat.messages[1].tools).toHaveLength(1);
  });

  it('deduplicates tool IDs with __dup_N suffix', () => {
    chat.loadHistory([
      {
        id: 'msg-1', role: 'assistant', content: 'Results', timestamp: 1000,
        tools: [
          { id: 'same-id', toolType: 'Read', target: 'a.txt' },
          { id: 'same-id', toolType: 'Read', target: 'b.txt' },
          { id: 'same-id', toolType: 'Read', target: 'c.txt' },
        ],
      },
    ]);

    expect(chat.messages).toHaveLength(1);
    const tools = chat.messages[0].tools!;
    expect(tools).toHaveLength(3);
    expect(tools[0].id).toBe('same-id');
    expect(tools[1].id).toBe('same-id__dup_1');
    expect(tools[2].id).toBe('same-id__dup_2');
  });

  it('assigns all tool statuses as "history"', () => {
    chat.loadHistory([
      {
        id: 'msg-1', role: 'assistant', content: 'Done', timestamp: 1000,
        tools: [{ id: 'tool-1', toolType: 'Read', target: 'file.txt' }],
      },
    ]);

    expect(chat.messages[0].tools![0].status).toBe('history');
  });

  it('assigns increasing insertionIndex across messages', () => {
    chat.loadHistory([
      {
        id: 'msg-1', role: 'assistant', content: 'First', timestamp: 1000,
        tools: [
          { id: 'tool-1', toolType: 'Read', target: 'a.txt' },
          { id: 'tool-2', toolType: 'Write', target: 'b.txt' },
        ],
      },
      {
        id: 'msg-2', role: 'assistant', content: 'Second', timestamp: 1001,
        tools: [{ id: 'tool-3', toolType: 'Bash', target: 'ls' }],
      },
    ]);

    const allTools = chat.messages.flatMap(m => m.tools ?? []);
    expect(allTools[0].insertionIndex).toBe(0);
    expect(allTools[1].insertionIndex).toBe(1);
    expect(allTools[2].insertionIndex).toBe(2);
  });

  it('handles empty history gracefully', () => {
    chat.loadHistory([]);
    expect(chat.messages).toHaveLength(0);
  });

  it('messages without tools get no tools array', () => {
    chat.loadHistory([
      { id: 'msg-1', role: 'user', content: 'Hi', timestamp: 1000, tools: [] },
    ]);

    expect(chat.messages[0].tools).toBeUndefined();
  });

  it('preserves history messages when new streaming messages are finalized after', () => {
    // Load some history
    chat.loadHistory([
      { id: 'msg-1', role: 'user', content: 'Old question', timestamp: 1000, tools: [] },
      { id: 'msg-2', role: 'assistant', content: 'Old answer', timestamp: 1001, tools: [] },
    ]);

    expect(chat.messages).toHaveLength(2);

    // Simulate new streaming content
    chat.appendText('New response content');
    chat.finalize();

    // History messages should still be there + new one
    expect(chat.messages).toHaveLength(3);
    expect(chat.messages[0].content).toBe('Old question');
    expect(chat.messages[1].content).toBe('Old answer');
    expect(chat.messages[2].content).toBe('New response content');
    expect(chat.messages[2].role).toBe('assistant');
  });

  it('handles mixed user/assistant without cross-role merging', () => {
    chat.loadHistory([
      { id: 'msg-1', role: 'user', content: 'Q1', timestamp: 1000, tools: [] },
      { id: 'msg-2', role: 'assistant', content: 'A1', timestamp: 1001, tools: [] },
      { id: 'msg-3', role: 'user', content: 'Q2', timestamp: 1002, tools: [] },
      {
        id: 'msg-4', role: 'assistant', content: '', timestamp: 1003,
        tools: [{ id: 'tool-1', toolType: 'Read', target: 'file.txt' }],
      },
    ]);

    // msg-4 is empty assistant after user, so it shouldn't merge with msg-2
    expect(chat.messages).toHaveLength(4);
    expect(chat.messages[2].role).toBe('user');
    expect(chat.messages[3].role).toBe('assistant');
    expect(chat.messages[3].tools).toHaveLength(1);
  });

  it('deduplicates tool IDs across merged messages', () => {
    chat.loadHistory([
      {
        id: 'msg-1', role: 'assistant', content: 'Working', timestamp: 1000,
        tools: [{ id: 'dup-id', toolType: 'Read', target: 'a.txt' }],
      },
      {
        id: 'msg-2', role: 'assistant', content: '', timestamp: 1001,
        tools: [{ id: 'dup-id', toolType: 'Write', target: 'b.txt' }],
      },
    ]);

    // msg-2 merges into msg-1, creating duplicate dup-id
    expect(chat.messages).toHaveLength(1);
    const tools = chat.messages[0].tools!;
    expect(tools).toHaveLength(2);
    expect(tools[0].id).toBe('dup-id');
    expect(tools[1].id).toBe('dup-id__dup_1');
  });
});
