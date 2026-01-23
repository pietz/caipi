import { writable, derived } from 'svelte/store';

export interface PermissionRequest {
  id: string;
  activityId: string | null;  // ID of the activity awaiting permission
  tool: string;
  description: string;
  timestamp: number;
}

export interface PermissionState {
  pendingPermissions: Record<string, PermissionRequest>;  // Keyed by activityId for parallel tools
}

const initialState: PermissionState = {
  pendingPermissions: {},
};

function createPermissionStore() {
  const { subscribe, set, update } = writable<PermissionState>(initialState);

  return {
    subscribe,

    addPermissionRequest: (request: PermissionRequest) => update(s => {
      // Use activityId as key (or request.id if no activityId)
      const key = request.activityId || request.id;
      return {
        ...s,
        pendingPermissions: {
          ...s.pendingPermissions,
          [key]: request,
        },
      };
    }),

    removePermissionRequest: (activityIdOrRequestId: string) => update(s => {
      const { [activityIdOrRequestId]: removed, ...rest } = s.pendingPermissions;
      return {
        ...s,
        pendingPermissions: rest,
      };
    }),

    clearPermissionRequests: () => update(s => ({
      ...s,
      pendingPermissions: {},
    })),

    reset: () => set(initialState),
  };
}

export const permissionStore = createPermissionStore();

// Derived store
export const pendingPermissions = derived(permissionStore, $store => $store.pendingPermissions);
