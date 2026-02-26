import { describe, expect, it } from 'vitest';
import type { StreamItem, ToolState } from '$lib/stores/chat.svelte';
import { buildGroupedStreamItems } from './subagent-grouping';

function makeTool(overrides: Partial<ToolState> & Pick<ToolState, 'id' | 'toolType' | 'status' | 'insertionIndex'>): ToolState {
  return {
    id: overrides.id,
    toolType: overrides.toolType,
    target: overrides.target ?? '',
    status: overrides.status,
    timestamp: overrides.timestamp ?? 1,
    insertionIndex: overrides.insertionIndex,
    startOrder: overrides.startOrder ?? overrides.insertionIndex,
    endOrder: overrides.endOrder,
    input: overrides.input,
    output: overrides.output,
    permissionRequestId: overrides.permissionRequestId,
  };
}

function makeToolItem(toolId: string, insertionIndex: number): StreamItem {
  return {
    id: `stream-tool-${toolId}`,
    type: 'tool',
    toolId,
    timestamp: 1,
    insertionIndex,
  };
}

function makeTextItem(content: string, insertionIndex: number): StreamItem {
  return {
    id: `stream-text-${insertionIndex}`,
    type: 'text',
    content,
    timestamp: 1,
    insertionIndex,
  };
}

describe('buildGroupedStreamItems', () => {
  it('creates an inline codex subagent group and assigns follow-up tools', () => {
    const tools = new Map<string, ToolState>([
      ['tool-spawn', makeTool({
        id: 'tool-spawn',
        toolType: 'spawn_agent',
        status: 'completed',
        insertionIndex: 0,
        input: { message: 'Research release notes' },
        output: { agent_id: 'agent-1' },
      })],
      ['tool-wait', makeTool({
        id: 'tool-wait',
        toolType: 'wait',
        status: 'running',
        insertionIndex: 2,
        input: { ids: ['agent-1'] },
      })],
      ['tool-send', makeTool({
        id: 'tool-send',
        toolType: 'send_input',
        status: 'completed',
        insertionIndex: 3,
        input: { id: 'agent-1' },
      })],
    ]);

    const streamItems: StreamItem[] = [
      makeToolItem('tool-spawn', 0),
      makeTextItem('working...\n', 1),
      makeToolItem('tool-wait', 2),
      makeToolItem('tool-send', 3),
    ];

    const grouped = buildGroupedStreamItems(streamItems, (toolId) => tools.get(toolId));
    expect(grouped).toHaveLength(2);
    expect(grouped[0]?.type).toBe('subagent-group');
    if (grouped[0]?.type === 'subagent-group') {
      expect(grouped[0].group.kind).toBe('codex');
      expect(grouped[0].group.status).toBe('running');
      expect(grouped[0].group.tools.map(tool => tool.id)).toEqual(['tool-spawn', 'tool-wait', 'tool-send']);
    }
    expect(grouped[1]).toEqual({ type: 'text', content: 'working...\n' });
  });

  it('assigns codex child-thread tools to the corresponding spawn group', () => {
    const tools = new Map<string, ToolState>([
      ['tool-spawn', makeTool({
        id: 'tool-spawn',
        toolType: 'spawn_agent',
        status: 'running',
        insertionIndex: 0,
        input: {
          prompt: 'Investigate failing tests',
          senderThreadId: 'thread-parent',
          receiverThreadIds: ['thread-child-a'],
        },
      })],
      ['tool-child-read', makeTool({
        id: 'tool-child-read',
        toolType: 'Read',
        status: 'completed',
        insertionIndex: 1,
        input: { __threadId: 'thread-child-a' },
      })],
      ['tool-parent-bash', makeTool({
        id: 'tool-parent-bash',
        toolType: 'Bash',
        status: 'completed',
        insertionIndex: 2,
        input: { __threadId: 'thread-parent' },
      })],
      ['tool-child-edit', makeTool({
        id: 'tool-child-edit',
        toolType: 'Edit',
        status: 'running',
        insertionIndex: 3,
        input: { __threadId: 'thread-child-a' },
      })],
    ]);

    const streamItems: StreamItem[] = [
      makeToolItem('tool-spawn', 0),
      makeToolItem('tool-child-read', 1),
      makeToolItem('tool-parent-bash', 2),
      makeToolItem('tool-child-edit', 3),
    ];

    const grouped = buildGroupedStreamItems(streamItems, (toolId) => tools.get(toolId));
    expect(grouped).toHaveLength(2);
    expect(grouped[0]?.type).toBe('subagent-group');
    if (grouped[0]?.type === 'subagent-group') {
      expect(grouped[0].group.tools.map(tool => tool.id)).toEqual([
        'tool-spawn',
        'tool-child-read',
        'tool-child-edit',
      ]);
    }
    expect(grouped[1]?.type).toBe('tool-group');
    if (grouped[1]?.type === 'tool-group') {
      expect(grouped[1].tools.map(tool => tool.id)).toEqual(['tool-parent-bash']);
    }
  });

  it('groups claude tools between Task start and end boundaries', () => {
    const tools = new Map<string, ToolState>([
      ['task', makeTool({
        id: 'task',
        toolType: 'Task',
        status: 'completed',
        target: 'Explore codebase',
        insertionIndex: 0,
        startOrder: 0,
        endOrder: 3,
      })],
      ['inner-read', makeTool({
        id: 'inner-read',
        toolType: 'Read',
        status: 'completed',
        target: '/tmp/file',
        insertionIndex: 1,
        startOrder: 1,
      })],
      ['outer-bash', makeTool({
        id: 'outer-bash',
        toolType: 'Bash',
        status: 'running',
        target: 'npm test',
        insertionIndex: 2,
        startOrder: 4,
      })],
    ]);

    const streamItems: StreamItem[] = [
      makeToolItem('task', 0),
      makeToolItem('inner-read', 1),
      makeToolItem('outer-bash', 2),
    ];

    const grouped = buildGroupedStreamItems(streamItems, (toolId) => tools.get(toolId));
    expect(grouped).toHaveLength(2);
    expect(grouped[0]?.type).toBe('subagent-group');
    if (grouped[0]?.type === 'subagent-group') {
      expect(grouped[0].group.kind).toBe('claude');
      expect(grouped[0].group.tools.map(tool => tool.id)).toEqual(['task', 'inner-read']);
    }
    expect(grouped[1]?.type).toBe('tool-group');
    if (grouped[1]?.type === 'tool-group') {
      expect(grouped[1].tools.map(tool => tool.id)).toEqual(['outer-bash']);
    }
  });

  it('keeps overlapping claude agent tools separated between agents', () => {
    const tools = new Map<string, ToolState>([
      ['task-a', makeTool({
        id: 'task-a',
        toolType: 'Task',
        status: 'running',
        target: 'Agent A',
        insertionIndex: 0,
        startOrder: 0,
        endOrder: 10,
      })],
      ['task-b', makeTool({
        id: 'task-b',
        toolType: 'Task',
        status: 'running',
        target: 'Agent B',
        insertionIndex: 1,
        startOrder: 1,
        endOrder: 11,
      })],
      ['read-a1', makeTool({
        id: 'read-a1',
        toolType: 'Read',
        status: 'completed',
        insertionIndex: 2,
        startOrder: 2,
      })],
      ['read-b1', makeTool({
        id: 'read-b1',
        toolType: 'Read',
        status: 'completed',
        insertionIndex: 3,
        startOrder: 3,
      })],
      ['edit-a2', makeTool({
        id: 'edit-a2',
        toolType: 'Edit',
        status: 'running',
        insertionIndex: 4,
        startOrder: 4,
      })],
    ]);

    const streamItems: StreamItem[] = [
      makeToolItem('task-a', 0),
      makeToolItem('task-b', 1),
      makeToolItem('read-a1', 2),
      makeToolItem('read-b1', 3),
      makeToolItem('edit-a2', 4),
    ];

    const grouped = buildGroupedStreamItems(streamItems, (toolId) => tools.get(toolId));
    expect(grouped).toHaveLength(2);
    expect(grouped[0]?.type).toBe('subagent-group');
    expect(grouped[1]?.type).toBe('subagent-group');

    if (grouped[0]?.type === 'subagent-group') {
      expect(grouped[0].group.title).toBe('Agent A');
      expect(grouped[0].group.tools.map(tool => tool.id)).toEqual(['task-a', 'read-a1', 'edit-a2']);
    }
    if (grouped[1]?.type === 'subagent-group') {
      expect(grouped[1].group.title).toBe('Agent B');
      expect(grouped[1].group.tools.map(tool => tool.id)).toEqual(['task-b', 'read-b1']);
    }
  });

  it('keeps standard consecutive grouping when no subagent tools exist', () => {
    const tools = new Map<string, ToolState>([
      ['read', makeTool({ id: 'read', toolType: 'Read', status: 'completed', insertionIndex: 0 })],
      ['write', makeTool({ id: 'write', toolType: 'Write', status: 'completed', insertionIndex: 1 })],
      ['bash', makeTool({ id: 'bash', toolType: 'Bash', status: 'running', insertionIndex: 3 })],
    ]);

    const streamItems: StreamItem[] = [
      makeToolItem('read', 0),
      makeToolItem('write', 1),
      makeTextItem('done\n', 2),
      makeToolItem('bash', 3),
    ];

    const grouped = buildGroupedStreamItems(streamItems, (toolId) => tools.get(toolId));
    expect(grouped).toHaveLength(3);
    expect(grouped[0]?.type).toBe('tool-group');
    if (grouped[0]?.type === 'tool-group') {
      expect(grouped[0].tools.map(tool => tool.id)).toEqual(['read', 'write']);
    }
    expect(grouped[1]).toEqual({ type: 'text', content: 'done\n' });
    expect(grouped[2]?.type).toBe('tool-group');
  });
});
