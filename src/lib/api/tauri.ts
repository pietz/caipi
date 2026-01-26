import { invoke } from '@tauri-apps/api/core';
import type { CliInstallStatus, StartupInfo, RecentFolder } from './types';

export const api = {
  // Setup
  checkCliInstalled: () => invoke<CliInstallStatus>('check_cli_installed'),
  getStartupInfo: () => invoke<StartupInfo>('get_startup_info'),
  validateFolder: (path: string) => invoke<boolean>('validate_folder', { path }),
  completeOnboarding: (defaultFolder?: string) =>
    invoke<void>('complete_onboarding', { defaultFolder }),
  saveRecentFolder: (path: string) => invoke<void>('save_recent_folder', { path }),

  // Folders
  getRecentFolders: () => invoke<RecentFolder[]>('get_recent_folders'),

  // Session
  createSession: (folderPath: string, permissionMode?: string, model?: string) =>
    invoke<string>('create_session', { folderPath, permissionMode, model }),
  sendMessage: (sessionId: string, message: string) =>
    invoke<void>('send_message', { sessionId, message }),
  abortSession: (sessionId: string) =>
    invoke<void>('abort_session', { sessionId }),

  // Permissions & Settings
  respondPermission: (sessionId: string, requestId: string, allowed: boolean) =>
    invoke<void>('respond_permission', { sessionId, requestId, allowed }),
  setPermissionMode: (sessionId: string, mode: string) =>
    invoke<void>('set_permission_mode', { sessionId, mode }),
  setModel: (sessionId: string, model: string) =>
    invoke<void>('set_model', { sessionId, model }),
  setExtendedThinking: (sessionId: string, enabled: boolean) =>
    invoke<void>('set_extended_thinking', { sessionId, enabled }),
};
