<script lang="ts">
  import { api } from '$lib/api';
  import { Loader2, Key, Sun, Moon, AlertCircle, CheckCircle } from 'lucide-svelte';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button, Input } from '$lib/components/ui';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import { app } from '$lib/stores/app.svelte';

  let licenseKey = $state('');
  let validating = $state(false);
  let error = $state<string | null>(null);
  let success = $state(false);

  const currentTheme = $derived($resolvedTheme);

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  async function validateLicense() {
    if (!licenseKey.trim()) {
      error = 'Please enter a license key';
      return;
    }

    validating = true;
    error = null;

    try {
      const result = await api.validateLicense(licenseKey.trim());

      if (result.valid) {
        success = true;
        app.setLicense({
          valid: true,
          licenseKey: licenseKey.trim(),
          activatedAt: Date.now(),
          email: result.email,
        });

        // Brief delay to show success state before proceeding
        setTimeout(() => {
          app.setScreen('onboarding');
        }, 800);
      } else {
        error = result.error || 'Invalid license key';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to validate license';
    } finally {
      validating = false;
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !validating) {
      validateLicense();
    }
  }
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
    <h1 class="text-lg font-semibold mt-4 text-foreground">Activate Caipi</h1>
    <p class="text-xs text-muted-foreground mt-1">Enter your license key to get started</p>
  </div>

  <!-- License Card -->
  <div class="w-[380px] rounded-lg border border-border bg-card p-5">
    <div class="flex items-center gap-2 mb-4">
      <Key size={16} class="text-muted-foreground" />
      <span class="text-sm font-medium text-foreground">License Key</span>
    </div>

    <div class="space-y-4">
      <Input
        type="text"
        placeholder="CAIPI-XXXX-XXXX-XXXX"
        bind:value={licenseKey}
        onkeydown={handleKeyDown}
        disabled={validating || success}
        class="font-mono text-sm"
      />

      {#if error}
        <div class="flex items-center gap-2 text-xs text-red-500">
          <AlertCircle size={14} />
          <span>{error}</span>
        </div>
      {/if}

      {#if success}
        <div class="flex items-center gap-2 text-xs text-green-500">
          <CheckCircle size={14} />
          <span>License activated successfully!</span>
        </div>
      {/if}

      <Button onclick={validateLicense} disabled={validating || success || !licenseKey.trim()} class="w-full">
        {#if validating}
          <span class="flex items-center gap-2">
            <Loader2 size={14} class="animate-spin" />
            Validating...
          </span>
        {:else if success}
          <span class="flex items-center gap-2">
            <CheckCircle size={14} />
            Activated
          </span>
        {:else}
          Activate License
        {/if}
      </Button>
    </div>

    <!-- Divider -->
    <div class="w-full h-px my-5 bg-border"></div>

    <!-- Help text -->
    <div class="text-xs text-muted-foreground space-y-2">
      <p>
        Don't have a license?
        <a
          href="https://caipi.ai"
          target="_blank"
          rel="noopener noreferrer"
          class="text-primary hover:underline"
        >
          Purchase one here
        </a>
      </p>
      <p class="text-muted-foreground/70">
        Your license key was sent to your email after purchase.
      </p>
    </div>
  </div>
</div>
