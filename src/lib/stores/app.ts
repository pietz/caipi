import { writable, derived, get } from 'svelte/store';

export type AppScreen = 'loading' | 'onboarding' | 'folder' | 'chat';

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
};

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
