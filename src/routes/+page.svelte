<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import SetupWizard from '$lib/components/onboarding/SetupWizard.svelte';
  import FolderPicker from '$lib/components/folder/FolderPicker.svelte';
  import ChatContainer from '$lib/components/chat/ChatContainer.svelte';
  import SpinnerIcon from '$lib/components/icons/SpinnerIcon.svelte';
  import { appStore, currentScreen } from '$lib/stores';

  interface StartupInfo {
    onboarding_completed: boolean;
    cli_status: {
      installed: boolean;
      version: string | null;
      authenticated: boolean;
      path: string | null;
    } | null;
    cli_status_fresh: boolean;
    default_folder: string | null;
  }

  let screen = $state<string>('loading');

  // Subscribe to screen changes
  currentScreen.subscribe((value) => {
    screen = value;
  });

  onMount(async () => {
    try {
      const startupInfo = await invoke<StartupInfo>('get_startup_info');

      // If onboarding is completed and we have a default folder, go directly to chat
      if (startupInfo.onboarding_completed && startupInfo.default_folder) {
        // Validate the folder still exists/is accessible
        const valid = await invoke<boolean>('validate_folder', { path: startupInfo.default_folder });

        if (valid) {
          // Set CLI status if available
          if (startupInfo.cli_status) {
            appStore.setCliStatus(startupInfo.cli_status);
          }

          // Set folder and create session
          appStore.setSelectedFolder(startupInfo.default_folder);

          // Get current settings
          const { permissionMode, model } = get(appStore);

          // Create session
          const sessionId = await invoke<string>('create_session', {
            folderPath: startupInfo.default_folder,
            permissionMode,
            model,
          });
          appStore.setSessionId(sessionId);

          // Go directly to chat
          appStore.setScreen('chat');
          appStore.setLoading(false);
          return;
        }
      }

      // Otherwise show onboarding (for first-time users or if CLI isn't installed)
      appStore.setScreen('onboarding');
      appStore.setLoading(false);
    } catch (e) {
      console.error('Failed to get startup info:', e);
      // Fallback to onboarding on error
      appStore.setScreen('onboarding');
      appStore.setLoading(false);
    }
  });
</script>

{#if screen === 'loading'}
  <div class="flex items-center justify-center h-full" data-tauri-drag-region>
    <SpinnerIcon size={24} />
  </div>
{:else if screen === 'onboarding'}
  <SetupWizard />
{:else if screen === 'folder'}
  <FolderPicker showClose={true} />
{:else if screen === 'chat'}
  <ChatContainer />
{/if}
