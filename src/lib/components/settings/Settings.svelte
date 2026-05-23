<script lang="ts">
  import { X, Sun, Moon, Monitor, Loader2, Copy, Check } from 'lucide-svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui';
  import { ClaudeIcon, OpenAIIcon } from '$lib/components/icons';
  import { theme, type ThemePreference } from '$lib/stores/theme.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { chat } from '$lib/stores/chat.svelte';
  import { api } from '$lib/api';
  import type { Backend } from '$lib/config/backends';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let appVersion = $state<string | null>(null);
  let cliPathInput = $state(app.cliPath ?? '');
  let proxyTargetInput = $state(app.proxyTarget ?? '');
  let savingCliPath = $state(false);
  let savingProxyTarget = $state(false);
  let loggingOut = $state(false);
  let copiedCommand = $state<string | null>(null);
  const STATUS_TIMEOUT_MS = 6000;
  type BackendInstallState = 'checking' | 'installed' | 'not_installed';
  type BackendStatus = {
    installState: BackendInstallState;
    authenticated: boolean;
    version?: string;
  };
  let backendStatuses = $state<Record<string, BackendStatus>>({
    claude: { installState: 'checking', authenticated: false },
    codex: { installState: 'checking', authenticated: false },
  });

  // Strip extra suffixes from version string
  const cliVersion = $derived.by(() => {
    const version = backendStatuses[app.defaultBackend]?.version;
    if (!version) return null;
    return version.replace(/\s*\(.*\)\s*$/, '').trim();
  });

  const currentPreference = $derived(theme.preference);
  const currentBackendStatus = $derived(backendStatuses[app.defaultBackend]);
  const loginCommand = $derived(app.defaultBackend === 'codex' ? 'codex login' : 'claude auth login');

  onMount(async () => {
    await refreshBackendStatuses();
    try {
      appVersion = await getVersion();
    } catch (e) {
      console.error('Failed to get app version:', e);
    }
  });

  async function refreshBackendStatuses() {
    const previousStatuses = backendStatuses;
    backendStatuses = {
      claude: { installState: 'checking', authenticated: previousStatuses.claude?.authenticated ?? false, version: previousStatuses.claude?.version },
      codex: { installState: 'checking', authenticated: previousStatuses.codex?.authenticated ?? false, version: previousStatuses.codex?.version },
    };

    try {
      const statuses = await Promise.race([
        api.checkAllBackendsStatus(),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('Timed out while checking backend status')), STATUS_TIMEOUT_MS)
        ),
      ]);
      backendStatuses = Object.fromEntries(
        statuses.map(s => [s.backend, {
          installState: s.installed ? 'installed' : 'not_installed',
          authenticated: s.authenticated,
          version: s.version,
        }])
      );
    } catch (e) {
      console.error('Failed to refresh backend statuses:', e);
      backendStatuses = {
        claude: { installState: 'not_installed', authenticated: false, version: previousStatuses.claude?.version },
        codex: { installState: 'not_installed', authenticated: false, version: previousStatuses.codex?.version },
      };
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }

  function setTheme(preference: ThemePreference) {
    theme.setPreference(preference);
  }

  async function saveCliPath() {
    savingCliPath = true;
    try {
      const trimmed = cliPathInput.trim();
      const pathToSave = trimmed === '' ? undefined : trimmed;
      await api.setBackendCliPath(app.defaultBackend, pathToSave);
      app.setCliPath(pathToSave ?? null, app.defaultBackend);
    } catch (e) {
      console.error('Failed to save CLI path:', e);
    } finally {
      savingCliPath = false;
    }
  }

  async function saveProxyTarget() {
    savingProxyTarget = true;
    try {
      const trimmed = proxyTargetInput.trim();
      const targetToSave = trimmed === '' ? undefined : trimmed;
      await api.setBackendProxyTarget(app.defaultBackend, targetToSave);
      app.setProxyTarget(targetToSave ?? null, app.defaultBackend);
    } catch (e) {
      console.error('Failed to save proxy target:', e);
    } finally {
      savingProxyTarget = false;
    }
  }

  function handleCliPathKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      saveCliPath();
    }
  }

  function handleProxyTargetKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      saveProxyTarget();
    }
  }

  async function copyCommand(command: string) {
    try {
      await navigator.clipboard.writeText(command);
      copiedCommand = command;
      setTimeout(() => {
        if (copiedCommand === command) copiedCommand = null;
      }, 2000);
    } catch (e) {
      console.error('Failed to copy command:', e);
    }
  }

  async function logoutCurrentBackend() {
    loggingOut = true;
    try {
      await api.logoutBackend(app.defaultBackend);
      app.setAuthType(null);

      if (app.sessionId && (app.sessionBackend ?? app.defaultBackend) === app.defaultBackend) {
        const activeSession = app.sessionId;
        app.setSessionId(null);
        app.sessionBackend = null;
        void api.destroySession(activeSession).catch(() => {});
        chat.reset();
        app.setScreen('folder');
      }

      await refreshBackendStatuses();
    } catch (e) {
      console.error('Failed to log out backend:', e);
    } finally {
      loggingOut = false;
    }
  }

  async function switchBackend(backend: Backend) {
    if (backend === app.defaultBackend) return;
    await app.setDefaultBackend(backend);
    cliPathInput = app.getCliPath(backend) ?? '';
    proxyTargetInput = app.getProxyTarget(backend) ?? '';
    // Auto-start a new session on the new backend if one is active
    if (app.sessionId && app.folder) {
      chat.reset();
      await app.startSession(app.folder);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="flex flex-col h-full pt-9 px-4 pb-8 relative" data-tauri-drag-region>
  <!-- Top right close button - matches ChatContainer titlebar positioning -->
  <div class="absolute top-1.5 right-4">
    <Button
      variant="ghost"
      size="icon"
      class="h-6 w-6 text-muted-foreground"
      onclick={onClose}
    >
      <X size={14} />
    </Button>
  </div>

  <div class="w-full max-w-sm mx-auto">
    <!-- Header -->
    <div class="mb-6">
      <h2 class="text-sm font-semibold text-foreground">Settings</h2>
    </div>

    <!-- Main Settings -->
    <div class="mb-6 space-y-4">
      <div>
        <span class="text-xs text-muted-foreground">Default Backend</span>
        <div class="mt-1 flex gap-1 p-1 bg-muted rounded-lg">
          {#each (['claude', 'codex'] as Backend[]) as backend}
            {@const status = backendStatuses[backend]}
            {@const isChecking = status?.installState === 'checking'}
            {@const isReady = status?.installState === 'installed' && !!status?.authenticated}
            {@const isSelectable = backend === 'codex'
              ? status?.installState === 'installed'
              : isReady}
            <button
              class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {app.defaultBackend === backend ? 'bg-background shadow-sm' : 'hover:bg-background/50'} {isSelectable ? '' : 'opacity-50'}"
              disabled={!isSelectable}
              onclick={() => switchBackend(backend)}
            >
              {#if isChecking}
                <Loader2 size={12} class="animate-spin" />
              {:else}
                {#if backend === 'claude'}
                  <ClaudeIcon size={12} />
                {:else}
                  <OpenAIIcon size={12} />
                {/if}
              {/if}
              {#if backend === 'claude'}
                Claude Code
              {:else}
                Codex CLI
              {/if}
            </button>
          {/each}
        </div>
        <p class="text-[10px] text-muted-foreground/70 mt-1">
          Switching backends will start a new session. Current backend: {app.sessionBackend ?? app.defaultBackend}.
        </p>
      </div>

      <div class="flex gap-1 p-1 bg-muted rounded-lg">
        <button
          class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {currentPreference === 'light' ? 'bg-background shadow-sm' : 'hover:bg-background/50'}"
          onclick={() => setTheme('light')}
        >
          <Sun size={12} />
          Light
        </button>
        <button
          class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {currentPreference === 'system' ? 'bg-background shadow-sm' : 'hover:bg-background/50'}"
          onclick={() => setTheme('system')}
        >
          <Monitor size={12} />
          System
        </button>
        <button
          class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {currentPreference === 'dark' ? 'bg-background shadow-sm' : 'hover:bg-background/50'}"
          onclick={() => setTheme('dark')}
        >
          <Moon size={12} />
          Dark
        </button>
      </div>

      <label class="block">
        <span class="text-xs text-muted-foreground">Custom CLI Path ({app.defaultBackend === 'claude' ? 'Claude Code' : 'Codex CLI'})</span>
        <div class="mt-1 flex gap-2">
          <input
            type="text"
            bind:value={cliPathInput}
            onkeydown={handleCliPathKeydown}
            placeholder={app.defaultBackend === 'claude' ? '/usr/local/bin/claude' : '/usr/local/bin/codex'}
            class="flex-1 h-7 px-2 text-xs bg-muted border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-ring"
          />
          <Button
            variant="outline"
            size="sm"
            class="h-7 text-xs"
            onclick={saveCliPath}
            disabled={savingCliPath}
          >
            {savingCliPath ? 'Saving...' : 'Save'}
          </Button>
        </div>
        <p class="text-[10px] text-muted-foreground/70 mt-1">
          Leave empty to use default. Requires restart.
        </p>
      </label>

      {#if currentBackendStatus?.installState === 'installed'}
        <div class="rounded-md border border-border/70 bg-muted/30 px-3 py-3">
          <div class="flex items-center justify-between gap-3">
            <div>
              <div class="text-xs font-medium text-foreground">{app.defaultBackend === 'codex' ? 'Codex Account' : 'Claude Account'}</div>
              <p class="text-[10px] text-muted-foreground mt-1">
                {#if currentBackendStatus?.authenticated}
                  Signed in via the terminal {app.defaultBackend === 'codex' ? 'Codex CLI' : 'Claude CLI'}.
                {:else}
                  Caipi uses the CLI session from your terminal. Run this command there, finish login, then refresh status here.
                {/if}
              </p>
            </div>
            {#if currentBackendStatus?.authenticated}
              <Button
                variant="outline"
                size="sm"
                class="h-7 text-xs shrink-0"
                onclick={logoutCurrentBackend}
                disabled={loggingOut}
              >
                {loggingOut ? 'Logging out...' : 'Log out'}
              </Button>
            {/if}
          </div>
          {#if !currentBackendStatus?.authenticated}
            <div class="mt-2 flex items-center gap-2">
              <code class="flex-1 text-[11px] px-2 py-1.5 rounded bg-muted border border-border text-foreground">{loginCommand}</code>
              <Button
                variant="outline"
                size="icon"
                class="h-7 w-7 shrink-0"
                onclick={() => copyCommand(loginCommand)}
              >
                {#if copiedCommand === loginCommand}
                  <Check size={12} />
                {:else}
                  <Copy size={12} />
                {/if}
              </Button>
            </div>
          {/if}
          <div class="mt-2 flex items-center justify-between gap-3 text-[10px] text-muted-foreground">
            <span>{currentBackendStatus.version ?? 'Installed'}{app.cliPath ? ` • Custom path override set` : ''}</span>
            <button
              type="button"
              class="text-primary hover:underline"
              onclick={refreshBackendStatuses}
            >
              Refresh status
            </button>
          </div>
        </div>
      {/if}

      {#if backendStatuses.codex?.installState === 'installed' || app.defaultBackend === 'codex'}
        <label class="block">
          <span class="text-xs text-muted-foreground">Codex Proxy Target</span>
          <div class="mt-1 flex gap-2">
            <input
              type="text"
              bind:value={proxyTargetInput}
              onkeydown={handleProxyTargetKeydown}
              placeholder="http://localhost:8080"
              class="flex-1 h-7 px-2 text-xs bg-muted border border-border rounded-md focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <Button
              variant="outline"
              size="sm"
              class="h-7 text-xs"
              onclick={saveProxyTarget}
              disabled={savingProxyTarget}
            >
              {savingProxyTarget ? 'Saving...' : 'Save'}
            </Button>
          </div>
          <p class="text-[10px] text-muted-foreground/70 mt-1">
            Optional. When set, Codex chats use this HTTP proxy target instead of `codex app-server`.
          </p>
        </label>
      {/if}
    </div>

    <!-- About Section -->
    <div>
      <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
        About
      </div>
      <div class="space-y-2 text-xs">
        <div class="flex justify-between">
          <span class="text-muted-foreground">Caipi Version</span>
          <span class="text-foreground">{appVersion ?? 'Loading...'}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-muted-foreground">CLI Version</span>
          <span class="text-foreground">{cliVersion ?? 'Not installed'}</span>
        </div>
      </div>
    </div>
  </div>
</div>
