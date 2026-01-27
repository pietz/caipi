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
