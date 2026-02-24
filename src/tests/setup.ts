import '@testing-library/jest-dom';
import { expect, afterEach, beforeEach, vi } from 'vitest';
import { cleanup } from '@testing-library/svelte';

// Mock the logger globally — it wraps @tauri-apps/plugin-log which needs
// the Tauri runtime and would cause unhandled rejections in tests.
vi.mock('$lib/utils/logger', () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  initLogger: vi.fn(),
}));

// Mock localStorage for happy-dom - must be available at module load time
// (before beforeEach) because some stores call localStorage in their initializers.
const localStorageMock = {
  getItem: (key: string) => null,
  setItem: (key: string, value: string) => {},
  removeItem: (key: string) => {},
  clear: () => {},
  length: 0,
  key: (index: number) => null,
};
Object.defineProperty(global, 'localStorage', {
  value: localStorageMock,
  writable: true,
  configurable: true,
});

beforeEach(() => {
  // Re-apply in case a test modified it
  Object.defineProperty(global, 'localStorage', {
    value: localStorageMock,
    writable: true,
    configurable: true,
  });
});

afterEach(() => {
  cleanup();
});
