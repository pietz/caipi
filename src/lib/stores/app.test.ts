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

      expect(invoke).toHaveBeenCalledWith('create_session', {
        folderPath: '/test/project',
        permissionMode: 'acceptEdits',
        model: 'opus',
      });

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

      expect(invoke).toHaveBeenCalledWith('create_session', {
        folderPath: '/another/folder',
        permissionMode: 'bypassPermissions',
        model: 'haiku',
      });
    });
  });
});
