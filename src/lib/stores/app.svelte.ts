// App state store using Svelte 5 runes
import { api } from '$lib/api';
import { chat } from './chat.svelte';

export type Screen = 'loading' | 'license' | 'onboarding' | 'folder' | 'chat';
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';
export type Model = 'opus' | 'sonnet' | 'haiku';
export type Backend = 'claude' | 'codex';

export interface LicenseInfo {
  valid: boolean;
  licenseKey?: string;
  activatedAt?: number;
  email?: string;
}

export interface CliStatus {
  installed: boolean;
  version?: string;
  authenticated: boolean;
  path?: string;
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
  settingsOpen = $state(false);

  // Settings
  permissionMode = $state<PermissionMode>('default');
  model = $state<Model>(getPersistedModel());
  extendedThinking = $state(true);

  // Auth info
  authType = $state<string | null>(null);

  // License
  license = $state<LicenseInfo | null>(null);

  // CLI Path (custom path to Claude CLI)
  cliPath = $state<string | null>(null);

  // Backend selection (claude or codex)
  backend = $state<Backend>('claude');

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

  setCliPath(path: string | null) {
    this.cliPath = path;
  }

  toggleLeftSidebar() {
    this.leftSidebar = !this.leftSidebar;
  }

  toggleRightSidebar() {
    this.rightSidebar = !this.rightSidebar;
  }

  openSettings() {
    this.settingsOpen = true;
  }

  closeSettings() {
    this.settingsOpen = false;
  }

  setModel(model: Model) {
    this.model = model;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('caipi:model', model);
    }
  }

  setBackend(backend: Backend) {
    this.backend = backend;
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
    this.sessionId = await api.createSession(folder, this.permissionMode, this.model, undefined, this.cliPath ?? undefined, this.backend);
    this.screen = 'chat';
  }

  async resumeSession(folderPath: string, sessionId: string): Promise<void> {
    this.folder = folderPath;

    // Create session first - if this fails, don't pollute chat state
    this.sessionId = await api.createSession(
      folderPath,
      this.permissionMode,
      this.model,
      sessionId,
      this.cliPath ?? undefined,
      this.backend
    );

    // Only load history after successful session creation
    const history = await api.getSessionHistory(folderPath, sessionId);
    chat.loadHistory(history);

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
