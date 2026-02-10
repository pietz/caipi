import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { Model, PermissionMode, Screen } from './app.svelte';

// Mock Tauri APIs before importing the store
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    setMinSize: vi.fn(),
  })),
}));

vi.mock('@tauri-apps/api/dpi', () => ({
  LogicalSize: class LogicalSize {
    width: number;
    height: number;
    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  },
}));

describe('AppState Store', () => {
  let mockLocalStorage: Record<string, string>;

  beforeEach(async () => {
    // Reset module cache to get a fresh instance
    vi.resetModules();
    vi.clearAllMocks();

    // Create a fresh localStorage mock for each test
    mockLocalStorage = {};
    Object.defineProperty(global, 'localStorage', {
      value: {
        getItem: (key: string) => mockLocalStorage[key] ?? null,
        setItem: (key: string, value: string) => {
          mockLocalStorage[key] = value;
        },
        removeItem: (key: string) => {
          delete mockLocalStorage[key];
        },
        clear: () => {
          mockLocalStorage = {};
        },
        length: Object.keys(mockLocalStorage).length,
        key: (index: number) => Object.keys(mockLocalStorage)[index] ?? null,
      },
      writable: true,
    });

    // Import handled per-test for fresh module state
  });

  describe('Model persistence', () => {
    it('setModel saves to localStorage', async () => {
      const { app } = await import('./app.svelte');

      app.setModel('opus');
      expect(mockLocalStorage['caipi:model']).toBe('opus');

      app.setModel('haiku');
      expect(mockLocalStorage['caipi:model']).toBe('haiku');

      app.setModel('sonnet');
      expect(mockLocalStorage['caipi:model']).toBe('sonnet');
    });

    it('initial model loaded from localStorage if valid', async () => {
      mockLocalStorage['caipi:model'] = 'opus';
      vi.resetModules();

      const { app } = await import('./app.svelte');
      expect(app.model).toBe('opus');
    });

    it('defaults to sonnet if localStorage invalid', async () => {
      mockLocalStorage['caipi:model'] = 'invalid-model';
      vi.resetModules();

      const { app } = await import('./app.svelte');
      expect(app.model).toBe('sonnet');
    });

    it('defaults to sonnet if localStorage empty', async () => {
      vi.resetModules();

      const { app } = await import('./app.svelte');
      expect(app.model).toBe('sonnet');
    });

    it('loads haiku from localStorage', async () => {
      mockLocalStorage['caipi:model'] = 'haiku';
      vi.resetModules();

      const { app } = await import('./app.svelte');
      expect(app.model).toBe('haiku');
    });

    it('loads sonnet from localStorage', async () => {
      mockLocalStorage['caipi:model'] = 'sonnet';
      vi.resetModules();

      const { app } = await import('./app.svelte');
      expect(app.model).toBe('sonnet');
    });
  });

  describe('Screen transitions', () => {
    it('setScreen updates screen state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.screen).toBe('loading');

      app.setScreen('onboarding');
      expect(app.screen).toBe('onboarding');

      app.setScreen('folder');
      expect(app.screen).toBe('folder');

      app.setScreen('chat');
      expect(app.screen).toBe('chat');

      app.setScreen('loading');
      expect(app.screen).toBe('loading');
    });
  });

  describe('Sidebar toggles', () => {
    it('toggleLeftSidebar flips leftSidebar state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.leftSidebar).toBe(false);

      app.toggleLeftSidebar();
      expect(app.leftSidebar).toBe(true);

      app.toggleLeftSidebar();
      expect(app.leftSidebar).toBe(false);
    });

    it('toggleRightSidebar flips rightSidebar state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.rightSidebar).toBe(false);

      app.toggleRightSidebar();
      expect(app.rightSidebar).toBe(true);

      app.toggleRightSidebar();
      expect(app.rightSidebar).toBe(false);
    });

    it('both sidebars can be toggled independently', async () => {
      const { app } = await import('./app.svelte');

      expect(app.leftSidebar).toBe(false);
      expect(app.rightSidebar).toBe(false);

      app.toggleLeftSidebar();
      expect(app.leftSidebar).toBe(true);
      expect(app.rightSidebar).toBe(false);

      app.toggleRightSidebar();
      expect(app.leftSidebar).toBe(true);
      expect(app.rightSidebar).toBe(true);

      app.toggleLeftSidebar();
      expect(app.leftSidebar).toBe(false);
      expect(app.rightSidebar).toBe(true);
    });
  });

  describe('Permission mode', () => {
    it('setPermissionMode updates permissionMode', async () => {
      const { app } = await import('./app.svelte');

      expect(app.permissionMode).toBe('default');

      app.setPermissionMode('acceptEdits');
      expect(app.permissionMode).toBe('acceptEdits');

      app.setPermissionMode('bypassPermissions');
      expect(app.permissionMode).toBe('bypassPermissions');

      app.setPermissionMode('default');
      expect(app.permissionMode).toBe('default');
    });

    it('cyclePermissionMode cycles through modes in order', async () => {
      const { app } = await import('./app.svelte');

      expect(app.permissionMode).toBe('default');

      app.cyclePermissionMode();
      expect(app.permissionMode).toBe('acceptEdits');

      app.cyclePermissionMode();
      expect(app.permissionMode).toBe('bypassPermissions');

      app.cyclePermissionMode();
      expect(app.permissionMode).toBe('default');

      app.cyclePermissionMode();
      expect(app.permissionMode).toBe('acceptEdits');
    });
  });

  describe('Model cycling', () => {
    it('cycleModel cycles through opus -> sonnet -> haiku -> opus', async () => {
      const { app } = await import('./app.svelte');

      // Start at default (sonnet)
      expect(app.model).toBe('sonnet');

      app.cycleModel();
      expect(app.model).toBe('haiku');
      expect(mockLocalStorage['caipi:model']).toBe('haiku');

      app.cycleModel();
      expect(app.model).toBe('opus');
      expect(mockLocalStorage['caipi:model']).toBe('opus');

      app.cycleModel();
      expect(app.model).toBe('sonnet');
      expect(mockLocalStorage['caipi:model']).toBe('sonnet');

      app.cycleModel();
      expect(app.model).toBe('haiku');
      expect(mockLocalStorage['caipi:model']).toBe('haiku');
    });

    it('cycleModel from opus goes to sonnet', async () => {
      const { app } = await import('./app.svelte');

      app.setModel('opus');
      app.cycleModel();
      expect(app.model).toBe('sonnet');
    });

    it('cycleModel from haiku goes to opus', async () => {
      const { app } = await import('./app.svelte');

      app.setModel('haiku');
      app.cycleModel();
      expect(app.model).toBe('opus');
    });
  });

  describe('Reset', () => {
    it('reset clears all state to defaults', async () => {
      const { app } = await import('./app.svelte');

      // Set up some non-default state
      app.setScreen('chat');
      app.setLoading(false);
      app.setError('Some error');
      app.setFolder('/some/folder');
      app.setSessionId('session-123');
      app.toggleLeftSidebar();
      app.toggleRightSidebar();
      app.setAuthType('api-key');

      // Reset
      app.reset();

      // Check all defaults
      expect(app.screen).toBe('loading');
      expect(app.loading).toBe(true);
      expect(app.error).toBe(null);
      expect(app.folder).toBe(null);
      expect(app.sessionId).toBe(null);
      expect(app.leftSidebar).toBe(false);
      expect(app.rightSidebar).toBe(false);
      expect(app.authType).toBe(null);
    });

    it('reset does not affect permission mode or model', async () => {
      const { app } = await import('./app.svelte');

      app.setPermissionMode('acceptEdits');
      app.setModel('opus');

      app.reset();

      // These should not be reset
      expect(app.permissionMode).toBe('acceptEdits');
      expect(app.model).toBe('opus');
    });
  });

  describe('Other state methods', () => {
    it('setLoading updates loading state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.loading).toBe(true);

      app.setLoading(false);
      expect(app.loading).toBe(false);

      app.setLoading(true);
      expect(app.loading).toBe(true);
    });

    it('setError updates error state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.error).toBe(null);

      app.setError('Test error');
      expect(app.error).toBe('Test error');

      app.setError(null);
      expect(app.error).toBe(null);
    });

    it('setFolder updates folder state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.folder).toBe(null);

      app.setFolder('/test/folder');
      expect(app.folder).toBe('/test/folder');

      app.setFolder(null);
      expect(app.folder).toBe(null);
    });

    it('setSessionId updates sessionId state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.sessionId).toBe(null);

      app.setSessionId('session-456');
      expect(app.sessionId).toBe('session-456');

      app.setSessionId(null);
      expect(app.sessionId).toBe(null);
    });

    it('setCliStatus updates cliStatus state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.cliStatus).toBe(null);

      const status = {
        installed: true,
        version: '1.0.0',
        authenticated: true,
        path: '/usr/local/bin/claude',
      };

      app.setCliStatus(status);
      expect(app.cliStatus).toEqual(status);

      app.setCliStatus(null);
      expect(app.cliStatus).toBe(null);
    });

    it('setAuthType updates authType state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.authType).toBe(null);

      app.setAuthType('api-key');
      expect(app.authType).toBe('api-key');

      app.setAuthType(null);
      expect(app.authType).toBe(null);
    });

    it('setCliPath updates cliPath state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.cliPath).toBe(null);

      app.setCliPath('/usr/local/bin/claude');
      expect(app.cliPath).toBe('/usr/local/bin/claude');

      app.setCliPath(null);
      expect(app.cliPath).toBe(null);
    });

    it('setLicense updates license state', async () => {
      const { app } = await import('./app.svelte');

      expect(app.license).toBe(null);

      const license = {
        valid: true,
        licenseKey: 'test-key',
        activatedAt: 1700000000,
        email: 'test@example.com',
      };

      app.setLicense(license);
      expect(app.license).toEqual(license);

      app.setLicense(null);
      expect(app.license).toBe(null);
    });

    it('cycleThinking cycles through thinking levels', async () => {
      const { app } = await import('./app.svelte');

      // Claude CLI currently has no thinking controls exposed.
      expect(app.thinkingLevel).toBe('');

      app.cycleThinking();
      expect(app.thinkingLevel).toBe('');

      app.cycleThinking();
      expect(app.thinkingLevel).toBe('');
    });

    it('setThinkingLevel updates and persists thinking level per model', async () => {
      const { app } = await import('./app.svelte');

      app.setThinkingLevel('off');
      expect(app.thinkingLevel).toBe('off');
      // Key includes both backend and model
      expect(mockLocalStorage[`caipi:thinking:claude:${app.model}`]).toBe('off');

      app.setThinkingLevel('on');
      expect(app.thinkingLevel).toBe('on');
      expect(mockLocalStorage[`caipi:thinking:claude:${app.model}`]).toBe('on');
    });
  });

  describe('Derived properties', () => {
    it('folderName returns last segment of folder path', async () => {
      const { app } = await import('./app.svelte');

      expect(app.folderName).toBe('');

      app.setFolder('/Users/test/project');
      expect(app.folderName).toBe('project');

      app.setFolder('/home/user/workspace/app');
      expect(app.folderName).toBe('app');

      app.setFolder('/single');
      expect(app.folderName).toBe('single');

      app.setFolder(null);
      expect(app.folderName).toBe('');
    });
  });

  describe('syncState', () => {
    it('syncState updates permissionMode and model without persistence', async () => {
      const { app } = await import('./app.svelte');

      app.setModel('opus');
      app.setPermissionMode('acceptEdits');

      // Clear to verify syncState doesn't persist
      mockLocalStorage = {};

      app.syncState('bypassPermissions', 'haiku');

      expect(app.permissionMode).toBe('bypassPermissions');
      expect(app.model).toBe('haiku');

      // syncState should not persist to localStorage
      expect(mockLocalStorage['caipi:model']).toBeUndefined();
    });
  });

  describe('startSession', () => {
    it('startSession invokes create_session and updates state', async () => {
      const { app } = await import('./app.svelte');
      const { invoke } = await import('@tauri-apps/api/core');

      vi.mocked(invoke).mockResolvedValue('session-789');

      app.setModel('opus');
      app.setPermissionMode('acceptEdits');

      await app.startSession('/test/project');

      expect(invoke).toHaveBeenCalledWith(
        'create_session',
        expect.objectContaining({
          folderPath: '/test/project',
          permissionMode: 'acceptEdits',
          model: 'opus',
        })
      );

      expect(app.folder).toBe('/test/project');
      expect(app.sessionId).toBe('session-789');
      expect(app.screen).toBe('chat');
    });

    it('startSession uses current permission mode and model', async () => {
      const { app } = await import('./app.svelte');
      const { invoke } = await import('@tauri-apps/api/core');

      vi.mocked(invoke).mockResolvedValue('session-abc');

      app.setModel('haiku');
      app.setPermissionMode('bypassPermissions');

      await app.startSession('/another/folder');

      expect(invoke).toHaveBeenCalledWith(
        'create_session',
        expect.objectContaining({
          folderPath: '/another/folder',
          permissionMode: 'bypassPermissions',
          model: 'haiku',
        })
      );
    });

    it('startSession does not block on destroy_session', async () => {
      const { app } = await import('./app.svelte');
      const { invoke } = await import('@tauri-apps/api/core');

      app.setSessionId('old-session');

      let resolveDestroy: (() => void) | undefined;
      const destroyPromise = new Promise<void>((resolve) => {
        resolveDestroy = resolve;
      });

      vi.mocked(invoke).mockImplementation((command) => {
        if (command === 'destroy_session') return destroyPromise;
        if (command === 'create_session') return Promise.resolve('new-session');
        if (command === 'set_thinking_level') return Promise.resolve(undefined);
        return Promise.resolve(undefined);
      });

      const result = await Promise.race([
        app.startSession('/test/project').then(() => 'done'),
        new Promise((resolve) => setTimeout(() => resolve('timeout'), 50)),
      ]);

      expect(result).toBe('done');
      expect(invoke).toHaveBeenCalledWith('destroy_session', { sessionId: 'old-session' });
      if (resolveDestroy) resolveDestroy();
    });
  });

  describe('resumeSession', () => {
    it('resumeSession loads history after successful session creation', async () => {
      const { app } = await import('./app.svelte');
      const { chat } = await import('./chat.svelte');
      const { invoke } = await import('@tauri-apps/api/core');

      const history = [
        {
          id: 'msg-1',
          role: 'assistant',
          content: 'Hello',
          timestamp: 1700000000,
          tools: [],
        },
      ];

      vi.spyOn(chat, 'loadHistory').mockImplementation(() => {});
      vi.mocked(invoke)
        .mockResolvedValueOnce('session-resume')  // create_session
        .mockResolvedValueOnce(history);          // get_session_history

      await app.resumeSession('/test/project', 'session-abc');

      expect(invoke).toHaveBeenCalledWith(
        'create_session',
        expect.objectContaining({
          folderPath: '/test/project',
          permissionMode: app.permissionMode,
          model: app.model,
          resumeSessionId: 'session-abc',
          backend: 'claude',
        })
      );
      expect(invoke).not.toHaveBeenCalledWith(
        'set_thinking_level',
        expect.anything()
      );
      expect(invoke).toHaveBeenCalledWith('get_session_history', {
        folderPath: '/test/project',
        sessionId: 'session-abc',
        backend: 'claude',
      });
      expect(chat.loadHistory).toHaveBeenCalledWith(history);
      expect(app.screen).toBe('chat');
    });

    it('resumeSession uses provided backend override', async () => {
      const { app } = await import('./app.svelte');
      const { invoke } = await import('@tauri-apps/api/core');

      vi.mocked(invoke).mockImplementation((command) => {
        if (command === 'create_session') return Promise.resolve('session-resume');
        if (command === 'set_thinking_level') return Promise.resolve(undefined);
        if (command === 'get_session_history') return Promise.resolve([]);
        return Promise.resolve(undefined);
      });

      await app.resumeSession('/test/project', 'session-override', 'codex');

      expect(invoke).toHaveBeenCalledWith(
        'create_session',
        expect.objectContaining({
          folderPath: '/test/project',
          resumeSessionId: 'session-override',
          backend: 'codex',
        })
      );
      expect(invoke).toHaveBeenCalledWith('set_thinking_level', {
        sessionId: 'session-resume',
        level: app.thinkingLevel,
      });
    });

    it('resumeSession does not block on destroy_session', async () => {
      const { app } = await import('./app.svelte');
      const { invoke } = await import('@tauri-apps/api/core');

      app.setSessionId('old-session');
      app.setFolder('/old/project');

      let resolveDestroy: (() => void) | undefined;
      const destroyPromise = new Promise<void>((resolve) => {
        resolveDestroy = resolve;
      });

      vi.mocked(invoke).mockImplementation((command) => {
        if (command === 'destroy_session') return destroyPromise;
        if (command === 'create_session') return Promise.resolve('session-resume');
        if (command === 'set_thinking_level') return Promise.resolve(undefined);
        if (command === 'get_session_history') return Promise.resolve([]);
        return Promise.resolve(undefined);
      });

      const result = await Promise.race([
        app.resumeSession('/test/project', 'session-abc').then(() => 'done'),
        new Promise((resolve) => setTimeout(() => resolve('timeout'), 50)),
      ]);

      expect(result).toBe('done');
      expect(invoke).toHaveBeenCalledWith('destroy_session', { sessionId: 'old-session' });
      if (resolveDestroy) resolveDestroy();
    });
  });
});
