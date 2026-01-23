import { writable, derived } from 'svelte/store';

export interface ToolActivity {
  id: string;
  toolType: string;
  target: string;
  status: 'running' | 'completed' | 'error' | 'aborted';
  timestamp: number;
}

export interface ActivityState {
  activities: ToolActivity[];
}

const initialState: ActivityState = {
  activities: [],
};

function createActivityStore() {
  const { subscribe, set, update } = writable<ActivityState>(initialState);

  return {
    subscribe,

    addActivity: (activity: ToolActivity) => update(s => ({
      ...s,
      activities: [...s.activities, activity],
    })),

    updateActivityStatus: (id: string, status: ToolActivity['status']) => update(s => ({
      ...s,
      activities: s.activities.map(a =>
        a.id === id ? { ...a, status } : a
      ),
    })),

    removeActivity: (id: string) => update(s => ({
      ...s,
      activities: s.activities.filter(a => a.id !== id),
    })),

    clearActivities: () => update(s => ({
      ...s,
      activities: [],
    })),

    reset: () => set(initialState),
  };
}

export const activityStore = createActivityStore();

// Derived store
export const activities = derived(activityStore, $store => $store.activities);
