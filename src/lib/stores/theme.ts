import { writable, derived } from 'svelte/store';
import { browser } from '$app/environment';

export type ThemePreference = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

interface ThemeState {
  preference: ThemePreference;
  systemTheme: ResolvedTheme;
}

function getSystemTheme(): ResolvedTheme {
  if (!browser) return 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

const STORAGE_KEY = 'caipi-theme-preference';

function getStoredPreference(): ThemePreference {
  if (!browser) return 'system';
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark' || stored === 'system') {
    return stored;
  }
  return 'system';
}

function createThemeStore() {
  const initialState: ThemeState = {
    preference: browser ? getStoredPreference() : 'system',
    systemTheme: browser ? getSystemTheme() : 'dark',
  };

  const { subscribe, set, update } = writable<ThemeState>(initialState);

  // Listen for system theme changes
  if (browser) {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

    const handleChange = (e: MediaQueryListEvent) => {
      update(state => ({
        ...state,
        systemTheme: e.matches ? 'dark' : 'light',
      }));
    };

    mediaQuery.addEventListener('change', handleChange);
  }

  return {
    subscribe,
    setPreference: (preference: ThemePreference) => {
      if (browser) {
        localStorage.setItem(STORAGE_KEY, preference);
      }
      update(state => ({ ...state, preference }));
    },
  };
}

export const themeStore = createThemeStore();

// Derived store for the actual theme to apply
export const resolvedTheme = derived(themeStore, ($theme): ResolvedTheme => {
  if ($theme.preference === 'system') {
    return $theme.systemTheme;
  }
  return $theme.preference;
});

// Apply theme to document
export function applyTheme(theme: ResolvedTheme) {
  if (!browser) return;

  const root = document.documentElement;

  if (theme === 'dark') {
    root.classList.add('dark');
  } else {
    root.classList.remove('dark');
  }
}
