<script lang="ts">
  import { api } from '$lib/api';
  import { onMount } from 'svelte';
  import { Loader2 } from 'lucide-svelte';
  import SetupWizard from '$lib/components/onboarding/SetupWizard.svelte';
  import SessionPicker from '$lib/components/folder/SessionPicker.svelte';
  import ChatContainer from '$lib/components/chat/ChatContainer.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { initLogger, info, error as logError } from '$lib/utils/logger';

  onMount(async () => {
    await initLogger();

    try {
      const startupInfo = await api.getStartupInfo();

      // Set CLI path if available (for custom Claude CLI location)
      if (startupInfo.cliPath) {
        app.setCliPath(startupInfo.cliPath, 'claude');
      }
      if (startupInfo.backendCliPaths) {
        for (const [backend, path] of Object.entries(startupInfo.backendCliPaths)) {
          const normalized = backend === 'claudecli' ? 'claude' : backend;
          if (normalized === 'claude' || normalized === 'codex') {
            app.setCliPath(path, normalized);
          }
        }
      }
      if (startupInfo.defaultBackend) {
        const normalized = startupInfo.defaultBackend === 'claudecli' ? 'claude' : startupInfo.defaultBackend;
        if (normalized === 'claude' || normalized === 'codex') {
          app.defaultBackend = normalized;
        }
        app.ensureBackendState(app.defaultBackend);
      }

      // Warm recent sessions for both backends in the background so opening
      // the session picker is responsive.
      app.prewarmRecentSessions();

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
            info(`Boot complete: auto-started session, screen=${app.screen}`);
            app.setLoading(false);
            return;
          } catch (e) {
            logError(`Failed to start session: ${e}`);
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
      logError(`Failed to get startup info: ${e}`);
      app.setScreen('onboarding');
      app.setLoading(false);
    }
  });
</script>

{#if app.screen === 'loading'}
  <div class="flex items-center justify-center h-full" data-tauri-drag-region>
    <Loader2 size={24} class="animate-spin text-muted-foreground" />
  </div>
{:else if app.screen === 'onboarding'}
  <SetupWizard />
{:else if app.screen === 'folder'}
  <SessionPicker showClose={!!app.sessionId} />
{:else if app.screen === 'chat'}
  <ChatContainer />
{/if}
