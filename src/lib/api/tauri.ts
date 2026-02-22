import { invoke } from '@tauri-apps/api/core';
import type {
  BackendCliStatus,
  StartupInfo,
  RecentFolder,
  SessionInfo,
  ProjectSessions,
  HistoryMessage,
} from './types';

export const api = {
  // Setup
  checkAllBackendsStatus: () => invoke<BackendCliStatus[]>('check_all_backends_status'),
  getStartupInfo: () => invoke<StartupInfo>('get_startup_info'),
  validateFolder: (path: string) => invoke<boolean>('validate_folder', { path }),
  completeOnboarding: (defaultFolder?: string, defaultBackend?: string) =>
    invoke<void>('complete_onboarding', { defaultFolder, defaultBackend }),
  saveRecentFolder: (path: string) => invoke<void>('save_recent_folder', { path }),
  setDefaultBackend: (backend?: string) => invoke<void>('set_default_backend', { backend }),

  // Folders
  getRecentFolders: () => invoke<RecentFolder[]>('get_recent_folders'),

  // Sessions
  getAllSessions: (backend?: string) => invoke<ProjectSessions[]>('get_all_sessions', { backend }),
  getRecentSessions: (limit: number, backend?: string) => invoke<ProjectSessions[]>('get_recent_sessions', { limit, backend }),
  getProjectSessions: (folderPath: string, backend?: string) =>
    invoke<SessionInfo[]>('get_project_sessions', { folderPath, backend }),
  getSessionHistory: (folderPath: string, sessionId: string, backend?: string) =>
    invoke<HistoryMessage[]>('get_session_history', { folderPath, sessionId, backend }),

  // Session
  createSession: (folderPath: string, permissionMode?: string, model?: string, resumeSessionId?: string, cliPath?: string, backend?: string) =>
    invoke<string>('create_session', { folderPath, permissionMode, model, resumeSessionId, cliPath, backend }),
  destroySession: (sessionId: string) =>
    invoke<void>('destroy_session', { sessionId }),
  sendMessage: (sessionId: string, message: string, turnId?: string) =>
    invoke<void>('send_message', { sessionId, message, turnId }),
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

  // CLI Path
  getBackendCliPath: (backend: string) => invoke<string | null>('get_backend_cli_path', { backend }),
  setBackendCliPath: (backend: string, path?: string) =>
    invoke<void>('set_backend_cli_path', { backend, path }),
};
