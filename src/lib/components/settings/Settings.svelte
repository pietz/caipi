<script lang="ts">
  import { X, Sun, Moon, Monitor, LogOut, Loader2 } from 'lucide-svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui';
  import { ClaudeIcon, OpenAIIcon } from '$lib/components/icons';
  import { theme, type ThemePreference } from '$lib/stores/theme.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { api } from '$lib/api';
  import type { Backend } from '$lib/config/backends';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let appVersion = $state<string | null>(null);
  let deactivating = $state(false);
  let cliPathInput = $state(app.cliPath ?? '');
  let savingCliPath = $state(false);
  let backendStatuses = $state<Record<string, { installed: boolean; authenticated: boolean; version?: string }>>({});

  // Strip extra suffixes from version string
  const cliVersion = $derived.by(() => {
    const version = backendStatuses[app.defaultBackend]?.version;
    if (!version) return null;
    return version.replace(/\s*\(.*\)\s*$/, '').trim();
  });

  const currentPreference = $derived(theme.preference);

  // License info
  const licenseKey = $derived(app.license?.licenseKey ?? null);
  const email = $derived(app.license?.email ?? null);


  onMount(async () => {
    try {
      const [version, statuses] = await Promise.all([
        getVersion(),
        api.checkAllBackendsStatus()
      ]);
      appVersion = version;
      backendStatuses = Object.fromEntries(
        statuses.map(s => [s.backend, { installed: s.installed, authenticated: s.authenticated, version: s.version }])
      );
    } catch (e) {
      console.error('Failed to get app version:', e);
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }

  function setTheme(preference: ThemePreference) {
    theme.setPreference(preference);
  }

  async function disconnectLicense() {
    if (!confirm('Are you sure you want to disconnect your license? You will need to re-enter your license key.')) {
      return;
    }

    deactivating = true;
    try {
      await api.clearLicense();
      app.setLicense(null);
      onClose();
      app.setScreen('license');
    } catch (e) {
      console.error('Failed to disconnect license:', e);
    } finally {
      deactivating = false;
    }
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

  function handleCliPathKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      saveCliPath();
    }
  }

  async function switchBackend(backend: Backend) {
    if (backend === app.defaultBackend) return;
    await app.setDefaultBackend(backend);
    cliPathInput = app.getCliPath(backend) ?? '';
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
            {@const isReady = !!backendStatuses[backend]?.installed && !!backendStatuses[backend]?.authenticated}
            <button
              class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {app.defaultBackend === backend ? 'bg-background shadow-sm' : 'hover:bg-background/50'} {isReady ? '' : 'opacity-50'}"
              disabled={!isReady}
              onclick={() => switchBackend(backend)}
            >
              {#if backend === 'claude'}
                <ClaudeIcon size={12} />
                Claude Code
              {:else}
                <OpenAIIcon size={12} />
                Codex CLI
              {/if}
            </button>
          {/each}
        </div>
        <p class="text-[10px] text-muted-foreground/70 mt-1">
          Applies to new sessions only. Current chat continues on {app.sessionBackend ?? app.defaultBackend}.
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
        {#if email}
          <div class="flex justify-between">
            <span class="text-muted-foreground">License Email</span>
            <span class="text-foreground truncate max-w-[200px]" title={email}>{email}</span>
          </div>
        {/if}
        {#if licenseKey}
          <div class="flex justify-between items-center">
            <span class="text-muted-foreground">License Key</span>
            <code class="text-foreground text-[10px] bg-muted px-1.5 py-0.5 rounded">{licenseKey}</code>
          </div>
        {/if}
      </div>

      <Button
        variant="ghost"
        size="sm"
        class="w-full mt-4 h-7 text-xs text-muted-foreground hover:text-red-500"
        onclick={disconnectLicense}
        disabled={deactivating}
      >
        {#if deactivating}
          <Loader2 size={12} class="animate-spin mr-1.5" />
          Disconnecting...
        {:else}
          <LogOut size={12} class="mr-1.5" />
          Disconnect License
        {/if}
      </Button>
    </div>
  </div>
</div>
