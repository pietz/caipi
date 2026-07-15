import type { StreamItem, ToolState } from '$lib/stores/chat.svelte';

type GroupKind = 'claude' | 'codex';

export type GroupedTextItem = { type: 'text'; content: string };
export type GroupedToolItem = { type: 'tool-group'; tools: ToolState[] };
export type GroupedSubagentItem = { type: 'subagent-group'; group: SubagentGroup };
export type GroupedItem = GroupedTextItem | GroupedToolItem | GroupedSubagentItem;

export interface SubagentGroup {
  id: string;
  kind: GroupKind;
  title: string;
  tools: ToolState[];
  launcherToolId: string;
  agentId?: string;
  status: 'running' | 'completed' | 'error';
}

type MutableSubagentGroup = Omit<SubagentGroup, 'status'>;

const RUNNING_STATUSES = new Set(['pending', 'awaiting_permission', 'running']);
const ERROR_STATUSES = new Set(['error']);
const CODEX_FOLLOW_UP_TOOLS = new Set(['send_input', 'wait', 'close_agent', 'resume_agent']);

function startOrder(tool: ToolState): number {
  return tool.startOrder ?? tool.insertionIndex;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((entry): entry is string => typeof entry === 'string');
}

function parseJsonString(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  try {
    return JSON.parse(value) as unknown;
  } catch {
    return value;
  }
}

function getToolInputRecord(tool: ToolState): Record<string, unknown> | null {
  return asRecord(parseJsonString(tool.input));
}

function parseSpawnedAgentId(tool: ToolState): string | undefined {
  const output = parseJsonString(tool.output);
  const outputRecord = asRecord(output);
  if (!outputRecord) return undefined;
  return asString(outputRecord.agent_id) ?? asString(outputRecord.agentId);
}

function getCodexLauncherTitle(tool: ToolState): string {
  if (tool.target.trim()) return tool.target;
  const input = getToolInputRecord(tool);
  return asString(input?.prompt) ?? asString(input?.message) ?? 'agent task';
}

function getCodexReceiverThreadIds(tool: ToolState): string[] {
  const input = getToolInputRecord(tool);
  if (!input) return [];

  const receiverThreadIds = asStringArray(input.receiverThreadIds);
  if (receiverThreadIds.length > 0) {
    return receiverThreadIds;
  }

  return asStringArray(input.receiver_thread_ids);
}

function getToolThreadId(tool: ToolState): string | undefined {
  const input = getToolInputRecord(tool);
  if (!input) return undefined;

  // `__threadId` is backend-added metadata so child-thread tools can be grouped
  // with their spawn card (works for both Codex 0.104 function_call and 0.105+ collab items).
  return asString(input.__threadId)
    ?? asString(input.senderThreadId)
    ?? asString(input.sender_thread_id)
    ?? asString(input.threadId)
    ?? asString(input.thread_id);
}

function getReceiverThreadIds(input: Record<string, unknown>): string[] {
  const ids = asStringArray(input.receiverThreadIds);
  if (ids.length > 0) return ids;
  return asStringArray(input.receiver_thread_ids);
}

function getCodexReferencedIds(tool: ToolState): string[] {
  const input = getToolInputRecord(tool);
  if (!input) return [];

  if (tool.toolType === 'wait') {
    const ids = asStringArray(input.ids);
    if (ids.length > 0) return ids;
    // Codex app-server may use receiverThreadIds instead of ids
    return getReceiverThreadIds(input);
  }

  if (tool.toolType === 'send_input' || tool.toolType === 'close_agent' || tool.toolType === 'resume_agent') {
    const id = asString(input.id);
    if (id) return [id];
    // Fallback: check receiverThreadIds
    return getReceiverThreadIds(input);
  }

  return [];
}

function getClaudeTaskCandidates(tool: ToolState, tasks: ToolState[]): ToolState[] {
  const toolStart = startOrder(tool);
  return tasks.filter((task) => {
    const taskStart = startOrder(task);
    if (taskStart >= toolStart) {
      return false;
    }
    if (task.endOrder !== undefined && toolStart > task.endOrder) {
      return false;
    }
    return true;
  });
}

