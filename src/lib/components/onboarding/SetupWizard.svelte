<script lang="ts">
  import { api, type BackendStatus } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { Check, Folder, Loader2, Sun, Moon, Copy, AlertTriangle } from 'lucide-svelte';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import { app, type Backend } from '$lib/stores/app.svelte';

  let backends = $state<BackendStatus[]>([]);
  let checkingBackends = $state(true);
  let selectedBackend = $state<Backend | null>(null);
  let selectedFolder = $state<string | null>(null);
  let folderName = $state<string>('');
  let completing = $state(false);
  let error = $state<string | null>(null);

  const currentTheme = $derived($resolvedTheme);

  const claudeInstallCommand = 'npm install -g @anthropic-ai/claude-code';
  const codexInstallCommand = 'npm install -g @openai/codex';
  let copiedCommand = $state<string | null>(null);

  // Backend display info
  const backendInfo: Record<Backend, { name: string; description: string; installCmd: string }> = {
    claude: {
      name: 'Claude Code',
      description: 'Anthropic',
      installCmd: claudeInstallCommand,
    },
    codex: {
      name: 'Codex CLI',
      description: 'OpenAI',
      installCmd: codexInstallCommand,
    },
  };

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  onMount(async () => {
    await checkBackends();
  });

  async function checkBackends() {
    checkingBackends = true;
    try {
      backends = await api.checkBackendsStatus();
    } catch (e) {
      console.error('Failed to check backends:', e);
      backends = [];
    } finally {
      checkingBackends = false;
    }
  }

  function getBackendStatus(kind: Backend): BackendStatus | undefined {
    return backends.find((b) => b.kind === kind);
  }

  function isBackendReady(kind: Backend): boolean {
    const status = getBackendStatus(kind);
    return !!status?.installed && !!status?.authenticated;
  }

  function selectBackend(kind: Backend) {
    if (!isBackendReady(kind)) return;
    selectedBackend = kind;
    error = null;
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
        folderName = selected.split('/').pop() || selected;
        error = null;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to select folder';
    }
  }

  async function copyToClipboard(command: string) {
    try {
      await navigator.clipboard.writeText(command);
      copiedCommand = command;
      setTimeout(() => {
        copiedCommand = null;
      }, 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }

  async function completeSetup() {
    if (!selectedBackend || !selectedFolder) return;

    completing = true;
    error = null;

    try {
      // Set the default backend
      app.setBackend(selectedBackend);

      // Complete onboarding with default folder
      await api.completeOnboarding(selectedFolder);

      // Save to recent folders
      await api.saveRecentFolder(selectedFolder);

      // Update app state with CLI status from selected backend
      const status = getBackendStatus(selectedBackend);
      if (status) {
        app.setCliStatus({
          installed: status.installed,
          version: status.version ?? undefined,
          authenticated: status.authenticated,
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

  const canProceed = $derived(selectedBackend && selectedFolder && !completing);
  const anyBackendReady = $derived(backends.some((b) => b.installed && b.authenticated));
</script>

<div
  class="flex flex-col items-center justify-center h-full gap-6 pt-12 px-10 pb-10 relative"
  data-tauri-drag-region
>
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
  <div class="flex flex-col items-center text-center">
    <CaipiIcon size={64} />
    <h1 class="text-lg font-semibold mt-4 text-foreground">Welcome to Caipi</h1>
    <p class="text-xs text-muted-foreground mt-1">A friendly UI for AI coding assistants</p>
  </div>

  <!-- Setup Card -->
  <div class="w-[420px] rounded-lg border border-border bg-card p-5">
    <!-- Backend Selection -->
    <div class="mb-5">
      <div class="flex items-center justify-between mb-3">
        <span class="text-sm font-medium text-foreground">Choose your assistant</span>
        {#if !checkingBackends && !anyBackendReady}
          <button type="button" onclick={checkBackends} class="text-xs text-primary hover:underline">
            Recheck
          </button>
        {/if}
      </div>

      <div class="grid grid-cols-2 gap-3">
        {#each ['claude', 'codex'] as kind}
          {@const status = getBackendStatus(kind as Backend)}
          {@const info = backendInfo[kind as Backend]}
          {@const isReady = isBackendReady(kind as Backend)}
          {@const isSelected = selectedBackend === kind}
          {@const isChecking = checkingBackends}

          <button
            type="button"
            class="relative flex flex-col items-center p-4 rounded-lg border-2 transition-all {isSelected
              ? 'border-primary bg-primary/5'
              : isReady
                ? 'border-border hover:border-muted-foreground cursor-pointer'
                : 'border-border/50 opacity-50 cursor-not-allowed'}"
            onclick={() => selectBackend(kind as Backend)}
            disabled={!isReady || isChecking}
          >
            <!-- Backend name and provider -->
            <span class="text-sm font-medium text-foreground {!isReady && 'opacity-50'}">
              {info.name}
            </span>
            <span class="text-xs text-muted-foreground mt-0.5 {!isReady && 'opacity-50'}">
              {info.description}
            </span>

            <!-- Status indicator -->
            <div class="mt-3 flex items-center gap-1.5">
              {#if isChecking}
                <Loader2 size={12} class="animate-spin text-muted-foreground" />
                <span class="text-xs text-muted-foreground">Checking...</span>
              {:else if isReady}
                <Check size={12} class="text-green-500" />
                <span class="text-xs text-muted-foreground">Ready</span>
              {:else if status?.installed && !status?.authenticated}
                <AlertTriangle size={12} class="text-yellow-500" />
                <span class="text-xs text-muted-foreground">Not authenticated</span>
              {:else}
                <span class="w-2 h-2 rounded-full bg-muted-foreground/30"></span>
                <span class="text-xs text-muted-foreground">Not installed</span>
              {/if}
            </div>

            <!-- Selected checkmark -->
            {#if isSelected}
              <div class="absolute top-2 right-2">
                <Check size={14} class="text-primary" />
              </div>
            {/if}
          </button>
        {/each}
      </div>

      <!-- Install instructions for non-ready backends -->
      {#if !checkingBackends && !anyBackendReady}
        <div class="mt-4 p-3 rounded-md bg-muted/50 border border-border">
          <p class="text-xs text-muted-foreground mb-2">
            Install at least one CLI to continue:
          </p>
          <div class="space-y-2">
            {#each ['claude', 'codex'] as kind}
              {@const info = backendInfo[kind as Backend]}
              {@const isCopied = copiedCommand === info.installCmd}
              <div class="flex items-center gap-2">
                <code
                  class="flex-1 text-xs px-2 py-1.5 rounded bg-background border border-border text-muted-foreground overflow-x-auto"
                >
                  {info.installCmd}
                </code>
                <Button
                  variant="outline"
                  size="icon"
                  class="shrink-0 h-7 w-7 {isCopied ? 'text-green-500' : ''}"
                  onclick={() => copyToClipboard(info.installCmd)}
                  title="Copy to clipboard"
                >
                  {#if isCopied}
                    <Check size={12} />
                  {:else}
                    <Copy size={12} />
                  {/if}
                </Button>
              </div>
            {/each}
          </div>
          <button
            type="button"
            onclick={checkBackends}
            class="text-xs text-primary mt-3 hover:underline"
          >
            Recheck installation
          </button>
        </div>
      {/if}
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

    {#if error}
      <div class="text-xs text-red-500 mt-3">{error}</div>
    {/if}
  </div>

  <!-- Get Started Button -->
  <Button onclick={completeSetup} disabled={!canProceed} class="px-6">
    {#if completing}
      <span class="flex items-center gap-2">
        <Loader2 size={14} class="animate-spin" />
        Setting up...
      </span>
    {:else}
      Get Started
    {/if}
  </Button>

  {#if !anyBackendReady && !checkingBackends}
    <p class="text-xs text-muted-foreground/50">Install a CLI to continue</p>
  {:else if !selectedBackend}
    <p class="text-xs text-muted-foreground/50">Select an assistant to continue</p>
  {:else if !selectedFolder}
    <p class="text-xs text-muted-foreground/50">Select a folder to continue</p>
  {/if}
</div>
