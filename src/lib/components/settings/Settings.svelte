<script lang="ts">
  import { X, Sun, Moon, Monitor, LogOut, Loader2 } from 'lucide-svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui';
  import { themeStore, type ThemePreference } from '$lib/stores/theme';
  import { app } from '$lib/stores/app.svelte';
  import { api } from '$lib/api';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let appVersion = $state<string | null>(null);
  let deactivating = $state(false);

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
  const activatedAt = $derived(
    app.license?.activatedAt
      ? new Date(app.license.activatedAt * 1000).toLocaleDateString()
      : null
  );


  onMount(async () => {
    try {
      appVersion = await getVersion();
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
      <h2 class="text-sm font-semibold text-foreground mb-1">Settings</h2>
      <p class="text-xs text-muted-foreground">Customize your experience</p>
    </div>

    <!-- Appearance Section -->
    <div class="mb-8">
      <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
        Appearance
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
    </div>

    <!-- About Section -->
    <div class="mb-8">
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
      </div>
    </div>

    <!-- License Section -->
    <div>
      <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
        License
      </div>
      <div class="space-y-2 text-xs">
        <div class="flex justify-between">
          <span class="text-muted-foreground">Status</span>
          <span class="text-green-500">Active</span>
        </div>

        {#if email}
          <div class="flex justify-between">
            <span class="text-muted-foreground">Email</span>
            <span class="text-foreground truncate max-w-[200px]" title={email}>{email}</span>
          </div>
        {/if}

        {#if activatedAt}
          <div class="flex justify-between">
            <span class="text-muted-foreground">Activated</span>
            <span class="text-foreground">{activatedAt}</span>
          </div>
        {/if}

        {#if licenseKey}
          <div class="flex justify-between items-center">
            <span class="text-muted-foreground">Key</span>
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
