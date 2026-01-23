<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import {
    FolderIcon,
    MenuIcon,
    SidebarLeftIcon,
    SidebarRightIcon,
    CaipiIcon,
    SunIcon,
    MoonIcon,
  } from '$lib/components/icons';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import ChatMessage from './ChatMessage.svelte';
  import ActivityCard from './ActivityCard.svelte';
  import MessageInput from './MessageInput.svelte';
  import { FileExplorer, ContextPanel } from '$lib/components/sidebar';
  import {
    appStore,
    chatStore,
    createStreamCoordinator,
    createPermissionCoordinator,
    type ChatEvent,
  } from '$lib/stores';

  let messagesContainer = $state<HTMLDivElement | null>(null);
  let unlisten: (() => void) | null = null;
  let cleanupKeyboardShortcuts: (() => void) | null = null;

  // Theme
  const currentTheme = $derived($resolvedTheme);

  // App store values
  const sessionId = $derived($appStore.sessionId);
  const folderPath = $derived($appStore.selectedFolder);
  const leftSidebarOpen = $derived($appStore.leftSidebarOpen);
  const rightSidebarOpen = $derived($appStore.rightSidebarOpen);
  const authType = $derived($appStore.authType);
  const folderName = $derived(folderPath ? folderPath.split('/').pop() || folderPath : '');

  // Chat store values
  const messages = $derived($chatStore.messages);
  const streamItems = $derived($chatStore.streamItems);
  const isStreaming = $derived($chatStore.isStreaming);
  const pendingPermissions = $derived($chatStore.pendingPermissions);

  // Stream coordinator handles all Claude events
  const streamCoordinator = createStreamCoordinator({
    onComplete: processQueuedMessages,
  });

  // Permission coordinator handles permission interactions
  const permissionCoordinator = createPermissionCoordinator({
    getSessionId: () => sessionId,
    getStreamItems: () => chatStore.getStreamItems(),
    getPendingPermissions: () => chatStore.getPendingPermissions(),
  });

  onMount(async () => {
    // Listen for Claude events and delegate to stream coordinator
    unlisten = await listen<ChatEvent>('claude:event', (event) => {
      streamCoordinator.handleEvent(event.payload);
      scrollToBottom();
    });

    // Set up keyboard shortcuts for permission handling
    cleanupKeyboardShortcuts = permissionCoordinator.setupKeyboardShortcuts();
  });

  onDestroy(() => {
    unlisten?.();
    cleanupKeyboardShortcuts?.();
  });

  async function sendMessage(message: string) {
    if (!sessionId) return;

    // Add user message
    chatStore.addMessage({
      id: crypto.randomUUID(),
      role: 'user',
      content: message,
      timestamp: Date.now() / 1000,
    });

    // Start streaming
    chatStore.setStreaming(true);

    // Scroll to bottom
    scrollToBottom();

    try {
      await invoke('send_message', {
        sessionId,
        message,
      });
    } catch (e) {
      console.error('Failed to send message:', e);
      chatStore.setStreaming(false);
    }
  }

  function queueMessage(message: string) {
    // Add to queue
    chatStore.enqueueMessage(message);

    // Show in UI immediately as user message
    chatStore.addMessage({
      id: crypto.randomUUID(),
      role: 'user',
      content: message,
      timestamp: Date.now() / 1000,
    });

    scrollToBottom();
  }

  async function processQueuedMessages() {
    const nextMessage = chatStore.dequeueMessage();
    if (!nextMessage || !sessionId) return;

    // Keep streaming state active
    chatStore.setStreaming(true);

    try {
      await invoke('send_message', {
        sessionId,
        message: nextMessage,
      });
    } catch (e) {
      console.error('Failed to send queued message:', e);
      chatStore.setStreaming(false);
    }
  }

  function scrollToBottom() {
    setTimeout(() => {
      if (messagesContainer) {
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
      }
    }, 50);
  }

  function goBack() {
    chatStore.reset();
    appStore.setScreen('folder');
  }

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  async function abortSession() {
    if (!sessionId) return;

    // Clear queue and permissions immediately - user wants to stop
    // This ensures they're cleared even if Complete arrives before AbortComplete
    chatStore.clearMessageQueue();
    chatStore.clearPermissionRequests();

    try {
      await invoke('abort_session', { sessionId });
      // AbortComplete event from backend will handle stream finalization.
    } catch (e) {
      console.error('Failed to abort session:', e);
      // Fallback: finalize locally if the command failed
      chatStore.finalizeStream();
      chatStore.setStreaming(false);
    }
  }
</script>

