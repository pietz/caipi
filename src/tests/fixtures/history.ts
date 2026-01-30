import type { HistoryMessage, HistoryTool } from '$lib/api/types';

export function makeHistoryTool(overrides: Partial<HistoryTool> = {}): HistoryTool {
  return {
    id: 'tool-1',
    toolType: 'Read',
    target: 'file.txt',
    ...overrides,
  };
}

export function makeHistoryMessage(overrides: Partial<HistoryMessage> = {}): HistoryMessage {
  return {
    id: 'msg-1',
    role: 'assistant',
    content: '',
    timestamp: 1700000000,
    tools: [],
    ...overrides,
  };
}
