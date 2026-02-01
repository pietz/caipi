// App state store using Svelte 5 runes
import { api } from '$lib/api';
import { backendConfigs } from '$lib/config/backends';
import { chat } from './chat.svelte';

export type Screen = 'loading' | 'license' | 'onboarding' | 'folder' | 'chat';
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';
export type Model = string; // Now backend-specific, validated against backendConfigs
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

function getPersistedModel(backend: Backend): Model {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem(`caipi:model:${backend}`);
    const config = backendConfigs[backend];
    if (saved && config.models.some(m => m.id === saved)) return saved;
  }
  return backendConfigs[backend].defaultModel;
}

function getPersistedThinkingLevel(backend: Backend): string {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem(`caipi:thinking:${backend}`);
    const config = backendConfigs[backend];
    if (saved && config.thinkingOptions.some(o => o.value === saved)) return saved;
  }
  return backendConfigs[backend].defaultThinking;
}

function getPersistedBackend(): Backend {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem('caipi:backend');
    if (saved === 'claude' || saved === 'codex') return saved;
  }
  return 'claude';
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
  model = $state<Model>(getPersistedModel(getPersistedBackend()));
  thinkingLevel = $state<string>(getPersistedThinkingLevel(getPersistedBackend()));

  // Auth info
  authType = $state<string | null>(null);

  // License
  license = $state<LicenseInfo | null>(null);

  // CLI Path (custom path to Claude CLI)
  cliPath = $state<string | null>(null);

  // Backend selection (claude or codex)
  backend = $state<Backend>(getPersistedBackend());

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
    const config = backendConfigs[this.backend];
    if (config.models.some(m => m.id === model)) {
      this.model = model;
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem(`caipi:model:${this.backend}`, model);
      }
    }
  }

  setThinkingLevel(level: string) {
    const config = backendConfigs[this.backend];
    if (config.thinkingOptions.some(o => o.value === level)) {
      this.thinkingLevel = level;
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem(`caipi:thinking:${this.backend}`, level);
      }
    }
  }

  setBackend(backend: Backend) {
    this.backend = backend;
    const config = backendConfigs[backend];

    // Load saved or use default for model
    const savedModel = typeof localStorage !== 'undefined'
      ? localStorage.getItem(`caipi:model:${backend}`)
      : null;
    this.model = (savedModel && config.models.some(m => m.id === savedModel))
      ? savedModel
      : config.defaultModel;

    // Load saved or use default for thinking level
    const savedThinking = typeof localStorage !== 'undefined'
      ? localStorage.getItem(`caipi:thinking:${backend}`)
      : null;
    this.thinkingLevel = (savedThinking && config.thinkingOptions.some(o => o.value === savedThinking))
      ? savedThinking
      : config.defaultThinking;

    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('caipi:backend', backend);
    }
  }

  cycleModel() {
    const config = backendConfigs[this.backend];
    const models = config.models;
    const currentIndex = models.findIndex(m => m.id === this.model);
    const nextIndex = (currentIndex + 1) % models.length;
    this.setModel(models[nextIndex].id);
  }

  cycleThinking() {
    const config = backendConfigs[this.backend];
    const options = config.thinkingOptions;
    const currentIndex = options.findIndex(o => o.value === this.thinkingLevel);
    const nextIndex = (currentIndex + 1) % options.length;
    this.setThinkingLevel(options[nextIndex].value);
  }

  setPermissionMode(mode: PermissionMode) {
    this.permissionMode = mode;
  }

  cyclePermissionMode() {
    const modes: PermissionMode[] = ['default', 'acceptEdits', 'bypassPermissions'];
    const next = (modes.indexOf(this.permissionMode) + 1) % modes.length;
    this.permissionMode = modes[next];
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
