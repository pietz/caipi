// App state store using Svelte 5 runes
import { api } from '$lib/api';
import type { CliStatus } from '$lib/api/types';
import { chat } from './chat.svelte';
import { getBackendConfig, type Backend } from '$lib/config/backends';
import { info } from '$lib/utils/logger';

export type { CliStatus };
export type Screen = 'loading' | 'license' | 'onboarding' | 'folder' | 'chat';
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';
export type Model = string;

export interface LicenseInfo {
  valid: boolean;
  licenseKey?: string;
  activatedAt?: number;
  email?: string;
}

const RECENT_SESSIONS_PREWARM_LIMIT = 50;
const RECENT_SESSIONS_PREWARM_TTL_MS = 5 * 60_000;

function getDefaultModel(backend: Backend): Model {
  const models = getBackendConfig(backend).models;
  if (backend === 'claude') {
    return models.find((m) => m.id === 'sonnet')?.id ?? models[0]?.id ?? '';
  }
  return models[0]?.id ?? '';
}

function getPersistedModel(backend: Backend): Model {
  const validModels = getBackendConfig(backend).models.map((m) => m.id);

  if (typeof localStorage !== 'undefined') {
    const scoped = localStorage.getItem(`caipi:model:${backend}`);
    if (scoped && validModels.includes(scoped)) return scoped;

    // Backwards compatibility: older builds used "claudecli" as the Claude backend key.
    if (backend === 'claude') {
      const legacyScoped = localStorage.getItem('caipi:model:claudecli');
      if (legacyScoped && validModels.includes(legacyScoped)) {
        localStorage.setItem(`caipi:model:${backend}`, legacyScoped);
        return legacyScoped;
      }
    }

    // Legacy fallback for older builds that used a single shared model key.
    const legacy = localStorage.getItem('caipi:model');
    if (legacy && validModels.includes(legacy)) return legacy;
  }
  return getDefaultModel(backend);
}

function getDefaultThinkingLevel(backend: Backend, model: Model): string {
  const config = getBackendConfig(backend);
  const modelConfig = config.models.find((m) => m.id === model);
  return modelConfig?.defaultThinking ?? '';
}

function getPersistedThinkingLevel(backend: Backend, model: Model): string {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem(`caipi:thinking:${backend}:${model}`);
    if (saved) {
      // Validate saved value is still valid for this model
      const config = getBackendConfig(backend);
      const modelConfig = config.models.find((m) => m.id === model);
      if (modelConfig?.thinkingOptions.some((o) => o.value === saved)) return saved;
    }

    // Backwards compatibility: older builds used "claudecli" as the Claude backend key.
    if (backend === 'claude') {
      const legacySaved = localStorage.getItem(`caipi:thinking:claudecli:${model}`);
      if (legacySaved) {
        const config = getBackendConfig(backend);
        const modelConfig = config.models.find((m) => m.id === model);
        if (modelConfig?.thinkingOptions.some((o) => o.value === legacySaved)) {
          localStorage.setItem(`caipi:thinking:${backend}:${model}`, legacySaved);
          return legacySaved;
        }
      }
    }
  }
  return getDefaultThinkingLevel(backend, model);
}

class AppState {
  private recentSessionsWarmupInFlight: Partial<Record<Backend, Promise<void>>> = {};
  private recentSessionsWarmedAt: Partial<Record<Backend, number>> = {};

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

  // Backend default
  defaultBackend = $state<Backend>('claude');
  sessionBackend = $state<Backend | null>(null);
  backendCliPaths = $state<Record<string, string>>({});

  // Settings
  permissionMode = $state<PermissionMode>('default');
  model = $state<Model>(getPersistedModel('claude'));
  thinkingLevel = $state<string>(getPersistedThinkingLevel('claude', getPersistedModel('claude')));

  // Auth info
  authType = $state<string | null>(null);

  // License
  license = $state<LicenseInfo | null>(null);

  // Derived
  get folderName(): string {
    if (!this.folder) return '';
    // Handle both Unix and Windows paths
    const normalized = this.folder.replace(/\\/g, '/');
    return normalized.split('/').pop() ?? '';
  }

  get backendConfig() {
    return getBackendConfig(this.sessionBackend ?? this.defaultBackend);
  }

  get activeBackend(): Backend {
    return this.sessionBackend ?? this.defaultBackend;
  }

  get currentModelConfig() {
    return this.backendConfig.models.find((m) => m.id === this.model) ?? this.backendConfig.models[0];
  }

  get thinkingOptions() {
    return this.currentModelConfig?.thinkingOptions ?? [];
  }

  get cliPath(): string | null {
    return this.backendCliPaths[this.defaultBackend] ?? null;
  }

