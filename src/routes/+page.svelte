<script lang="ts">
  import Welcome from '$lib/components/onboarding/Welcome.svelte';
  import FolderPicker from '$lib/components/folder/FolderPicker.svelte';
  import ChatContainer from '$lib/components/chat/ChatContainer.svelte';
  import { appStore, currentScreen } from '$lib/stores';

  let screen = $state<string>('onboarding');

  // Subscribe to screen changes
  currentScreen.subscribe((value) => {
    screen = value;
  });
</script>

{#if screen === 'onboarding'}
  <Welcome />
{:else if screen === 'folder'}
  <FolderPicker />
{:else if screen === 'chat'}
  <ChatContainer />
{/if}
