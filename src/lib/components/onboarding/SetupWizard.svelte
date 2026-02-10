<script lang="ts">
  import { api, type BackendCliStatus } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { Check, Folder, Loader2, Sun, Moon, Copy, AlertTriangle } from 'lucide-svelte';
  import { CaipiIcon, ClaudeIcon, OpenAIIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { theme } from '$lib/stores/theme.svelte';
  import { app } from '$lib/stores/app.svelte';
  import type { Backend } from '$lib/config/backends';

  let backendStatuses = $state<BackendCliStatus[]>([]);
  let checkingCli = $state(true);
  let selectedFolder = $state<string | null>(null);
  let selectedBackend = $state<Backend | null>(null);
  let folderName = $state<string>('');
  let completing = $state(false);
  let error = $state<string | null>(null);

  const currentTheme = $derived(theme.resolved);

  let copiedBackend = $state<string | null>(null);

  function toggleTheme() {
    theme.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  onMount(async () => {
    await checkCliStatus();
  });

  async function checkCliStatus() {
    checkingCli = true;
    try {
      backendStatuses = await api.checkAllBackendsStatus();
      const ready = backendStatuses.filter(b => b.installed && b.authenticated);
      if (ready.length === 1) {
        selectedBackend = ready[0].backend as Backend;
      }
    } catch (e) {
      console.error('Failed to check CLI:', e);
      backendStatuses = [];
    } finally {
      checkingCli = false;
    }
  }

  async function selectFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select your default project folder',
      });

      if (selected && typeof selected === 'string') {
        const valid = await api.validateFolder(selected);
        if (!valid) {
          error = 'Cannot access this folder. Please choose another.';
          return;
        }
        selectedFolder = selected;
        // Handle both Unix and Windows paths
        const normalized = selected.replace(/\\/g, '/');
        folderName = normalized.split('/').pop() || selected;
        error = null;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to select folder';
    }
  }

  async function copyToClipboard(text: string, backend: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedBackend = backend;
      setTimeout(() => {
        copiedBackend = null;
      }, 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }

  async function completeSetup() {
    if (!selectedFolder || !selectedBackend) return;

    completing = true;
    error = null;

    try {
      // Complete onboarding with default folder
      await api.completeOnboarding(selectedFolder, selectedBackend);
      app.defaultBackend = selectedBackend;

      // Save to recent folders
      await api.saveRecentFolder(selectedFolder);

      // Update app state
      const selected = backendStatuses.find(s => s.backend === selectedBackend);
      if (selected) {
        app.setCliStatus({
          installed: selected.installed,
          version: selected.version,
          authenticated: selected.authenticated,
          path: selected.path,
        });
      }

      // Start session
      await app.startSession(selectedFolder);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to complete setup';
    } finally {
      completing = false;
    }
  }

  const readyBackends = $derived(backendStatuses.filter(b => b.installed && b.authenticated));
  const canProceed = $derived(!!selectedBackend && !!selectedFolder && !completing);
  const backendRows = $derived(
    (['claude', 'codex'] as Backend[]).map((id) => {
      const status = backendStatuses.find((b) => b.backend === id);
      return {
        backend: id,
        installed: status?.installed ?? false,
        authenticated: status?.authenticated ?? false,
        version: status?.version,
        installHint: status?.installHint,
        authHint: status?.authHint,
      };
    })
  );
  const helperText = $derived(
    !readyBackends.length && !checkingCli
      ? 'Install and authenticate at least one backend to continue'
      : ''
  );
</script>

<div class="flex flex-col items-center justify-center h-full gap-6 pt-12 px-10 pb-10 relative" data-tauri-drag-region>
  <!-- Top right controls -->
  <div class="absolute top-3 right-4 flex items-center gap-1">
    <Button
      variant="ghost"
      size="icon"
      class="h-8 w-8 text-muted-foreground"
      onclick={toggleTheme}
    >
      {#if currentTheme === 'dark'}
        <Sun size={16} />
      {:else}
        <Moon size={16} />
      {/if}
    </Button>
  </div>

  <!-- Logo and Title -->
  <div class="flex items-center gap-4">
    <CaipiIcon size={72} />
    <div class="text-left">
      <h1 class="text-lg font-semibold text-foreground">
        Welcome to Caipi
      </h1>
      <p class="text-xs text-muted-foreground mt-1">
        A friendly UI for coding CLIs
      </p>
    </div>
  </div>

  <!-- Setup Card -->
  <div class="w-[380px] rounded-lg border border-border bg-card p-5">
    <!-- Backend Status -->
    <div class="mb-5">
      <div class="space-y-2">
        {#each backendRows as backend}
          {@const ready = backend.installed && backend.authenticated}
          <button
            type="button"
            class="w-full text-left rounded-md border px-3 py-2 transition-colors {ready ? 'border-border hover:bg-muted/40' : 'border-border/40 opacity-60'} {selectedBackend === backend.backend ? 'ring-2 ring-primary/70 border-primary/60 bg-muted/50' : ''}"
            disabled={!ready}
            onclick={() => selectedBackend = backend.backend as Backend}
          >
            <div class="flex items-center gap-3">
              <div class="shrink-0 text-foreground/70">
                {#if backend.backend === 'claude'}
                  <ClaudeIcon size={32} />
                {:else}
                  <OpenAIIcon size={32} />
                {/if}
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center justify-between gap-2">
                  <div class="text-sm text-foreground">{backend.backend === 'claude' ? 'Claude Code' : 'Codex CLI'}</div>
                  {#if checkingCli}
                    <Loader2 size={14} class="text-muted-foreground animate-spin" />
                  {:else if ready}
                    <Check size={14} class="text-green-500" />
                  {:else if backend.installed}
                    <AlertTriangle size={14} class="text-yellow-500" />
                  {:else}
                    <span class="w-2 h-2 rounded-full bg-red-500"></span>
                  {/if}
                </div>
                <div class="text-xs text-muted-foreground mt-0.5">
                  {#if checkingCli}
                    Checking...
                  {:else if ready}
                    {backend.version ?? 'Installed'}
                  {:else if backend.installed}
                    Not authenticated
                  {:else}
                    Not installed
                  {/if}
                </div>
              </div>
            </div>
            {#if !checkingCli && !backend.installed && backend.installHint}
              <div class="mt-2 flex items-center gap-2">
                <code class="flex-1 text-[10px] px-2 py-1 rounded bg-muted border border-border text-muted-foreground">{backend.installHint}</code>
                <Button
                  variant="outline"
                  size="icon"
                  class="shrink-0 h-7 w-7 {copiedBackend === backend.backend ? 'text-green-500' : ''}"
                  onclick={(e) => {
                    e.stopPropagation();
                    copyToClipboard(backend.installHint!, backend.backend);
                  }}
                >
                  {#if copiedBackend === backend.backend}
                    <Check size={12} />
                  {:else}
                    <Copy size={12} />
                  {/if}
                </Button>
              </div>
            {:else if !checkingCli && backend.installed && !backend.authenticated && backend.authHint}
              <p class="text-[10px] text-muted-foreground mt-2">{backend.authHint}</p>
            {/if}
          </button>
        {/each}
      </div>
      <button
        type="button"
        onclick={checkCliStatus}
        class="text-xs text-primary mt-3 hover:underline"
      >
        Recheck backends
      </button>
    </div>

    <!-- Divider -->
    <div class="w-full h-px mb-5 bg-border"></div>

    <!-- Default Folder Selection -->
    <div>
      <div class="text-sm font-medium text-foreground mb-2">Default Project Folder</div>
      <p class="text-xs text-muted-foreground mb-3">
        Choose where you usually work. The Desktop is a solid choice.
      </p>

      {#if selectedFolder}
        <button
          type="button"
          onclick={selectFolder}
          class="w-full flex items-center gap-2.5 py-2.5 px-3 rounded-md cursor-pointer transition-colors text-left border border-border hover:bg-muted/50"
        >
          <span class="text-muted-foreground">
            <Folder size={16} />
          </span>
          <div class="flex-1 min-w-0">
            <div class="text-sm text-foreground truncate">{folderName}</div>
            <div class="text-xs text-muted-foreground/70 truncate">{selectedFolder}</div>
          </div>
          <span class="text-xs text-muted-foreground">Change</span>
        </button>
      {:else}
        <button
          type="button"
          onclick={selectFolder}
          class="w-full flex items-center justify-center gap-2 py-3 px-4 rounded-md cursor-pointer transition-colors border border-dashed border-border hover:border-muted-foreground hover:bg-muted/50"
        >
          <Folder size={16} class="text-muted-foreground" />
          <span class="text-sm text-muted-foreground">Select a folder</span>
        </button>
      {/if}
    </div>

    <div class="text-xs mt-3 min-h-[1rem] {error ? 'text-red-500' : 'text-transparent'}">
      {error ?? '\u00A0'}
    </div>
  </div>

  <!-- Get Started Button -->
  <Button
    onclick={completeSetup}
    disabled={!canProceed}
    class="px-6"
  >
    {#if completing}
      <span class="flex items-center gap-2">
        <Loader2 size={14} class="animate-spin" />
        Setting up...
      </span>
    {:else}
      Get Started
    {/if}
  </Button>

  <p class="text-xs text-muted-foreground/50 min-h-[1rem]">
    {helperText || '\u00A0'}
  </p>
</div>
