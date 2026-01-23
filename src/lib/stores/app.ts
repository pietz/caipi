import { writable, derived, get } from 'svelte/store';

export type AppScreen = 'loading' | 'onboarding' | 'folder' | 'chat';
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';
export type ModelType = 'opus' | 'sonnet' | 'haiku';

export interface CliStatus {
  installed: boolean;
  version: string | null;
  authenticated: boolean;
  path: string | null;
}

export interface AppState {
  screen: AppScreen;
  cliStatus: CliStatus | null;
  selectedFolder: string | null;
  sessionId: string | null;
  loading: boolean;
  error: string | null;
  leftSidebarOpen: boolean;
  rightSidebarOpen: boolean;
  authType: string | null;
  permissionMode: PermissionMode;
  model: ModelType;
}

// Load persisted model from localStorage (default to sonnet)
function getPersistedModel(): ModelType {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem('caipi:model');
    if (saved === 'opus' || saved === 'sonnet' || saved === 'haiku') {
      return saved;
    }
  }
  return 'sonnet';
}

const initialState: AppState = {
  screen: 'loading',
  cliStatus: null,
  selectedFolder: null,
  sessionId: null,
  loading: true,
  error: null,
  leftSidebarOpen: false,
  rightSidebarOpen: false,
  authType: null,
  permissionMode: 'default',
  model: getPersistedModel(),
};

const PERMISSION_MODES: PermissionMode[] = ['default', 'acceptEdits', 'bypassPermissions'];
const MODELS: ModelType[] = ['opus', 'sonnet', 'haiku'];

function createAppStore() {
  const { subscribe, set, update } = writable<AppState>(initialState);

  return {
    subscribe,
    setScreen: (screen: AppScreen) => update(s => ({ ...s, screen })),
    setCliStatus: (cliStatus: CliStatus) => update(s => ({ ...s, cliStatus })),
    setSelectedFolder: (folder: string | null) => update(s => ({ ...s, selectedFolder: folder })),
    setSessionId: (sessionId: string | null) => update(s => ({ ...s, sessionId })),
    setLoading: (loading: boolean) => update(s => ({ ...s, loading })),
    setError: (error: string | null) => update(s => ({ ...s, error })),
    toggleLeftSidebar: () => update(s => ({ ...s, leftSidebarOpen: !s.leftSidebarOpen })),
    toggleRightSidebar: () => update(s => ({ ...s, rightSidebarOpen: !s.rightSidebarOpen })),
    setLeftSidebarOpen: (open: boolean) => update(s => ({ ...s, leftSidebarOpen: open })),
    setRightSidebarOpen: (open: boolean) => update(s => ({ ...s, rightSidebarOpen: open })),
    setAuthType: (authType: string | null) => update(s => ({ ...s, authType })),
    setPermissionMode: (mode: PermissionMode) => update(s => ({ ...s, permissionMode: mode })),
    cyclePermissionMode: () => update(s => {
      const currentIndex = PERMISSION_MODES.indexOf(s.permissionMode);
      const nextIndex = (currentIndex + 1) % PERMISSION_MODES.length;
      return { ...s, permissionMode: PERMISSION_MODES[nextIndex] };
    }),
    setModel: (model: ModelType) => {
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('caipi:model', model);
      }
      update(s => ({ ...s, model }));
    },
    cycleModel: () => update(s => {
      const currentIndex = MODELS.indexOf(s.model);
      const nextIndex = (currentIndex + 1) % MODELS.length;
      const newModel = MODELS[nextIndex];
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('caipi:model', newModel);
      }
      return { ...s, model: newModel };
    }),
    syncState: (permissionMode: PermissionMode, model: ModelType) => {
      update(s => ({ ...s, permissionMode, model }));
    },
    reset: () => set(initialState),
  };
}

export const appStore = createAppStore();

// Derived stores for convenience
export const currentScreen = derived(appStore, $app => $app.screen);
export const isLoading = derived(appStore, $app => $app.loading);
export const appError = derived(appStore, $app => $app.error);
export const leftSidebarOpen = derived(appStore, $app => $app.leftSidebarOpen);
export const rightSidebarOpen = derived(appStore, $app => $app.rightSidebarOpen);
export const authType = derived(appStore, $app => $app.authType);
export const permissionMode = derived(appStore, $app => $app.permissionMode);
export const model = derived(appStore, $app => $app.model);
