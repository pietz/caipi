<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { CaipiIcon, CheckIcon, FolderIcon, SpinnerIcon, SunIcon, MoonIcon } from '$lib/components/icons';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import { app } from '$lib/stores/app.svelte';

  interface CliInstallStatus {
    installed: boolean;
    version: string | null;
    path: string | null;
  }

  let cliStatus = $state<CliInstallStatus | null>(null);
  let checkingCli = $state(true);
  let selectedFolder = $state<string | null>(null);
  let folderName = $state<string>('');
  let completing = $state(false);
  let error = $state<string | null>(null);

  const currentTheme = $derived($resolvedTheme);

  const installCommand = 'curl -fsSL https://claude.ai/install.sh | bash';
  let copied = $state(false);

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  onMount(async () => {
    await checkCliStatus();
  });

  async function checkCliStatus() {
    checkingCli = true;
    try {
      cliStatus = await invoke<CliInstallStatus>('check_cli_installed');
    } catch (e) {
      console.error('Failed to check CLI:', e);
      cliStatus = { installed: false, version: null, path: null };
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
        const valid = await invoke<boolean>('validate_folder', { path: selected });
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

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(installCommand);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }

  async function completeSetup() {
    if (!cliStatus?.installed || !selectedFolder) return;

    completing = true;
    error = null;

    try {
      // Complete onboarding with default folder
      await invoke('complete_onboarding', { defaultFolder: selectedFolder });

      // Save to recent folders
      await invoke('save_recent_folder', { path: selectedFolder });

      // Update app state
      app.setCliStatus({
        installed: true,
        version: cliStatus.version,
        authenticated: true,
        path: cliStatus.path,
      });

      // Start session
      await app.startSession(selectedFolder);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to complete setup';
    } finally {
      completing = false;
    }
  }

  const canProceed = $derived(cliStatus?.installed && selectedFolder && !completing);
</script>

<div class="flex flex-col items-center justify-center h-full gap-6 pt-12 px-10 pb-10 relative" data-tauri-drag-region>
  <!-- Top right controls -->
  <div class="absolute top-3 right-4 flex items-center gap-2">
    <button
      type="button"
      onclick={toggleTheme}
      class="p-1 rounded transition-all duration-100 text-dim hover:bg-hover hover:text-foreground"
      title={currentTheme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
    >
      {#if currentTheme === 'dark'}
        <SunIcon size={16} />
      {:else}
        <MoonIcon size={16} />
      {/if}
    </button>
  </div>

  <!-- Logo and Title -->
  <div class="flex flex-col items-center text-center">
    <CaipiIcon size={64} />
    <h1 class="text-lg font-semibold mt-4 text-foreground">
      Welcome to Caipi
    </h1>
    <p class="text-xs text-muted-foreground mt-1">
      A friendly UI for Claude Code
    </p>
  </div>

  <!-- Setup Card -->
  <div
    class="w-[380px] rounded-lg border bg-card p-5"
  >
    <!-- CLI Status -->
    <div class="mb-5">
      <div class="flex items-center gap-2 mb-2">
        <span class="text-sm font-medium text-foreground">Claude Code CLI</span>
        {#if checkingCli}
          <SpinnerIcon size={14} />
        {:else if cliStatus?.installed}
          <span class="text-green-500"><CheckIcon size={14} /></span>
        {:else}
          <span class="w-2 h-2 rounded-full bg-red-500"></span>
        {/if}
      </div>

      {#if checkingCli}
        <p class="text-xs text-muted-foreground">Checking installation...</p>
      {:else if cliStatus?.installed}
        <p class="text-xs text-muted-foreground">
          Installed {cliStatus.version ? `(${cliStatus.version})` : ''}
        </p>
      {:else}
        <p class="text-xs text-muted-foreground mb-3">
          Required to use Caipi. Run this in your terminal:
        </p>
        <div class="flex items-center gap-2">
          <code
            class="flex-1 text-xs px-3 py-2 rounded overflow-x-auto bg-muted border border-border text-muted-foreground"
          >
            {installCommand}
          </code>
          <button
            type="button"
            onclick={copyToClipboard}
            class="shrink-0 p-2 rounded transition-all duration-150 bg-accent text-muted-foreground hover:bg-accent/80"
            class:text-primary={copied}
            title="Copy to clipboard"
          >
            {#if copied}
              <CheckIcon size={14} />
            {:else}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
            {/if}
          </button>
        </div>
        <button
          type="button"
          onclick={checkCliStatus}
          class="text-xs text-primary mt-3 hover:underline"
        >
          Recheck installation
        </button>
      {/if}
    </div>

    <!-- Divider -->
    <div class="w-full h-px mb-5 bg-border"></div>

    <!-- Default Folder Selection -->
    <div>
      <div class="text-sm font-medium text-foreground mb-2">Default Project Folder</div>
      <p class="text-xs text-muted-foreground mb-3">
        Choose where you usually work. You can always open other projects later.
      </p>

      {#if selectedFolder}
        <button
          type="button"
          onclick={selectFolder}
          class="w-full flex items-center gap-2.5 py-2.5 px-3 rounded-md cursor-pointer transition-colors duration-100 text-left border border-border"
          style="background: var(--hover);"
        >
          <span class="text-folder">
            <FolderIcon size={16} />
          </span>
          <div class="flex-1 min-w-0">
            <div class="text-sm text-foreground truncate">{folderName}</div>
            <div class="text-xs text-dim truncate">{selectedFolder}</div>
          </div>
          <span class="text-xs text-muted-foreground">Change</span>
        </button>
      {:else}
        <button
          type="button"
          onclick={selectFolder}
          class="w-full flex items-center justify-center gap-2 py-3 px-4 rounded-md cursor-pointer transition-colors duration-100"
          style="
            border: 1px dashed var(--border-hover);
            background: transparent;
          "
        >
          <FolderIcon size={16} />
          <span class="text-sm text-muted-foreground">Select a folder</span>
        </button>
      {/if}
    </div>

    {#if error}
      <div class="text-xs text-red-500 mt-3">{error}</div>
    {/if}
  </div>

  <!-- Get Started Button -->
  <button
    type="button"
    onclick={completeSetup}
    disabled={!canProceed}
    class="px-6 py-2.5 rounded-md text-sm font-medium transition-all duration-150"
    style="
      background: {canProceed ? 'var(--accent-blue)' : 'hsl(var(--card))'};
      color: {canProceed ? 'white' : 'var(--text-dim)'};
      cursor: {canProceed ? 'pointer' : 'not-allowed'};
      opacity: {completing ? '0.7' : '1'};
    "
  >
    {#if completing}
      <span class="flex items-center gap-2">
        <SpinnerIcon size={14} />
        Setting up...
      </span>
    {:else}
      Get Started
    {/if}
  </button>

  {#if !cliStatus?.installed && !checkingCli}
    <p class="text-xs text-dim">
      Install Claude Code CLI to continue
    </p>
  {:else if !selectedFolder}
    <p class="text-xs text-dim">
      Select a folder to continue
    </p>
  {/if}
</div>
