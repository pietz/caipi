import { invoke } from '@tauri-apps/api/core';
import type {
  CliInstallStatus,
  CliAuthStatus,
  StartupInfo,
  RecentFolder,
  LicenseStatus,
  LicenseValidationResult,
  SessionInfo,
  ProjectSessions,
  HistoryMessage,
  BackendStatus,
} from './types';

export const api = {
  // Setup
  checkCliInstalled: () => invoke<CliInstallStatus>('check_cli_installed'),
  checkCliAuthenticated: () => invoke<CliAuthStatus>('check_cli_authenticated'),
  getStartupInfo: () => invoke<StartupInfo>('get_startup_info'),
  validateFolder: (path: string) => invoke<boolean>('validate_folder', { path }),
  completeOnboarding: (defaultFolder?: string) =>
    invoke<void>('complete_onboarding', { defaultFolder }),
  saveRecentFolder: (path: string) => invoke<void>('save_recent_folder', { path }),

  // Folders
  getRecentFolders: () => invoke<RecentFolder[]>('get_recent_folders'),

  // Sessions
  getAllSessions: (backend?: string) => invoke<ProjectSessions[]>('get_all_sessions', { backend }),
  getRecentSessions: (limit: number, backend?: string) => invoke<ProjectSessions[]>('get_recent_sessions', { limit, backend }),
  getProjectSessions: (folderPath: string) =>
    invoke<SessionInfo[]>('get_project_sessions', { folderPath }),
  getSessionHistory: (folderPath: string, sessionId: string) =>
    invoke<HistoryMessage[]>('get_session_history', { folderPath, sessionId }),

  // Session
  createSession: (folderPath: string, permissionMode?: string, model?: string, resumeSessionId?: string, cliPath?: string, backend?: string) =>
    invoke<string>('create_session', { folderPath, permissionMode, model, resumeSessionId, cliPath, backend }),
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
  setThinkingLevel: (sessionId: string, level: string) =>
    invoke<void>('set_thinking_level', { sessionId, level }),

  // License
  validateLicense: (licenseKey: string) =>
    invoke<LicenseValidationResult>('validate_license', { licenseKey }),
  getLicenseStatus: () => invoke<LicenseStatus>('get_license_status'),
  clearLicense: () => invoke<void>('clear_license'),
  revalidateLicenseBackground: () => invoke<void>('revalidate_license_background'),

  // CLI Path
  getCliPath: () => invoke<string | null>('get_cli_path'),
  setCliPath: (path?: string) => invoke<void>('set_cli_path', { path }),

  // Backends
  checkBackendsStatus: () => invoke<BackendStatus[]>('check_backends_status'),
};
