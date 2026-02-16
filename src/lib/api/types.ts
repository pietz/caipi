// Setup
export interface CliStatus {
  installed: boolean;
  version?: string;
  authenticated: boolean;
  path?: string;
}

export interface BackendCliStatus {
  backend: string;
  installed: boolean;
  authenticated: boolean;
  version?: string;
  path?: string;
  installHint?: string;
  authHint?: string;
}

export interface StartupInfo {
  onboardingCompleted: boolean;
  cliStatus?: CliStatus;
  cliStatusFresh: boolean;
  defaultFolder?: string;
  cliPath?: string;
  defaultBackend: string;
  backendCliPaths?: Record<string, string>;
}

// Folders
export interface RecentFolder {
  path: string;
  name: string;
  timestamp: number;
}

// Sessions
export interface SessionInfo {
  sessionId: string;
  folderPath: string;
  folderName: string;
  firstPrompt: string;
  messageCount: number;
  created: string;
  modified: string;
  backend?: string;
}

export interface ProjectSessions {
  folderPath: string;
  folderName: string;
  sessions: SessionInfo[];
  latestModified: string;
}

export interface HistoryTool {
  id: string;
  toolType: string;
  target: string;
}

export interface HistoryMessage {
  id: string;
  role: string;
  content: string;
  timestamp: number;
  tools: HistoryTool[];
}

// License
export interface LicenseStatus {
  valid: boolean;
  licenseKey?: string;
  activatedAt?: number;
  email?: string;
}

export interface LicenseValidationResult {
  valid: boolean;
  error?: string;
  email?: string;
}
