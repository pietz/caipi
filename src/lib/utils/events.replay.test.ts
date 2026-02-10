import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { replay } from '../../tests/replay/harness';
import { chat } from '$lib/stores/chat.svelte';
import {
	simpleTextScenario,
	toolLifecycleScenario,
	permissionFlowScenario,
	permissionDeniedScenario,
	multiToolScenario,
	thinkingWithToolsScenario,
	subagentScenario,
	abortMidStreamScenario,
	errorMidStreamScenario,
	tokenUsageScenario,
	sessionGatingScenario,
} from '../../tests/fixtures/events';

// Mock Tauri invoke (needed by app store's api imports)
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

describe('Event Replay Scenarios', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
		chat.reset();
	});

	describe('Invariant checks', () => {
		it('simpleTextScenario has no invariant violations', () => {
			const { violations } = replay(simpleTextScenario());
			expect(violations).toHaveLength(0);
		});

		it('toolLifecycleScenario has no invariant violations', () => {
			const { violations } = replay(toolLifecycleScenario());
			expect(violations).toHaveLength(0);
		});

		it('permissionFlowScenario has no invariant violations', () => {
			const { violations } = replay(permissionFlowScenario());
			expect(violations).toHaveLength(0);
		});

		it('permissionDeniedScenario has no invariant violations', () => {
			const { violations } = replay(permissionDeniedScenario());
			expect(violations).toHaveLength(0);
		});

		it('multiToolScenario has no invariant violations', () => {
			const { violations } = replay(multiToolScenario());
			expect(violations).toHaveLength(0);
		});

		it('abortMidStreamScenario has no invariant violations', () => {
			const { violations } = replay(abortMidStreamScenario());
			expect(violations).toHaveLength(0);
		});

		it('errorMidStreamScenario has no invariant violations', () => {
			const { violations } = replay(errorMidStreamScenario());
			expect(violations).toHaveLength(0);
		});

		it('tokenUsageScenario has no invariant violations', () => {
			const { violations } = replay(tokenUsageScenario());
			expect(violations).toHaveLength(0);
		});
	});

	describe('Context usage / token tracking', () => {
		it('tracks token counts through multiple TokenUsage events', () => {
			const { snapshots, final } = replay(tokenUsageScenario());

			// After first TokenUsage(1000, 800, 200000) - event index 0
			expect(snapshots[0].tokenCount).toBe(800);
			expect(snapshots[0].contextWindow).toBe(200000);

			// After second TokenUsage - event index 2
			expect(snapshots[2].tokenCount).toBe(1600);

			// After third TokenUsage - event index 4
			expect(snapshots[4].tokenCount).toBe(2400);

			// Final state after Complete preserves token data
			expect(final.tokenCount).toBe(2400);
			expect(final.contextWindow).toBe(200000);
		});
	});

	describe('Permission flow', () => {
		it('tracks tool through full permission grant lifecycle', () => {
			// permissionFlowScenario events:
			// 0: Text, 1: ToolStart, 2: StatusUpdate(awaiting_permission), 3: StatusUpdate(running), 4: ToolEnd, 5: Complete
			const { snapshots } = replay(permissionFlowScenario());

			// After ToolStart (index 1): tool exists with pending status
			const toolAfterStart = snapshots[1].tools.get('tool-1');
			expect(toolAfterStart).toBeDefined();
			expect(toolAfterStart?.status).toBe('pending');

			// After awaiting_permission update (index 2): permissionRequestId set
			const toolAfterPerm = snapshots[2].tools.get('tool-1');
			expect(toolAfterPerm?.status).toBe('awaiting_permission');
			expect(toolAfterPerm?.permissionRequestId).toBe('perm-1');

			// After running update (index 3): permissionRequestId cleared
			const toolAfterRunning = snapshots[3].tools.get('tool-1');
			expect(toolAfterRunning?.status).toBe('running');
			expect(toolAfterRunning?.permissionRequestId).toBeUndefined();

			// After ToolEnd (index 4): status is completed
			const toolAfterEnd = snapshots[4].tools.get('tool-1');
			expect(toolAfterEnd?.status).toBe('completed');

			// After Complete (index 5): tools map is empty (finalized)
			expect(snapshots[5].tools.size).toBe(0);
		});

		it('handles permission denied flow', () => {
			// permissionDeniedScenario events:
			// 0: Text, 1: ToolStart, 2: StatusUpdate(awaiting_permission), 3: ToolEnd(denied), 4: Complete
			const { snapshots } = replay(permissionDeniedScenario());

			// After ToolEnd with denied status (index 3)
			const toolAfterDenied = snapshots[3].tools.get('tool-1');
			expect(toolAfterDenied?.status).toBe('denied');

			// After Complete (index 4): finalized, tools map empty
			expect(snapshots[4].tools.size).toBe(0);
			expect(snapshots[4].messages.length).toBeGreaterThan(0);
		});
	});

	describe('Tool lifespan / display', () => {
		it('tracks tool lifecycle intermediate states', () => {
			// toolLifecycleScenario events:
			// 0: Text, 1: ToolStart, 2: StatusUpdate(running), 3: ToolEnd(completed), 4: Text, 5: Complete
			const { snapshots } = replay(toolLifecycleScenario());

			// After ToolStart (index 1)
			expect(snapshots[1].tools.size).toBe(1);
			expect(snapshots[1].tools.get('tool-1')?.status).toBe('pending');

			// After StatusUpdate(running) (index 2)
			expect(snapshots[2].tools.get('tool-1')?.status).toBe('running');

			// After ToolEnd(completed) (index 3)
			expect(snapshots[3].tools.get('tool-1')?.status).toBe('completed');

			// After Complete (index 5): finalized
			// Finalize splits at text-after-tool boundary: msg1 = text + [tool], msg2 = trailing text
			expect(snapshots[5].tools.size).toBe(0);
			expect(snapshots[5].messages.length).toBe(2);
		});

		it('produces correct message grouping in multiToolScenario', () => {
			// multiToolScenario events:
			// 0: Text("Working on it.\n"), 1: ToolStart(tool-1), 2: ToolEnd(tool-1),
			// 3: ToolStart(tool-2), 4: ToolEnd(tool-2), 5: Text("All done.\n"),
			// 6: ToolStart(tool-3), 7: ToolEnd(tool-3), 8: Complete
			const { final } = replay(multiToolScenario());

			// After finalize, check messages
			// The finalize algorithm: text -> tool1 -> tool2 -> text (flush!) -> tool3 -> Complete (flush!)
			// Result: 2 messages
			expect(final.messages).toHaveLength(2);

			// First message: "Working on it.\n" with tool-1 and tool-2
			expect(final.messages[0].content).toBe('Working on it.\n');
			expect(final.messages[0].tools).toHaveLength(2);
			expect(final.messages[0].tools![0].id).toBe('tool-1');
			expect(final.messages[0].tools![1].id).toBe('tool-2');

			// Second message: "All done.\n" with tool-3
			expect(final.messages[1].content).toBe('All done.\n');
			expect(final.messages[1].tools).toHaveLength(1);
			expect(final.messages[1].tools![0].id).toBe('tool-3');
		});
	});

	describe('Subagent / skill display', () => {
		it('tracks skill activation and todo management', () => {
			// subagentScenario events:
			// 0: Text, 1: ToolStart(Skill), 2: ToolEnd(Skill),
			// 3: ToolStart(TodoWrite), 4: ToolEnd(TodoWrite),
			// 5: ToolStart(TaskCreate), 6: ToolEnd(TaskCreate),
			// 7: ToolStart(TaskUpdate), 8: ToolEnd(TaskUpdate),
			// 9: Complete
			const { snapshots, final } = replay(subagentScenario());

			// After Skill ToolEnd (index 2): activeSkills contains 'pdf'
			expect(snapshots[2].activeSkills).toContain('pdf');

			// After TodoWrite ToolEnd (index 4): todos populated
			expect(snapshots[4].todos).toHaveLength(2);
			expect(snapshots[4].todos[0].text).toBe('Task A');
			expect(snapshots[4].todos[0].done).toBe(false);
			expect(snapshots[4].todos[1].text).toBe('Task B');
			expect(snapshots[4].todos[1].done).toBe(true);

			// After TaskCreate ToolEnd (index 6): new todo added
			expect(snapshots[6].todos).toHaveLength(3);
			expect(snapshots[6].todos[2].text).toBe('New task');
			expect(snapshots[6].todos[2].active).toBe(true);

			// After TaskUpdate ToolEnd (index 8): todo '1' updated
			const todoOne = snapshots[8].todos.find((t) => t.id === '1');
			expect(todoOne?.done).toBe(true);
		});
	});

	describe('Text streaming', () => {
		it('flushes buffer on Complete', () => {
			const { final } = replay(simpleTextScenario());

			// simpleTextScenario: Text("Hello "), Text("world\n"), Complete
			// After finalize, should have 1 message with full text
			expect(final.messages).toHaveLength(1);
			expect(final.messages[0].content).toBe('Hello world\n');
			expect(final.messages[0].role).toBe('assistant');
		});

		it('flushes buffer before ToolStart', () => {
			// toolLifecycleScenario: Text -> ToolStart -> ... -> Complete
			const { snapshots } = replay(toolLifecycleScenario());

			// After ToolStart (index 1): the preceding text should have been flushed
			// Check streamItems: should have text item then tool item
			expect(snapshots[1].streamItems.length).toBeGreaterThanOrEqual(2);
			expect(snapshots[1].streamItems[0].type).toBe('text');
			expect(snapshots[1].streamItems[1].type).toBe('tool');
		});
	});

	describe('Abort / error recovery', () => {
		it('finalizes on abort with running tools becoming aborted', () => {
			// abortMidStreamScenario: Text, ToolStart, StatusUpdate(running), AbortComplete
			const { final } = replay(abortMidStreamScenario());

			// After AbortComplete: finalized
			expect(final.tools.size).toBe(0);
			expect(final.streamItems).toHaveLength(0);
			expect(final.isStreaming).toBe(false);

			// Messages should contain the finalized content
			expect(final.messages.length).toBeGreaterThan(0);

			// The running tool should be aborted in the finalized message
			const lastMsg = final.messages[final.messages.length - 1];
			const abortedTool = lastMsg.tools?.find((t: any) => t.id === 'tool-1');
			expect(abortedTool?.status).toBe('aborted');
		});

		it('creates error message on error event', () => {
			// errorMidStreamScenario: Text, ToolStart, StatusUpdate(running), Error
			const { final } = replay(errorMidStreamScenario());

			// Should have finalized assistant message + error message
			expect(final.messages.length).toBeGreaterThanOrEqual(2);

			// Last message should be the error
			const errorMsg = final.messages[final.messages.length - 1];
			expect(errorMsg.role).toBe('error');
			expect(errorMsg.content).toBe('Session terminated unexpectedly');
		});
	});

	describe('Session/turn gating', () => {
		it('ignores events with wrong sessionId', () => {
			const scenario = sessionGatingScenario();

			// Set up session with matching ID
			const { final: matchResult } = replay(scenario.matchingEvents, {
				sessionId: 'session-1',
				turnId: 'turn-1',
			});
			expect(matchResult.messages.length).toBeGreaterThan(0);

			// Wrong session events should be ignored
			const { final: wrongSessionResult } = replay(scenario.wrongSessionEvents, {
				sessionId: 'session-1',
				turnId: 'turn-1',
			});
			expect(wrongSessionResult.messages).toHaveLength(0);
		});

		it('ignores events with wrong turnId', () => {
			const scenario = sessionGatingScenario();

			// Wrong turn events should be ignored
			const { final: wrongTurnResult } = replay(scenario.wrongTurnEvents, {
				sessionId: 'session-1',
				turnId: 'turn-1',
			});
			expect(wrongTurnResult.messages).toHaveLength(0);
		});

		it('processes events without turnId (best-effort gating)', () => {
			// Events without turnId should pass through even if activeTurnId is set
			const { final } = replay(simpleTextScenario(), {
				sessionId: undefined,
				turnId: 'turn-1',
			});
			// simpleTextScenario events have no turnId, so they should pass through
			expect(final.messages.length).toBeGreaterThan(0);
		});
	});

	describe('Thinking with tools', () => {
		it('creates thinking tool and preserves ordering', () => {
			// thinkingWithToolsScenario:
			// 0: ThinkingStart, 1: Text, 2: ToolStart, 3: ToolEnd, 4: Text, 5: Complete
			const { snapshots, final } = replay(thinkingWithToolsScenario());

			// After ThinkingStart (index 0): thinking tool exists with completed status
			const thinkingTool = snapshots[0].tools.get('think-1');
			expect(thinkingTool).toBeDefined();
			expect(thinkingTool?.toolType).toBe('Thinking');
			expect(thinkingTool?.status).toBe('completed');

			// Final state: all finalized
			expect(final.messages.length).toBeGreaterThan(0);
		});
	});
});
