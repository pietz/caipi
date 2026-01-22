<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { CheckCircle, XCircle, Loader, Terminal, ExternalLink } from 'lucide-svelte';
  import { Button, Card } from '$lib/components/ui';
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

<div class="flex flex-col items-center justify-center h-full p-8">
  <div class="max-w-md w-full space-y-8">
    <!-- Logo/Title -->
    <div class="text-center">
      <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-primary/10 mb-4">
        <Terminal class="w-8 h-8 text-primary" />
      </div>
      <h1 class="text-3xl font-bold">Caipi</h1>
      <p class="text-muted-foreground mt-2">
        A friendly chat interface for Claude Code
      </p>
    </div>

    <!-- Status Card -->
    <Card class="p-6">
      <h2 class="font-semibold mb-4">Setup Status</h2>

      {#if error}
        <div class="text-destructive text-sm mb-4">{error}</div>
        <Button onclick={checkCliStatus} variant="outline" class="w-full">
          Retry
        </Button>
      {:else}
        <div class="space-y-4">
          <!-- CLI Installation -->
          <div class="flex items-start gap-3">
            {#if checkingInstall}
              <Loader class="w-5 h-5 text-muted-foreground mt-0.5 animate-spin" />
            {:else if installStatus?.installed}
              <CheckCircle class="w-5 h-5 text-green-500 mt-0.5" />
            {:else}
              <XCircle class="w-5 h-5 text-destructive mt-0.5" />
            {/if}
            <div class="flex-1">
              <div class="font-medium">Claude Code CLI</div>
              {#if checkingInstall}
                <div class="text-sm text-muted-foreground">
                  Checking installation...
                </div>
              {:else if installStatus?.installed}
                <div class="text-sm text-muted-foreground">
                  Installed {installStatus.version ? `(${installStatus.version})` : ''}
                </div>
              {:else}
                <div class="text-sm text-muted-foreground">
                  Not installed. Install with:
                </div>
                <code class="text-xs bg-muted px-2 py-1 rounded mt-1 block">
                  npm install -g @anthropic-ai/claude-code
                </code>
              {/if}
            </div>
          </div>

          <!-- Authentication -->
          <div class="flex items-start gap-3">
            {#if checkingInstall || checkingAuth}
              <Loader class="w-5 h-5 text-muted-foreground mt-0.5 animate-spin" />
            {:else if authStatus?.authenticated}
              <CheckCircle class="w-5 h-5 text-green-500 mt-0.5" />
            {:else if installStatus?.installed}
              <XCircle class="w-5 h-5 text-destructive mt-0.5" />
            {:else}
              <div class="w-5 h-5 mt-0.5 rounded-full border-2 border-muted-foreground/30"></div>
            {/if}
            <div class="flex-1">
              <div class="font-medium">Authentication</div>
              {#if checkingInstall}
                <div class="text-sm text-muted-foreground">
                  Waiting for CLI check...
                </div>
              {:else if checkingAuth}
                <div class="text-sm text-muted-foreground">
                  Checking authentication...
                </div>
              {:else if authStatus?.authenticated}
                <div class="text-sm text-muted-foreground">
                  Authenticated and ready
                </div>
              {:else if installStatus?.installed}
                <div class="text-sm text-muted-foreground">
                  Not authenticated. Run:
                </div>
                <code class="text-xs bg-muted px-2 py-1 rounded mt-1 block">
                  claude login
                </code>
              {:else}
                <div class="text-sm text-muted-foreground">
                  Install CLI first
                </div>
              {/if}
            </div>
          </div>
        </div>

        <div class="mt-6 space-y-3">
          {#if canProceed}
            <Button onclick={proceed} class="w-full">
              Get Started
            </Button>
          {:else if !checkingInstall && !checkingAuth}
            <Button onclick={checkCliStatus} variant="outline" class="w-full">
              Check Again
            </Button>
          {/if}
        </div>
      {/if}
    </Card>

    <!-- Help Link -->
    <div class="text-center">
      <a
        href="https://docs.anthropic.com/en/docs/claude-code"
        target="_blank"
        rel="noopener noreferrer"
        class="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1"
      >
        Learn more about Claude Code
        <ExternalLink class="w-3 h-3" />
      </a>
    </div>
  </div>
</div>
