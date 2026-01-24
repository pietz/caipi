// Re-export from individual store files for convenience
// Note: For .svelte.ts files, direct imports are recommended in components
// This file provides compatibility for existing code

// Theme store (standard Svelte store - works fine with barrel exports)
export { themeStore, resolvedTheme, applyTheme, type ThemePreference, type ResolvedTheme } from './theme';

// Type exports from new stores (types work fine with barrel exports)
export type { Screen, PermissionMode, Model, CliStatus } from './app.svelte';
export type { Message, ToolActivity, PermissionRequest, TodoItem, StreamItem } from './chat.svelte';
export type { FileEntry } from './files.svelte';

// Event handling utilities
export { handleClaudeEvent, respondToPermission, resetEventState, type ChatEvent, type EventHandlerOptions } from '$lib/utils/events';