function pushTool(group: MutableSubagentGroup, tool: ToolState) {
  if (!group.tools.some(existing => existing.id === tool.id)) {
    group.tools = [...group.tools, tool];
  }
}

function finalizeGroup(group: MutableSubagentGroup): SubagentGroup {
  const sortedTools = [...group.tools].sort((a, b) => a.insertionIndex - b.insertionIndex);
  const hasError = sortedTools.some(tool => ERROR_STATUSES.has(tool.status));
  const hasRunning = sortedTools.some(tool => RUNNING_STATUSES.has(tool.status));

  return {
    ...group,
    tools: sortedTools,
    status: hasRunning ? 'running' : (hasError ? 'error' : 'completed'),
  };
}

function inferSubagentGroups(orderedTools: ToolState[]) {
  const groups = new Map<string, MutableSubagentGroup>();
  const groupByToolId = new Map<string, string>();

  const taskTools = orderedTools
    .filter(tool => tool.toolType === 'Task')
    .sort((a, b) => startOrder(a) - startOrder(b));

  for (const task of taskTools) {
    const groupId = `claude-task-${task.id}`;
    groups.set(groupId, {
      id: groupId,
      kind: 'claude',
      title: task.target || 'agent',
      tools: [task],
      launcherToolId: task.id,
    });
    groupByToolId.set(task.id, groupId);
  }

  const taskLastAssignedOrder = new Map<string, number>(
    taskTools.map(task => [task.id, startOrder(task)])
  );

  for (const tool of orderedTools) {
    if (tool.toolType === 'Task') continue;
    const candidates = getClaudeTaskCandidates(tool, taskTools);
    if (candidates.length === 0) continue;

    const selectedTask = candidates.length === 1
      ? candidates[0]
      : [...candidates].sort((a, b) => {
          const aLastAssigned = taskLastAssignedOrder.get(a.id) ?? Number.NEGATIVE_INFINITY;
          const bLastAssigned = taskLastAssignedOrder.get(b.id) ?? Number.NEGATIVE_INFINITY;
          if (aLastAssigned !== bLastAssigned) {
            return aLastAssigned - bLastAssigned;
          }
          return startOrder(a) - startOrder(b);
        })[0];

    const groupId = `claude-task-${selectedTask.id}`;
    const group = groups.get(groupId);
    if (!group) continue;

    pushTool(group, tool);
    groupByToolId.set(tool.id, groupId);
    taskLastAssignedOrder.set(selectedTask.id, startOrder(tool));
  }

  const unresolvedSpawnGroupIds: string[] = [];
  const codexGroupByAgentId = new Map<string, string>();
  const codexGroupByThreadId = new Map<string, string>();

  for (const tool of orderedTools) {
    if (tool.toolType === 'spawn_agent') {
      const groupId = `codex-agent-${tool.id}`;
      const group: MutableSubagentGroup = {
        id: groupId,
        kind: 'codex',
        title: getCodexLauncherTitle(tool),
        tools: [tool],
        launcherToolId: tool.id,
      };

      const spawnedAgentId = parseSpawnedAgentId(tool);
      if (spawnedAgentId) {
        group.agentId = spawnedAgentId;
        codexGroupByAgentId.set(spawnedAgentId, groupId);
      } else {
        unresolvedSpawnGroupIds.push(groupId);
      }

      const receiverThreadIds = getCodexReceiverThreadIds(tool);
      for (const receiverThreadId of receiverThreadIds) {
        codexGroupByThreadId.set(receiverThreadId, groupId);
      }

      groups.set(groupId, group);
      groupByToolId.set(tool.id, groupId);
      continue;
    }

    if (CODEX_FOLLOW_UP_TOOLS.has(tool.toolType)) {
      const referencedIds = getCodexReferencedIds(tool);
      let resolvedGroupId: string | undefined;

      for (const referencedId of referencedIds) {
        let groupId = codexGroupByAgentId.get(referencedId) ?? codexGroupByThreadId.get(referencedId);

        if (!groupId) {
          const inferredGroupId = unresolvedSpawnGroupIds.shift();
          if (inferredGroupId) {
            groupId = inferredGroupId;
            codexGroupByAgentId.set(referencedId, inferredGroupId);
            codexGroupByThreadId.set(referencedId, inferredGroupId);
            const inferredGroup = groups.get(inferredGroupId);
            if (inferredGroup && !inferredGroup.agentId) {
              inferredGroup.agentId = referencedId;
            }
          }
        }

        if (!resolvedGroupId && groupId) {
          resolvedGroupId = groupId;
        }
      }

      if (!resolvedGroupId) {
        continue;
      }

      const group = groups.get(resolvedGroupId);
      if (!group) {
        continue;
      }

      pushTool(group, tool);
      groupByToolId.set(tool.id, resolvedGroupId);
      continue;
    }

    const threadId = getToolThreadId(tool);
    if (!threadId) {
      continue;
    }

    const groupId = codexGroupByThreadId.get(threadId);
    if (!groupId) {
      continue;
    }

    const group = groups.get(groupId);
    if (!group) {
      continue;
    }

    pushTool(group, tool);
    groupByToolId.set(tool.id, groupId);
  }

  const finalizedGroups = new Map<string, SubagentGroup>();
  for (const [groupId, group] of groups) {
    finalizedGroups.set(groupId, finalizeGroup(group));
  }

  // Debug: log grouping results
  if (finalizedGroups.size > 0 || orderedTools.some(t => t.toolType === 'spawn_agent')) {
    console.debug('[subagent-grouping] groups:', [...finalizedGroups.entries()].map(([id, g]) => ({
      id, kind: g.kind, title: g.title.slice(0, 40), toolCount: g.tools.length,
      toolIds: g.tools.map(t => `${t.id}(${t.toolType})`),
    })));
    console.debug('[subagent-grouping] ungrouped tools:', orderedTools
      .filter(t => !groupByToolId.has(t.id))
      .map(t => {
        const input = typeof t.input === 'string' ? JSON.parse(t.input) : t.input;
        return `${t.id}(${t.toolType}) __threadId=${input?.__threadId}`;
      }));
  }

  return { finalizedGroups, groupByToolId };
}

