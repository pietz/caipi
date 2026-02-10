import type { ChatEvent } from '$lib/utils/events';

// --- Factory Functions ---

export function makeTextEvent(content: string): ChatEvent {
	return { type: 'Text', content };
}

export function makeToolStartEvent(
	overrides?: Partial<{
		toolUseId: string;
		toolType: string;
		target: string;
		status: string;
		input: Record<string, unknown>;
	}>
): ChatEvent {
	return {
		type: 'ToolStart',
		toolUseId: overrides?.toolUseId ?? 'tool-1',
		toolType: overrides?.toolType ?? 'Read',
		target: overrides?.target ?? '/file.txt',
		status: overrides?.status ?? 'pending',
		...(overrides?.input !== undefined ? { input: overrides.input } : {})
	};
}

export function makeToolStatusUpdate(
	toolUseId: string,
	status: string,
	permissionRequestId?: string
): ChatEvent {
	return {
		type: 'ToolStatusUpdate',
		toolUseId,
		status,
		...(permissionRequestId !== undefined ? { permissionRequestId } : {})
	};
}

export function makeToolEndEvent(id: string, status: string): ChatEvent {
	return { type: 'ToolEnd', id, status };
}

export function makeCompleteEvent(): ChatEvent {
	return { type: 'Complete' };
}

export function makeAbortCompleteEvent(sessionId: string = 'session-1'): ChatEvent {
	return { type: 'AbortComplete', sessionId };
}

export function makeTokenUsageEvent(
	total: number,
	context?: number,
	window?: number
): ChatEvent {
	return {
		type: 'TokenUsage',
		totalTokens: total,
		...(context !== undefined ? { contextTokens: context } : {}),
		...(window !== undefined ? { contextWindow: window } : {})
	};
}

export function makeThinkingStartEvent(
	id: string = 'think-1',
	content: string = ''
): ChatEvent {
	return { type: 'ThinkingStart', thinkingId: id, content };
}

export function makeErrorEvent(msg: string): ChatEvent {
	return { type: 'Error', message: msg };
}

// --- Pre-built Scenarios ---

export function simpleTextScenario(): ChatEvent[] {
	return [makeTextEvent('Hello '), makeTextEvent('world\n'), makeCompleteEvent()];
}

export function toolLifecycleScenario(): ChatEvent[] {
	return [
		makeTextEvent('Let me read the file.\n'),
		makeToolStartEvent({ toolUseId: 'tool-1', toolType: 'Read', target: '/file.txt', status: 'pending' }),
		makeToolStatusUpdate('tool-1', 'running'),
		makeToolEndEvent('tool-1', 'completed'),
		makeTextEvent('File contents look good.\n'),
		makeCompleteEvent()
	];
}

export function permissionFlowScenario(): ChatEvent[] {
	return [
		makeTextEvent('I need to edit a file.\n'),
		makeToolStartEvent({
			toolUseId: 'tool-1',
			toolType: 'Edit',
			target: '/src/main.ts',
			status: 'pending',
			input: { file_path: '/src/main.ts' }
		}),
		makeToolStatusUpdate('tool-1', 'awaiting_permission', 'perm-1'),
		makeToolStatusUpdate('tool-1', 'running'),
		makeToolEndEvent('tool-1', 'completed'),
		makeCompleteEvent()
	];
}

export function permissionDeniedScenario(): ChatEvent[] {
	return [
		makeTextEvent("I'll run this command.\n"),
		makeToolStartEvent({
			toolUseId: 'tool-1',
			toolType: 'Bash',
			target: 'rm -rf /',
			status: 'pending'
		}),
		makeToolStatusUpdate('tool-1', 'awaiting_permission', 'perm-1'),
		makeToolEndEvent('tool-1', 'denied'),
		makeCompleteEvent()
	];
}

export function multiToolScenario(): ChatEvent[] {
	return [
		makeTextEvent('Working on it.\n'),
		makeToolStartEvent({ toolUseId: 'tool-1', toolType: 'Read', target: '/a.txt', status: 'pending' }),
		makeToolEndEvent('tool-1', 'completed'),
		makeToolStartEvent({ toolUseId: 'tool-2', toolType: 'Write', target: '/b.txt', status: 'pending' }),
		makeToolEndEvent('tool-2', 'completed'),
		makeTextEvent('All done.\n'),
		makeToolStartEvent({ toolUseId: 'tool-3', toolType: 'Bash', target: 'npm test', status: 'pending' }),
		makeToolEndEvent('tool-3', 'completed'),
		makeCompleteEvent()
	];
}

