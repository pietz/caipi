<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { CaipiIcon, CheckIcon, SpinnerIcon } from '$lib/components/icons';
  import { appStore, type CliStatus } from '$lib/stores';

  interface CliInstallStatus {
    installed: boolean;
    version: string | null;
    path: string | null;
  }

  interface CliAuthStatus {
    authenticated: boolean;
  }

  let installStatus = $state<CliInstallStatus | null>(null);
  let authStatus = $state<CliAuthStatus | null>(null);
  let checkingInstall = $state(true);
  let checkingAuth = $state(false);
  let error = $state<string | null>(null);

  async function checkCliStatus() {
    checkingInstall = true;
    checkingAuth = false;
    installStatus = null;
    authStatus = null;
    error = null;

    try {
      // First check installation
      installStatus = await invoke<CliInstallStatus>('check_cli_installed');
      checkingInstall = false;

      // Only check auth if installed
      if (installStatus.installed) {
        checkingAuth = true;
        authStatus = await invoke<CliAuthStatus>('check_cli_authenticated');
        checkingAuth = false;
      }

      // Update app store with combined status
      const status: CliStatus = {
        installed: installStatus.installed,
        version: installStatus.version,
        authenticated: authStatus?.authenticated ?? false,
        path: installStatus.path,
      };
      appStore.setCliStatus(status);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to check CLI status';
      checkingInstall = false;
      checkingAuth = false;
    }
  }

  function proceed() {
    appStore.setScreen('folder');
  }

  // Check on mount
  $effect(() => {
    checkCliStatus();
  });

  const canProceed = $derived(installStatus?.installed && authStatus?.authenticated);
</script>

<div class="flex flex-col items-center justify-center h-full gap-8 pt-12 px-10 pb-10" data-tauri-drag-region>
  <!-- Logo and Title -->
  <div class="flex flex-col items-center text-center">
    <CaipiIcon size={64} />
    <h1 class="text-lg font-semibold mt-4 text-primary">
      Caipi
    </h1>
    <p class="text-xs text-muted mt-1">
      A friendly UI for Claude Code
    </p>
  </div>

  <!-- System Check Card -->
  <div
    class="w-[280px] rounded-lg p-4 px-5"
    style="background: var(--card); border: 1px solid var(--border);"
  >
    <div class="text-xs font-medium text-muted uppercase tracking-[0.5px] mb-3">
      System Check
    </div>

    <div class="flex flex-col gap-2.5">
      <!-- CLI Installation Check -->
      <div class="flex items-center justify-between">
        <span class="text-sm text-secondary">Claude CLI installed</span>
        <span class="text-accent">
          {#if checkingInstall}
            <SpinnerIcon size={14} />
          {:else if installStatus?.installed}
            <CheckIcon size={14} />
          {:else}
            <span class="text-muted">✗</span>
          {/if}
        </span>
      </div>

      <!-- Authentication Check -->
      <div class="flex items-center justify-between">
        <span class="text-sm text-secondary">Authenticated</span>
        <span class="text-accent">
          {#if checkingInstall || checkingAuth}
            <SpinnerIcon size={14} />
          {:else if authStatus?.authenticated}
            <CheckIcon size={14} />
          {:else}
            <span class="text-muted">✗</span>
          {/if}
        </span>
      </div>
    </div>

    {#if error}
      <div class="mt-3 text-xs text-red-500">{error}</div>
    {/if}

    {#if !checkingInstall && !checkingAuth && !installStatus?.installed}
      <div class="mt-3 text-xs text-secondary">
        Install with: <code class="bg-muted px-1.5 py-0.5 rounded text-xs">npm install -g @anthropic-ai/claude-code</code>
      </div>
    {/if}

    {#if !checkingInstall && !checkingAuth && installStatus?.installed && !authStatus?.authenticated}
      <div class="mt-3 text-xs text-secondary">
        Run: <code class="bg-muted px-1.5 py-0.5 rounded text-xs">claude login</code>
      </div>
    {/if}
  </div>

  <!-- Continue Button -->
  <button
    onclick={canProceed ? proceed : checkCliStatus}
    disabled={checkingInstall || checkingAuth}
    class="px-6 py-2 text-sm font-medium rounded-md transition-all duration-150"
    style="
      background-color: {canProceed ? 'var(--primary)' : 'var(--secondary)'};
      color: {canProceed ? '#fff' : 'var(--text-dim)'};
      opacity: {checkingInstall || checkingAuth ? 0.6 : 1};
      cursor: {checkingInstall || checkingAuth ? 'not-allowed' : 'pointer'};
    "
  >
    {canProceed ? 'Continue' : 'Check Again'}
  </button>
</div>
