// Setup
export interface CliInstallStatus {
  installed: boolean;
  version?: string;
}

export interface CliAuthStatus {
  authenticated: boolean;
}

export interface StartupInfo {
  needsOnboarding: boolean;
  lastFolder?: string;
}

// Folders
export interface RecentFolder {
  path: string;
  lastUsed: string;
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
  license_key: string | null;
  activated_at: number | null;
  email: string | null;
}

export interface LicenseValidationResult {
  valid: boolean;
  error: string | null;
  email: string | null;
}