export function thinkingWithToolsScenario(): ChatEvent[] {
	return [
		makeThinkingStartEvent('think-1', 'Let me consider the approach...'),
		makeTextEvent("Here's my plan.\n"),
		makeToolStartEvent({ toolUseId: 'tool-1', toolType: 'Read', target: '/config.json', status: 'pending' }),
		makeToolEndEvent('tool-1', 'completed'),
		makeTextEvent('Done reading.\n'),
		makeCompleteEvent()
	];
}

export function subagentScenario(): ChatEvent[] {
	return [
		makeTextEvent('Setting up tasks.\n'),
		makeToolStartEvent({ toolUseId: 'tool-skill', toolType: 'Skill', target: 'pdf', status: 'pending', input: {} }),
		makeToolEndEvent('tool-skill', 'completed'),
		makeToolStartEvent({
			toolUseId: 'tool-todo',
			toolType: 'TodoWrite',
			target: '',
			status: 'pending',
			input: {
				todos: [
					{ id: '1', content: 'Task A', status: 'pending' },
					{ id: '2', content: 'Task B', status: 'completed' }
				]
			}
		}),
		makeToolEndEvent('tool-todo', 'completed'),
		makeToolStartEvent({
			toolUseId: 'tool-create',
			toolType: 'TaskCreate',
			target: '',
			status: 'pending',
			input: { subject: 'New task' }
		}),
		makeToolEndEvent('tool-create', 'completed'),
		makeToolStartEvent({
			toolUseId: 'tool-update',
			toolType: 'TaskUpdate',
			target: '',
			status: 'pending',
			input: { taskId: '1', status: 'completed' }
		}),
		makeToolEndEvent('tool-update', 'completed'),
		makeCompleteEvent()
	];
}

export function abortMidStreamScenario(): ChatEvent[] {
	return [
		makeTextEvent('Starting work.\n'),
		makeToolStartEvent({ toolUseId: 'tool-1', toolType: 'Write', target: '/out.txt', status: 'pending' }),
		makeToolStatusUpdate('tool-1', 'running'),
		makeAbortCompleteEvent('session-1')
	];
}

export function errorMidStreamScenario(): ChatEvent[] {
	return [
		makeTextEvent('Beginning.\n'),
		makeToolStartEvent({ toolUseId: 'tool-1', toolType: 'Read', target: '/missing.txt', status: 'pending' }),
		makeToolStatusUpdate('tool-1', 'running'),
		makeErrorEvent('Session terminated unexpectedly')
	];
}

export function tokenUsageScenario(): ChatEvent[] {
	return [
		makeTokenUsageEvent(1000, 800, 200000),
		makeTextEvent('First response.\n'),
		makeTokenUsageEvent(2000, 1600, 200000),
		makeTextEvent('More output.\n'),
		makeTokenUsageEvent(3000, 2400, 200000),
		makeCompleteEvent()
	];
}

export function sessionGatingScenario(): {
	matchingEvents: ChatEvent[];
	wrongSessionEvents: ChatEvent[];
	wrongTurnEvents: ChatEvent[];
} {
	return {
		matchingEvents: [
			{ ...makeTextEvent('Matching text.\n'), sessionId: 'session-1', turnId: 'turn-1' },
			{ ...makeCompleteEvent(), sessionId: 'session-1', turnId: 'turn-1' }
		],
		wrongSessionEvents: [
			{ ...makeTextEvent('Wrong session text.\n'), sessionId: 'session-2', turnId: 'turn-1' },
			{ ...makeCompleteEvent(), sessionId: 'session-2', turnId: 'turn-1' }
		],
		wrongTurnEvents: [
			{ ...makeTextEvent('Wrong turn text.\n'), sessionId: 'session-1', turnId: 'turn-2' },
			{ ...makeCompleteEvent(), sessionId: 'session-1', turnId: 'turn-2' }
		]
	};
}
