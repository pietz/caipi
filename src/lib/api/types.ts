// Setup
export interface CliInstallStatus {
  installed: boolean;
  version?: string;
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
