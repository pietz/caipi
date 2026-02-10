// Re-export from individual store files for convenience
// Note: For .svelte.ts files, direct imports are recommended in components
// This file provides compatibility for existing code

// Theme store (Svelte 5 runes - direct import from .svelte.ts recommended in components)
export { theme, applyTheme, type ThemePreference, type ResolvedTheme } from './theme.svelte';

// Type exports from new stores (types work fine with barrel exports)
export type { Screen, PermissionMode, Model, CliStatus } from './app.svelte';
export type { Message, ToolState, ToolStatus, TodoItem, StreamItem } from './chat.svelte';
export type { FileEntry } from './files.svelte';
export type { UpdateStatus } from './updater.svelte';

// Event handling utilities
export { handleChatEvent, respondToPermission, resetEventState, type ChatEvent, type EventHandlerOptions } from '$lib/utils/events';
