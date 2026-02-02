<script lang="ts">
  import { api, type CliInstallStatus, type CliAuthStatus } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { Check, Folder, Loader2, Sun, Moon, Copy, AlertTriangle } from 'lucide-svelte';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { theme } from '$lib/stores/theme.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { isWindows } from '$lib/utils/platform';

  let cliStatus = $state<CliInstallStatus | null>(null);
  let authStatus = $state<CliAuthStatus | null>(null);
  let checkingCli = $state(true);
  let selectedFolder = $state<string | null>(null);
  let folderName = $state<string>('');
  let completing = $state(false);
  let error = $state<string | null>(null);

  const currentTheme = $derived(theme.resolved);

  // Platform-specific install commands
  const macLinuxCommand = 'curl -fsSL https://claude.ai/install.sh | bash';
  const windowsCommand = 'irm https://claude.ai/install.ps1 | iex';
  const installCommand = $derived(isWindows() ? windowsCommand : macLinuxCommand);
  let copied = $state(false);

  function toggleTheme() {
    theme.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  onMount(async () => {
    await checkCliStatus();
  });

  async function checkCliStatus() {
    checkingCli = true;
    try {
      const status = await api.checkCliInstalled();
      cliStatus = status;

      // Only check auth if CLI is installed
      if (status.installed) {
        const auth = await api.checkCliAuthenticated();
        authStatus = auth;
      } else {
        authStatus = { authenticated: false };
      }
    } catch (e) {
      console.error('Failed to check CLI:', e);
      cliStatus = { installed: false };
      authStatus = { authenticated: false };
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
      await api.completeOnboarding(selectedFolder);

      // Save to recent folders
      await api.saveRecentFolder(selectedFolder);

      // Update app state
      app.setCliStatus({
        installed: true,
        version: cliStatus.version,
        authenticated: authStatus?.authenticated ?? false,
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

  const canProceed = $derived(cliStatus?.installed && authStatus?.authenticated && selectedFolder && !completing);
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
  <div class="w-[380px] rounded-lg border border-border bg-card p-5">
    <!-- CLI Status -->
    <div class="mb-5">
      <div class="flex items-center gap-2 mb-2">
        <span class="text-sm font-medium text-foreground">Claude Code CLI</span>
        {#if checkingCli}
          <Loader2 size={14} class="animate-spin text-muted-foreground" />
        {:else if cliStatus?.installed && authStatus?.authenticated}
          <Check size={14} class="text-green-500" />
        {:else if cliStatus?.installed && !authStatus?.authenticated}
          <AlertTriangle size={14} class="text-yellow-500" />
        {:else}
          <span class="w-2 h-2 rounded-full bg-red-500"></span>
        {/if}
      </div>

      {#if checkingCli}
        <p class="text-xs text-muted-foreground">Checking installation...</p>
      {:else if cliStatus?.installed && authStatus?.authenticated}
        <p class="text-xs text-muted-foreground">
          Installed {cliStatus.version ? `(${cliStatus.version})` : ''}
        </p>
      {:else if cliStatus?.installed && !authStatus?.authenticated}
        <p class="text-xs text-muted-foreground mb-3">
          Installed but not authenticated. Run this in your terminal:
        </p>
        <div class="flex items-center gap-2 mb-2">
          <code class="flex-1 text-xs px-3 py-2 rounded bg-muted border border-border text-muted-foreground">
            claude
          </code>
        </div>
        <p class="text-xs text-muted-foreground/70 mb-2">
          Follow the prompts to log in to your Anthropic account.
        </p>
        <button
          type="button"
          onclick={checkCliStatus}
          class="text-xs text-primary hover:underline"
        >
          Recheck authentication
        </button>
      {:else}
        <p class="text-xs text-muted-foreground mb-3">
          Required to use Caipi. Run this in your terminal:
        </p>
        <div class="flex items-center gap-2">
          <code class="flex-1 text-xs px-3 py-2 rounded overflow-x-auto bg-muted border border-border text-muted-foreground">
            {installCommand}
          </code>
          <Button
            variant="outline"
            size="icon"
            class="shrink-0 h-8 w-8 {copied ? 'text-green-500' : ''}"
            onclick={copyToClipboard}
            title="Copy to clipboard"
          >
            {#if copied}
              <Check size={14} />
            {:else}
              <Copy size={14} />
            {/if}
          </Button>
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

  {#if !cliStatus?.installed && !checkingCli}
    <p class="text-xs text-muted-foreground/50">
      Install Claude Code CLI to continue
    </p>
  {:else if cliStatus?.installed && !authStatus?.authenticated && !checkingCli}
    <p class="text-xs text-muted-foreground/50">
      Authenticate Claude Code CLI to continue
    </p>
  {:else if !selectedFolder}
    <p class="text-xs text-muted-foreground/50">
      Select a folder to continue
    </p>
  {/if}
</div>