  // Methods
  setScreen(screen: Screen) {
    info(`Screen: ${this.screen} → ${screen}`);
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

  setCliPath(path: string | null, backend: Backend = this.defaultBackend) {
    const next = { ...this.backendCliPaths };
    if (path) {
      next[backend] = path;
    } else {
      delete next[backend];
    }
    this.backendCliPaths = next;
  }

  getCliPath(backend: Backend): string | undefined {
    return this.backendCliPaths[backend];
  }

  async setDefaultBackend(backend: Backend) {
    this.defaultBackend = backend;
    if (!this.sessionBackend) {
      this.model = getPersistedModel(backend);
      this.thinkingLevel = getPersistedThinkingLevel(backend, this.model);
    }
    await api.setDefaultBackend(backend);
    // Warm this backend's recent sessions in the background so folder picker opens faster.
    void this.prewarmRecentSessions([backend]);
  }

  prewarmRecentSessions(backends: Backend[] = ['claude', 'codex'], force = false) {
    for (const backend of backends) {
      void this.prewarmRecentSessionsForBackend(backend, force);
    }
  }

  private prewarmRecentSessionsForBackend(backend: Backend, force = false): Promise<void> {
    const inFlight = this.recentSessionsWarmupInFlight[backend];
    if (!force && inFlight) {
      return inFlight;
    }

    const warmedAt = this.recentSessionsWarmedAt[backend] ?? 0;
    if (!force && warmedAt && Date.now() - warmedAt < RECENT_SESSIONS_PREWARM_TTL_MS) {
      return Promise.resolve();
    }

    const warmup = api
      .getRecentSessions(RECENT_SESSIONS_PREWARM_LIMIT, backend)
      .then(() => {
        this.recentSessionsWarmedAt[backend] = Date.now();
      })
      .catch(() => {
        // Best-effort only; runtime load path still fetches sessions normally.
      })
      .finally(() => {
        delete this.recentSessionsWarmupInFlight[backend];
      });

    this.recentSessionsWarmupInFlight[backend] = warmup;
    return warmup;
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

  setModel(model: Model, backend: Backend = this.activeBackend) {
    this.model = model;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(`caipi:model:${backend}`, model);
      // Keep legacy key in sync for backwards compatibility.
      localStorage.setItem('caipi:model', model);
    }
    // Reset thinking level to persisted or default for the new model
    this.thinkingLevel = getPersistedThinkingLevel(backend, model);
  }

  cycleModel() {
    const models = this.backendConfig.models.map((m) => m.id);
    if (!models.length) return;
    const currentIndex = models.indexOf(this.model);
    const next = (currentIndex + 1) % models.length;
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

  setThinkingLevel(level: string) {
    this.thinkingLevel = level;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(`caipi:thinking:${this.activeBackend}:${this.model}`, level);
    }
  }

  cycleThinking() {
    const options = this.thinkingOptions;
    if (!options.length) return;
    const currentIndex = options.findIndex(opt => opt.value === this.thinkingLevel);
    const nextIndex = (currentIndex + 1) % options.length;
    this.setThinkingLevel(options[nextIndex].value);
  }

  // Sync state from backend events
  syncState(permissionMode: PermissionMode, model: Model) {
    this.permissionMode = permissionMode;
    this.model = model;
  }

  ensureBackendState(backend: Backend) {
    const backendModels = getBackendConfig(backend).models.map((m) => m.id);
    if (!backendModels.includes(this.model)) {
      this.setModel(getPersistedModel(backend), backend);
    }
    this.thinkingLevel = getPersistedThinkingLevel(backend, this.model);
  }

  async startSession(folder: string): Promise<void> {
    // Clean up previous session to prevent memory leaks
    if (this.sessionId) {
      const oldSession = this.sessionId;
      this.sessionId = null;
      void api.destroySession(oldSession).catch(() => {});
    }

    this.folder = folder;
    const backend = this.defaultBackend;
    this.ensureBackendState(backend);
    this.sessionId = await api.createSession(
      folder,
      this.permissionMode,
      this.model,
      undefined,
      this.getCliPath(backend),
      backend
    );
    this.sessionBackend = backend;
    info(`Session started: folder=${folder} backend=${backend} model=${this.model} id=${this.sessionId}`);
    // Sync persisted thinking level to the new session
    if (this.thinkingOptions.length > 0 && this.thinkingLevel) {
      await api.setThinkingLevel(this.sessionId, this.thinkingLevel);
    }
    this.screen = 'chat';
  }

  async resumeSession(folderPath: string, sessionId: string, backendOverride?: Backend): Promise<void> {
    // Clean up previous session to prevent memory leaks
    if (this.sessionId) {
      const oldSession = this.sessionId;
      this.sessionId = null;
      void api.destroySession(oldSession).catch(() => {});
    }

    this.folder = folderPath;
    const backend = backendOverride ?? this.defaultBackend;
    this.ensureBackendState(backend);

    // Create session first - if this fails, don't pollute chat state
    this.sessionId = await api.createSession(
      folderPath,
      this.permissionMode,
      this.model,
      sessionId,
      this.getCliPath(backend),
      backend
    );
    this.sessionBackend = backend;
    info(`Session resumed: folder=${folderPath} id=${this.sessionId} backend=${backend}`);

    // Sync persisted thinking level to the resumed session
    if (this.thinkingOptions.length > 0 && this.thinkingLevel) {
      await api.setThinkingLevel(this.sessionId, this.thinkingLevel);
    }

    // Only load history after successful session creation
    const history = await api.getSessionHistory(folderPath, sessionId, backend);
    chat.loadHistory(history);
    this.screen = 'chat';
  }

  reset() {
    this.screen = 'loading';
    this.loading = true;
    this.error = null;
    this.folder = null;
    this.sessionId = null;
    this.sessionBackend = null;
    this.leftSidebar = false;
    this.rightSidebar = false;
    this.authType = null;
  }
}

export const app = new AppState();
