<script lang="ts">
  import { api } from '$lib/api';
  import { Loader2, Key, Sun, Moon, AlertCircle, CheckCircle } from 'lucide-svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button, Input } from '$lib/components/ui';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import { app } from '$lib/stores/app.svelte';

  let licenseKey = $state('');
  let validating = $state(false);
  let error = $state<string | null>(null);
  let errorHint = $state<string | null>(null);
  let success = $state(false);

  const currentTheme = $derived($resolvedTheme);

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  /**
   * Converts API error messages to user-friendly messages with optional hints
   */
  function getErrorMessage(apiError: string, key: string): { message: string; hint: string | null } {
    const trimmedKey = key.trim();

    // Check if this might be a coupon code (8 characters)
    if (trimmedKey.length === 8) {
      return {
        message: 'This doesn\'t look like a valid license key',
        hint: 'Could this be a coupon code? Coupon codes should be redeemed at caipi.ai during checkout, not here.',
      };
    }

    // Handle common Lemon Squeezy errors
    const lowerError = apiError.toLowerCase();

    if (lowerError.includes('not found') || lowerError.includes('does not exist')) {
      return {
        message: 'License key not found',
        hint: 'Please check for typos. Your license key was sent to your email after purchase.',
      };
    }

    if (lowerError.includes('disabled') || lowerError.includes('expired')) {
      return {
        message: 'This license has been disabled or expired',
        hint: 'Please contact support if you believe this is an error.',
      };
    }

    if (lowerError.includes('limit') || lowerError.includes('activation')) {
      return {
        message: 'Activation limit reached',
        hint: 'This license has been activated on too many devices. Deactivate it on another device or contact support.',
      };
    }

    if (lowerError.includes('connect') || lowerError.includes('network') || lowerError.includes('timeout')) {
      return {
        message: 'Could not connect to the license server',
        hint: 'Please check your internet connection and try again.',
      };
    }

    // Default fallback
    return {
      message: apiError,
      hint: null,
    };
  }

  async function validateLicense() {
    if (!licenseKey.trim()) {
      error = 'Please enter a license key';
      errorHint = null;
      return;
    }

    validating = true;
    error = null;
    errorHint = null;

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
        const { message, hint } = getErrorMessage(result.error || 'Invalid license key', licenseKey);
        error = message;
        errorHint = hint;
      }
    } catch (e) {
      const rawError = e instanceof Error ? e.message : 'Failed to validate license';
      const { message, hint } = getErrorMessage(rawError, licenseKey);
      error = message;
      errorHint = hint;
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
        placeholder="Enter your license key"
        bind:value={licenseKey}
        onkeydown={handleKeyDown}
        disabled={validating || success}
        class="font-mono text-sm"
      />

      {#if error}
        <div class="flex flex-col gap-1">
          <div class="flex items-center gap-2 text-xs text-red-500">
            <AlertCircle size={14} class="flex-shrink-0" />
            <span>{error}</span>
          </div>
          {#if errorHint}
            <p class="text-xs text-muted-foreground ml-[22px]">{errorHint}</p>
          {/if}
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
        <button
          onclick={() => openUrl('https://caipi.ai')}
          class="text-primary underline underline-offset-2 hover:opacity-80"
        >
          Purchase one here
        </button>
      </p>
      <p class="text-muted-foreground/70">
        Your license key was sent to your email after purchase.
      </p>
    </div>
  </div>
</div>