<div class="flex flex-col h-full relative">
  <!-- Header - Full width at top -->
  <div
    class="py-2 px-3 flex items-center justify-between shrink-0"
    style="border-bottom: 1px solid var(--border); background-color: var(--header-bg);"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2 pl-[70px]">
      <!-- Left sidebar toggle -->
      <button
        type="button"
        onclick={() => appStore.toggleLeftSidebar()}
        class="p-1 rounded transition-all duration-100"
        style="
          background-color: {leftSidebarOpen ? 'var(--hover)' : 'transparent'};
          color: {leftSidebarOpen ? 'var(--text-secondary)' : 'var(--text-dim)'};
        "
        title="Toggle file explorer"
      >
        <SidebarLeftIcon size={16} />
      </button>

      <!-- Open project button -->
      <button
        type="button"
        onclick={goBack}
        class="p-1 rounded transition-all duration-100 text-muted hover:bg-hover hover:text-secondary"
        title="Open project"
      >
        <MenuIcon size={16} />
      </button>

      <!-- Separator -->
      <div
        class="w-px h-4 mx-1"
        style="background-color: var(--border-hover);"
      ></div>

      <!-- Project info -->
      <span class="text-folder flex items-center">
        <FolderIcon size={14} />
      </span>
      <span class="text-sm font-medium text-primary">{folderName}</span>
    </div>

    <div class="flex items-center gap-2">
      <!-- Auth type indicator -->
      {#if authType}
        <span class="text-xs text-dim px-2 py-0.5 rounded" style="background-color: var(--card);">
          {authType}
        </span>
      {/if}

      <!-- Theme toggle -->
      <button
        type="button"
        onclick={toggleTheme}
        class="p-1 rounded transition-all duration-100 text-dim hover:bg-hover hover:text-secondary"
        title={currentTheme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
      >
        {#if currentTheme === 'dark'}
          <SunIcon size={16} />
        {:else}
          <MoonIcon size={16} />
        {/if}
      </button>

      <!-- Right sidebar toggle -->
      <button
        type="button"
        onclick={() => appStore.toggleRightSidebar()}
        class="p-1 rounded transition-all duration-100"
        style="
          background-color: {rightSidebarOpen ? 'var(--hover)' : 'transparent'};
          color: {rightSidebarOpen ? 'var(--text-secondary)' : 'var(--text-dim)'};
        "
        title="Toggle context panel"
      >
        <SidebarRightIcon size={16} />
      </button>
    </div>
  </div>

  <!-- Content area with sidebars -->
  <div class="flex flex-1 min-h-0">
    <!-- Left Sidebar - File Explorer -->
    <div
      class="shrink-0 overflow-hidden transition-[width] duration-200 ease-out"
      style="
        width: {leftSidebarOpen ? '200px' : '0px'};
        border-right: {leftSidebarOpen ? '1px solid var(--border)' : 'none'};
      "
    >
      {#if folderPath}
        <FileExplorer rootPath={folderPath} />
      {/if}
    </div>

    <!-- Main Chat Area -->
    <div class="flex-1 flex flex-col min-w-0">
      <!-- Messages -->
      <div
        bind:this={messagesContainer}
        class="flex-1 overflow-y-auto p-6"
      >
        {#if messages.length === 0 && !isStreaming}
          <!-- Empty State -->
          <div class="flex flex-col items-center justify-center h-full text-dim">
            <div class="mb-3 opacity-50">
              <CaipiIcon size={64} />
            </div>
            <p class="text-sm mb-1 text-muted">
              Start a conversation
            </p>
            <p class="text-xs">
              Ask Claude to help with your code
            </p>
          </div>
        {:else}
          <!-- Message List -->
          <div class="flex flex-col">
            {#each messages as message, index (message.id)}
              <ChatMessage {message} showDivider={index > 0} />
            {/each}

            <!-- Stream Items (during streaming) - sorted by insertionIndex for stable ordering -->
            {#if isStreaming && streamItems.length > 0}
              {@const sortedItems = [...streamItems].sort((a, b) => a.insertionIndex - b.insertionIndex)}
              {#each sortedItems as item, index (item.insertionIndex)}
                {#if item.type === 'text' && item.content}
                  <ChatMessage
                    message={{
                      id: item.id,
                      role: 'assistant',
                      content: item.content,
                      timestamp: item.timestamp,
                    }}
                    streaming={index === sortedItems.length - 1 && item.type === 'text'}
                    showDivider={messages.length > 0 || index > 0}
                  />
                {:else if item.type === 'tool' && item.activity}
                  <div class="mt-1">
                    <ActivityCard
                      activity={item.activity}
                      {pendingPermissions}
                      onPermissionResponse={(allowed) => permissionCoordinator.respondToPermissionByActivityId(item.activity!.id, allowed)}
                    />
                  </div>
                {/if}
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      <!-- Input -->
      <MessageInput
        onSend={sendMessage}
        onQueue={queueMessage}
        onAbort={abortSession}
        {isStreaming}
      />
    </div>

    <!-- Right Sidebar - Context Panel -->
    <div
      class="shrink-0 overflow-hidden transition-[width] duration-200 ease-out"
      style="
        width: {rightSidebarOpen ? '220px' : '0px'};
        border-left: {rightSidebarOpen ? '1px solid var(--border)' : 'none'};
      "
    >
      <ContextPanel />
    </div>
  </div>

</div>
