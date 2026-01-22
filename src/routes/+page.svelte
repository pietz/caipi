<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import Welcome from '$lib/components/onboarding/Welcome.svelte';
  import FolderPicker from '$lib/components/folder/FolderPicker.svelte';
  import ChatContainer from '$lib/components/chat/ChatContainer.svelte';
  import SpinnerIcon from '$lib/components/icons/SpinnerIcon.svelte';
  import { appStore, currentScreen } from '$lib/stores';

  let screen = $state<string>('loading');

  // Subscribe to screen changes
  currentScreen.subscribe((value) => {
    screen = value;
  });

  onMount(async () => {
    // Show folder picker immediately for smooth UX
    appStore.setScreen('folder');
    appStore.setLoading(false);

    // Check CLI in background - redirect to onboarding only if not installed
    try {
      const installStatus = await invoke<{ installed: boolean; version: string | null; path: string | null }>('check_cli_installed');

      if (installStatus.installed) {
        appStore.setCliStatus({
          installed: true,
          version: installStatus.version,
          authenticated: true, // We'll handle auth errors at chat time
          path: installStatus.path,
        });
      } else {
        // CLI not installed - redirect to welcome page
        appStore.setScreen('onboarding');
      }
    } catch (e) {
      console.error('Failed to check CLI status:', e);
      appStore.setScreen('onboarding');
    }
  });
</script>

{#if screen === 'loading'}
  <div class="flex items-center justify-center h-full" data-tauri-drag-region>
    <SpinnerIcon size={24} />
  </div>
{:else if screen === 'onboarding'}
  <Welcome />
{:else if screen === 'folder'}
  <FolderPicker />
{:else if screen === 'chat'}
  <ChatContainer />
{/if}
