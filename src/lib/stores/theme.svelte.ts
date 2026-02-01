import { browser } from '$app/environment';

export type ThemePreference = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

const STORAGE_KEY = 'caipi-theme-preference';

function getSystemTheme(): ResolvedTheme {
  if (!browser) return 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function getStoredPreference(): ThemePreference {
  if (!browser) return 'system';
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark' || stored === 'system') {
    return stored;
  }
  return 'system';
}

class ThemeStore {
  preference = $state<ThemePreference>(browser ? getStoredPreference() : 'system');
  systemTheme = $state<ResolvedTheme>(browser ? getSystemTheme() : 'dark');

  private mediaQuery: MediaQueryList | null = null;
  private handleChange: ((e: MediaQueryListEvent) => void) | null = null;

  constructor() {
    if (browser) {
      this.mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      this.handleChange = (e: MediaQueryListEvent) => {
        this.systemTheme = e.matches ? 'dark' : 'light';
      };
      this.mediaQuery.addEventListener('change', this.handleChange);
    }
  }

  get resolved(): ResolvedTheme {
    return this.preference === 'system' ? this.systemTheme : this.preference;
  }

  setPreference(preference: ThemePreference) {
    if (browser) {
      localStorage.setItem(STORAGE_KEY, preference);
    }
    this.preference = preference;
  }

  destroy() {
    if (this.mediaQuery && this.handleChange) {
      this.mediaQuery.removeEventListener('change', this.handleChange);
      this.mediaQuery = null;
      this.handleChange = null;
    }
  }
}

export const theme = new ThemeStore();

// Apply theme to document
export function applyTheme(themeValue: ResolvedTheme) {
  if (!browser) return;

  const root = document.documentElement;

  if (themeValue === 'dark') {
    root.classList.add('dark');
  } else {
    root.classList.remove('dark');
  }
}
