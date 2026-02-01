<script lang="ts">
  import { X, Sun, Moon, Monitor, LogOut, Loader2 } from 'lucide-svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui';
  import { themeStore, type ThemePreference } from '$lib/stores/theme';
  import { app, type Backend } from '$lib/stores/app.svelte';
  import { api, type BackendStatus } from '$lib/api';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let appVersion = $state<string | null>(null);
  let deactivating = $state(false);
  let cliPathInput = $state(app.cliPath ?? '');
  let savingCliPath = $state(false);
  let backends = $state<BackendStatus[]>([]);
  let loadingBackends = $state(true);

  // Backend display names
  const backendNames: Record<Backend, string> = {
    claude: 'Claude',
    codex: 'Codex',
  };

  // Strip "(Claude Code)" or similar suffixes from version string
  const cliVersion = $derived(() => {
    const version = app.cliStatus?.version;
    if (!version) return null;
    return version.replace(/\s*\(.*\)\s*$/, '').trim();
  });

  const currentPreference = $derived($themeStore.preference);

  // License info
  const licenseKey = $derived(app.license?.licenseKey ?? null);
  const email = $derived(app.license?.email ?? null);


  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch (e) {
      console.error('Failed to get app version:', e);
    }

    // Load backend status
    try {
      backends = await api.checkBackendsStatus();
    } catch (e) {
      console.error('Failed to check backends:', e);
    } finally {
      loadingBackends = false;
    }
  });

  function isBackendAvailable(kind: Backend): boolean {
    const status = backends.find((b) => b.kind === kind);
    return !!status?.installed && !!status?.authenticated;
  }

  function selectBackend(kind: Backend) {
    if (!isBackendAvailable(kind)) return;
    app.setBackend(kind);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }

  function setTheme(preference: ThemePreference) {
    themeStore.setPreference(preference);
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
      await api.setCliPath(pathToSave);
      app.setCliPath(pathToSave ?? null);
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
      <!-- Default Backend -->
      <div>
        <span class="text-xs text-muted-foreground">Default Backend</span>
        <p class="text-[10px] text-muted-foreground/70 mt-0.5">Applies to new sessions</p>
        <div class="mt-1 flex gap-1 p-1 bg-muted rounded-lg">
          {#each ['claude', 'codex'] as kind}
            {@const isAvailable = isBackendAvailable(kind as Backend)}
            {@const isSelected = app.backend === kind}
            <button
              class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {isSelected
                ? 'bg-background shadow-sm'
                : isAvailable
                  ? 'hover:bg-background/50'
                  : 'opacity-50 cursor-not-allowed'}"
              onclick={() => selectBackend(kind as Backend)}
              disabled={!isAvailable || loadingBackends}
            >
              {#if loadingBackends}
                <Loader2 size={12} class="animate-spin" />
              {/if}
              {backendNames[kind as Backend]}
              {#if !loadingBackends && !isAvailable}
                <span class="text-muted-foreground/50">(unavailable)</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>

      <!-- Theme -->
      <div>
        <span class="text-xs text-muted-foreground">Theme</span>
        <div class="mt-1 flex gap-1 p-1 bg-muted rounded-lg">
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
      </div>

      <label class="block">
        <span class="text-xs text-muted-foreground">Custom CLI Path</span>
        <div class="mt-1 flex gap-2">
          <input
            type="text"
            bind:value={cliPathInput}
            onkeydown={handleCliPathKeydown}
            placeholder="/usr/local/bin/claude"
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
          <span class="text-muted-foreground">Claude CLI Version</span>
          <span class="text-foreground">{cliVersion() ?? 'Not installed'}</span>
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