export function buildGroupedStreamItems(
  sortedStreamItems: StreamItem[],
  getTool: (toolId: string) => ToolState | undefined,
): GroupedItem[] {
  const groups: GroupedItem[] = [];
  const currentToolGroup: ToolState[] = [];
  const emittedSubagentGroups = new Set<string>();

  const orderedTools = sortedStreamItems
    .filter((item): item is StreamItem & { toolId: string } => item.type === 'tool' && !!item.toolId)
    .map(item => getTool(item.toolId))
    .filter((tool): tool is ToolState => !!tool);

  const { finalizedGroups, groupByToolId } = inferSubagentGroups(orderedTools);

  const flushCurrentToolGroup = () => {
    if (currentToolGroup.length === 0) {
      return;
    }
    groups.push({ type: 'tool-group', tools: [...currentToolGroup] });
    currentToolGroup.length = 0;
  };

  for (const item of sortedStreamItems) {
    if (item.type === 'tool' && item.toolId) {
      const tool = getTool(item.toolId);
      if (!tool) {
        continue;
      }

      const groupId = groupByToolId.get(tool.id);
      if (groupId) {
        flushCurrentToolGroup();
        if (!emittedSubagentGroups.has(groupId)) {
          const subagentGroup = finalizedGroups.get(groupId);
          if (subagentGroup) {
            groups.push({ type: 'subagent-group', group: subagentGroup });
            emittedSubagentGroups.add(groupId);
          }
        }
        continue;
      }

      currentToolGroup.push(tool);
      continue;
    }

    if (item.type === 'text' && item.content) {
      flushCurrentToolGroup();
      groups.push({ type: 'text', content: item.content });
    }
  }

  flushCurrentToolGroup();
  return groups;
}
