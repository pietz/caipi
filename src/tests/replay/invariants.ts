import { chat } from '$lib/stores/chat.svelte';

export interface Violation {
  invariant: string;
  message: string;
  eventIndex: number;
}

export interface StateSnapshot {
  messages: any[];
  streamItems: any[];
  tools: Map<string, any>;
  isStreaming: boolean;
  tokenCount: number;
  contextWindow: number | null;
  todos: any[];
  activeSkills: string[];
}

export type InvariantChecker = (
  snapshot: StateSnapshot,
  eventIndex: number,
  toolHistory: Map<string, string[]>,
) => Violation[];

type ToolStatus =
  | 'pending'
  | 'awaiting_permission'
  | 'running'
  | 'completed'
  | 'error'
  | 'denied'
  | 'aborted'
  | 'history';

const VALID_TRANSITIONS: Record<string, Set<string>> = {
  pending: new Set([
    'running',
    'awaiting_permission',
    'completed',
    'error',
    'denied',
    'aborted',
  ]),
  awaiting_permission: new Set(['running', 'denied', 'aborted']),
  running: new Set(['completed', 'error', 'aborted']),
};

let lastSeenTokenCount = 0;

export function resetInvariantState(): void {
  lastSeenTokenCount = 0;
}

export const checkToolLifecycleOrdering: InvariantChecker = (
  snapshot,
  eventIndex,
  toolHistory,
) => {
  const violations: Violation[] = [];

  for (const [toolId, tool] of snapshot.tools) {
    const status: ToolStatus = tool.status;
    const history = toolHistory.get(toolId);

    if (!history) {
      toolHistory.set(toolId, [status]);
      continue;
    }

    const lastStatus = history[history.length - 1];
    if (lastStatus === status) continue;

    const allowed = VALID_TRANSITIONS[lastStatus];
    if (!allowed || !allowed.has(status)) {
      violations.push({
        invariant: 'tool-lifecycle-ordering',
        message: `Tool ${toolId}: invalid transition ${lastStatus} → ${status}`,
        eventIndex,
      });
    }

    history.push(status);
  }

  return violations;
};

export const checkTokenMonotonicity: InvariantChecker = (
  snapshot,
  eventIndex,
) => {
  const violations: Violation[] = [];
  const current = snapshot.tokenCount;

  if (current < lastSeenTokenCount) {
    violations.push({
      invariant: 'token-monotonicity',
      message: `tokenCount decreased from ${lastSeenTokenCount} to ${current}`,
      eventIndex,
    });
  }

  lastSeenTokenCount = current;
  return violations;
};

export const checkPostFinalizeCleanup: InvariantChecker = (
  snapshot,
  eventIndex,
) => {
  const violations: Violation[] = [];

  if (snapshot.streamItems.length === 0 && snapshot.tools.size === 0) {
    return violations;
  }

  if (snapshot.streamItems.length === 0) {
    const TERMINAL: Set<string> = new Set([
      'completed',
      'error',
      'denied',
      'aborted',
      'history',
    ]);
    for (const [toolId, tool] of snapshot.tools) {
      if (!TERMINAL.has(tool.status)) {
        violations.push({
          invariant: 'post-finalize-cleanup',
          message: `streamItems cleared but tool ${toolId} still in non-terminal status "${tool.status}"`,
          eventIndex,
        });
      }
    }
  }

  return violations;
};

export const checkPermissionConsistency: InvariantChecker = (
  snapshot,
  eventIndex,
) => {
  const violations: Violation[] = [];

  for (const [toolId, tool] of snapshot.tools) {
    if (tool.status === 'awaiting_permission' && !tool.permissionRequestId) {
      violations.push({
        invariant: 'permission-consistency',
        message: `Tool ${toolId} is awaiting_permission but has no permissionRequestId`,
        eventIndex,
      });
    }

    // Note: denied tools may still have permissionRequestId if denied via
    // ToolEnd (which doesn't clear extras). Only ToolStatusUpdate with
    // explicit null clears it. This is expected behavior.
  }

  return violations;
};

export function runAllInvariants(
  snapshot: StateSnapshot,
  eventIndex: number,
  toolHistory: Map<string, string[]>,
): Violation[] {
  return [
    ...checkToolLifecycleOrdering(snapshot, eventIndex, toolHistory),
    ...checkTokenMonotonicity(snapshot, eventIndex, toolHistory),
    ...checkPostFinalizeCleanup(snapshot, eventIndex, toolHistory),
    ...checkPermissionConsistency(snapshot, eventIndex, toolHistory),
  ];
}

export function captureSnapshot(): StateSnapshot {
  return {
    messages: [...chat.messages],
    streamItems: [...chat.streamItems],
    tools: new Map(chat.tools),
    isStreaming: chat.isStreaming,
    tokenCount: chat.tokenCount,
    contextWindow: chat.contextWindow,
    todos: [...chat.todos],
    activeSkills: [...chat.activeSkills],
  };
}
