// App state store using Svelte 5 runes
import { api } from '$lib/api';

export type Screen = 'loading' | 'license' | 'onboarding' | 'folder' | 'chat';
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';
export type Model = 'opus' | 'sonnet' | 'haiku';

export interface LicenseInfo {
  valid: boolean;
  licenseKey: string | null;
  activatedAt: number | null;
  email: string | null;
}

export interface CliStatus {
  installed: boolean;
  version: string | null;
  authenticated: boolean;
  path: string | null;
}

function getPersistedModel(): Model {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem('caipi:model');
    if (saved === 'opus' || saved === 'sonnet' || saved === 'haiku') return saved;
  }
  return 'sonnet';
}

class AppState {
  // Navigation
  screen = $state<Screen>('loading');
  loading = $state(true);
  error = $state<string | null>(null);

  // Session
  folder = $state<string | null>(null);
  sessionId = $state<string | null>(null);

  // CLI status (for onboarding)
  cliStatus = $state<CliStatus | null>(null);

  // UI
  leftSidebar = $state(false);
  rightSidebar = $state(false);

  // Settings
  permissionMode = $state<PermissionMode>('default');
  model = $state<Model>(getPersistedModel());
  extendedThinking = $state(false);

  // Auth info
  authType = $state<string | null>(null);

  // License
  license = $state<LicenseInfo | null>(null);

  // Derived
  get folderName(): string {
    return this.folder?.split('/').pop() ?? '';
  }

  // Methods
  setScreen(screen: Screen) {
    this.screen = screen;
  }

  setLoading(loading: boolean) {
    this.loading = loading;
  }

  setError(error: string | null) {
    this.error = error;
  }

  setFolder(folder: string | null) {
    this.folder = folder;
  }

  setSessionId(sessionId: string | null) {
    this.sessionId = sessionId;
  }

  setCliStatus(status: CliStatus | null) {
    this.cliStatus = status;
  }

  setAuthType(authType: string | null) {
    this.authType = authType;
  }

  setLicense(license: LicenseInfo | null) {
    this.license = license;
  }

  toggleLeftSidebar() {
    this.leftSidebar = !this.leftSidebar;
  }

  toggleRightSidebar() {
    this.rightSidebar = !this.rightSidebar;
  }

  setModel(model: Model) {
    this.model = model;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('caipi:model', model);
    }
  }

  cycleModel() {
    const models: Model[] = ['opus', 'sonnet', 'haiku'];
    const next = (models.indexOf(this.model) + 1) % models.length;
    this.setModel(models[next]);
  }

  setPermissionMode(mode: PermissionMode) {
    this.permissionMode = mode;
  }

  cyclePermissionMode() {
    const modes: PermissionMode[] = ['default', 'acceptEdits', 'bypassPermissions'];
    const next = (modes.indexOf(this.permissionMode) + 1) % modes.length;
    this.permissionMode = modes[next];
  }

  toggleExtendedThinking() {
    this.extendedThinking = !this.extendedThinking;
  }

  // Sync state from backend events
  syncState(permissionMode: PermissionMode, model: Model) {
    this.permissionMode = permissionMode;
    this.model = model;
  }

  async startSession(folder: string): Promise<void> {
    this.folder = folder;
    this.sessionId = await api.createSession(folder, this.permissionMode, this.model);
    this.screen = 'chat';
  }

  reset() {
    this.screen = 'loading';
    this.loading = true;
    this.error = null;
    this.folder = null;
    this.sessionId = null;
    this.leftSidebar = false;
    this.rightSidebar = false;
    this.authType = null;
  }
}

export const app = new AppState();
