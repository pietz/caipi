<script lang="ts">
  import { api } from '$lib/api';
  import { onMount } from 'svelte';
  import { Loader2 } from 'lucide-svelte';
  import SetupWizard from '$lib/components/onboarding/SetupWizard.svelte';
  import SessionPicker from '$lib/components/folder/SessionPicker.svelte';
  import ChatContainer from '$lib/components/chat/ChatContainer.svelte';
  import { LicenseEntry } from '$lib/components/license';
  import { app } from '$lib/stores/app.svelte';

  onMount(async () => {
    try {
      // First check license status
      const licenseStatus = await api.getLicenseStatus();

      if (!licenseStatus.valid) {
        // No valid license - show license entry screen
        app.setScreen('license');
        app.setLoading(false);
        return;
      }

      // Store license info in app state
      app.setLicense({
        valid: true,
        licenseKey: licenseStatus.licenseKey,
        activatedAt: licenseStatus.activatedAt,
        email: licenseStatus.email,
      });

      const startupInfo = await api.getStartupInfo();

      // Set CLI path if available (for custom Claude CLI location)
      if (startupInfo.cliPath) {
        app.setCliPath(startupInfo.cliPath);
      }

      // If onboarding is completed and we have a default folder, go directly to chat
      if (startupInfo.onboardingCompleted && startupInfo.defaultFolder) {
        // Validate the folder still exists/is accessible
        const valid = await api.validateFolder(startupInfo.defaultFolder);

        if (valid) {
          // Set CLI status if available
          if (startupInfo.cliStatus) {
            app.setCliStatus(startupInfo.cliStatus);
          }

          // Start session directly
          try {
            await app.startSession(startupInfo.defaultFolder);
            app.setLoading(false);
            return;
          } catch (e) {
            console.error('Failed to start session:', e);
            // Fall through to folder picker so user can try again
            app.setScreen('folder');
            app.setLoading(false);
            return;
          }
        }
      }

      // Otherwise show onboarding (for first-time users or if CLI isn't installed)
      app.setScreen('onboarding');
      app.setLoading(false);
    } catch (e) {
      console.error('Failed to get startup info:', e);
      // Fallback to license check on error (most secure default)
      app.setScreen('license');
      app.setLoading(false);
    }
  });
</script>

{#if app.screen === 'loading'}
  <div class="flex items-center justify-center h-full" data-tauri-drag-region>
    <Loader2 size={24} class="animate-spin text-muted-foreground" />
  </div>
{:else if app.screen === 'license'}
  <LicenseEntry />
{:else if app.screen === 'onboarding'}
  <SetupWizard />
{:else if app.screen === 'folder'}
  <SessionPicker showClose={!!app.sessionId} />
{:else if app.screen === 'chat'}
  <ChatContainer />
{/if}
