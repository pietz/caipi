import { vi } from 'vitest';
import { handleClaudeEvent, resetEventState, type ChatEvent } from '$lib/utils/events';
import { chat } from '$lib/stores/chat.svelte';
import { app } from '$lib/stores/app.svelte';
import {
  captureSnapshot,
  runAllInvariants,
  resetInvariantState,
  type StateSnapshot,
  type Violation,
  type InvariantChecker,
} from './invariants';

export interface Checkpoint {
  afterEvent: number; // 0-based event index
  assert: (state: StateSnapshot) => void;
}

export interface ReplayOptions {
  checkpoints?: Checkpoint[];
  invariants?: InvariantChecker[];
  skipDefaultInvariants?: boolean;
  sessionId?: string;
  turnId?: string;
  onComplete?: () => void;
  onError?: (msg: string) => void;
}

export interface ReplayResult {
  final: StateSnapshot;
  violations: Violation[];
  snapshots: StateSnapshot[]; // snapshot after each event
}

export function replay(events: ChatEvent[], opts: ReplayOptions = {}): ReplayResult {
  // Reset all state
  chat.reset();
  resetEventState();
  resetInvariantState();

  // Set up app state for session/turn gating
  app.setSessionId(opts.sessionId ?? null);
  chat.setActiveTurnId(opts.turnId ?? null);

  const violations: Violation[] = [];
  const snapshots: StateSnapshot[] = [];
  const toolHistory = new Map<string, string[]>();
  const invariants = opts.skipDefaultInvariants
    ? (opts.invariants ?? [])
    : [...(opts.invariants ?? []), runAllInvariants];

  // Use the provided callbacks or defaults
  const eventOptions = {
    onComplete: opts.onComplete,
    onError: opts.onError,
  };

  for (let i = 0; i < events.length; i++) {
    handleClaudeEvent(events[i], eventOptions);

    // Advance fake timers to flush text buffer (150ms is the FLUSH_DELAY_MS)
    vi.advanceTimersByTime(200);

    // Capture snapshot after each event
    const snapshot = captureSnapshot();
    snapshots.push(snapshot);

    // Run invariant checks
    for (const checker of invariants) {
      violations.push(...checker(snapshot, i, toolHistory));
    }

    // Run checkpoint assertions
    if (opts.checkpoints) {
      for (const cp of opts.checkpoints) {
        if (cp.afterEvent === i) {
          cp.assert(snapshot);
        }
      }
    }
  }

  const final = snapshots.length > 0 ? snapshots[snapshots.length - 1] : captureSnapshot();

  // Clean up app state
  app.setSessionId(null);
  chat.setActiveTurnId(null);

  return { final, violations, snapshots };
}
