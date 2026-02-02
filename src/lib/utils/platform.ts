import { platform, type Platform } from '@tauri-apps/plugin-os';

let cachedPlatform: Platform | null = null;

/**
 * Get the current platform. Caches the result after first call.
 */
export function getPlatform(): Platform {
  if (cachedPlatform === null) {
    cachedPlatform = platform();
  }
  return cachedPlatform;
}

/**
 * Check if running on macOS
 */
export function isMacOS(): boolean {
  return getPlatform() === 'macos';
}

/**
 * Check if running on Windows
 */
export function isWindows(): boolean {
  return getPlatform() === 'windows';
}

/**
 * Check if running on Linux
 */
export function isLinux(): boolean {
  return getPlatform() === 'linux';
}

/**
 * Get the appropriate keyboard modifier symbol for the current platform
 * Returns ⌘ on macOS, Ctrl on Windows/Linux
 */
export function getModifierKey(): string {
  return isMacOS() ? '⌘' : 'Ctrl';
}
