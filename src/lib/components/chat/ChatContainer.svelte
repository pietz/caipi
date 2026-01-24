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
  import { app } from '$lib/stores/app.svelte';
  import { chat, type StreamItem } from '$lib/stores/chat.svelte';
  import { handleClaudeEvent, respondToPermission, resetEventState, type ChatEvent } from '$lib/utils/events';

  let messagesContainer = $state<HTMLDivElement | null>(null);
  let unlisten: (() => void) | null = null;
  let cleanupKeyboardShortcuts: (() => void) | null = null;

  // Theme
  const currentTheme = $derived($resolvedTheme);

  onMount(async () => {
    // Listen for Claude events
    unlisten = await listen<ChatEvent>('claude:event', (event) => {
      handleClaudeEvent(event.payload, { onComplete: processQueuedMessages });
      scrollToBottom();
    });

    // Set up keyboard shortcuts for permission handling
    cleanupKeyboardShortcuts = setupKeyboardShortcuts();
  });

  onDestroy(() => {
    unlisten?.();
    cleanupKeyboardShortcuts?.();
    resetEventState();
  });

  function setupKeyboardShortcuts(): () => void {
    function handleKeydown(e: KeyboardEvent) {
      const permissionKeys = Object.keys(chat.pendingPermissions);
      if (permissionKeys.length === 0) return;

      if (e.key === 'Enter' || e.key === 'Escape') {
        const activeElement = document.activeElement as HTMLTextAreaElement | null;
        const isTextareaWithContent =
          activeElement?.tagName === 'TEXTAREA' && activeElement.value.trim().length > 0;

        if (isTextareaWithContent) return;

        e.preventDefault();

        // Find the first pending permission by activity order in streamItems
        const sortedItems = [...chat.streamItems].sort((a, b) => a.insertionIndex - b.insertionIndex);
        const firstPendingActivity = sortedItems.find(
          (item) => item.type === 'tool' && item.activity && chat.pendingPermissions[item.activity.id]
        );

        if (firstPendingActivity?.activity && app.sessionId) {
          const permission = chat.pendingPermissions[firstPendingActivity.activity.id];
          if (permission) {
            const allowed = e.key === 'Enter';
            respondToPermission(app.sessionId, permission, allowed);
          }
        }
      }
    }

    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  }

  async function sendMessage(message: string) {
    if (!app.sessionId) return;

    // Add user message
    chat.addUserMessage(message);

    // Start streaming
    chat.setStreaming(true);

    // Scroll to bottom
    scrollToBottom();

    try {
      await invoke('send_message', {
        sessionId: app.sessionId,
        message,
      });
    } catch (e) {
      console.error('Failed to send message:', e);
      chat.setStreaming(false);
    }
  }

  function queueMessage(message: string) {
    // Add to queue
    chat.enqueueMessage(message);

    // Show in UI immediately as user message
    chat.addUserMessage(message);

    scrollToBottom();
  }

  async function processQueuedMessages() {
    const nextMessage = chat.dequeueMessage();
    if (!nextMessage || !app.sessionId) return;

    // Keep streaming state active
    chat.setStreaming(true);

    try {
      await invoke('send_message', {
        sessionId: app.sessionId,
        message: nextMessage,
      });
    } catch (e) {
      console.error('Failed to send queued message:', e);
      chat.setStreaming(false);
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
    resetEventState();
    chat.reset();
    app.setScreen('folder');
  }

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  async function abortSession() {
    if (!app.sessionId) return;

    // Clear queue and permissions immediately
    chat.clearMessageQueue();
    chat.clearPermissionRequests();

    try {
      await invoke('abort_session', { sessionId: app.sessionId });
    } catch (e) {
      console.error('Failed to abort session:', e);
      chat.finalize();
      chat.setStreaming(false);
    }
  }

  function handlePermissionResponse(activityId: string, allowed: boolean) {
    if (!app.sessionId) return;
    const permission = chat.pendingPermissions[activityId];
    if (permission) {
      respondToPermission(app.sessionId, permission, allowed);
    }
  }

  // Derived values for template
  const sortedStreamItems = $derived(
    [...chat.streamItems].sort((a, b) => a.insertionIndex - b.insertionIndex)
  );
</script>

<div class="flex flex-col h-full relative">
  <!-- Header - Full width at top -->
  <div
    class="py-2 px-3 flex items-center justify-between shrink-0 border-b border-border bg-header"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2 pl-[70px]">
      <!-- Left sidebar toggle -->
      <button
        type="button"
        onclick={() => app.toggleLeftSidebar()}
        class="p-1 rounded transition-all duration-100"
        style="
          background-color: {app.leftSidebar ? 'var(--hover)' : 'transparent'};
          color: {app.leftSidebar ? 'var(--text-secondary)' : 'var(--text-dim)'};
        "
        title="Toggle file explorer"
      >
        <SidebarLeftIcon size={16} />
      </button>

      <!-- Open project button -->
      <button
        type="button"
        onclick={goBack}
        class="p-1 rounded transition-all duration-100 text-muted-foreground hover:bg-hover hover:text-foreground"
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
      <span class="text-sm font-medium text-foreground">{app.folderName}</span>
    </div>

    <div class="flex items-center gap-2">
      <!-- Auth type indicator -->
      {#if app.authType}
        <span class="text-xs text-dim px-2 py-0.5 rounded bg-card">
          {app.authType}
        </span>
      {/if}

      <!-- Theme toggle -->
      <button
        type="button"
        onclick={toggleTheme}
        class="p-1 rounded transition-all duration-100 text-dim hover:bg-hover hover:text-foreground"
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
        onclick={() => app.toggleRightSidebar()}
        class="p-1 rounded transition-all duration-100"
        style="
          background-color: {app.rightSidebar ? 'var(--hover)' : 'transparent'};
          color: {app.rightSidebar ? 'var(--text-secondary)' : 'var(--text-dim)'};
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
        width: {app.leftSidebar ? '200px' : '0px'};
        border-right: {app.leftSidebar ? '1px solid hsl(var(--border))' : 'none'};
      "
    >
      {#if app.folder}
        <FileExplorer rootPath={app.folder} />
      {/if}
    </div>

    <!-- Main Chat Area -->
    <div class="flex-1 flex flex-col min-w-0">
      <!-- Messages -->
      <div
        bind:this={messagesContainer}
        class="flex-1 overflow-y-auto p-6"
      >
        {#if chat.messages.length === 0 && !chat.isStreaming}
          <!-- Empty State -->
          <div class="flex flex-col items-center justify-center h-full text-dim">
            <div class="mb-3 opacity-50">
              <CaipiIcon size={64} />
            </div>
            <p class="text-sm mb-1 text-muted-foreground">
              Start a conversation
            </p>
            <p class="text-xs">
              Ask Claude to help with your code
            </p>
          </div>
        {:else}
          <!-- Message List -->
          <div class="flex flex-col">
            {#each chat.messages as message, index (message.id)}
              <ChatMessage {message} showDivider={index > 0} />
            {/each}

            <!-- Stream Items (during streaming) -->
            {#if chat.isStreaming && sortedStreamItems.length > 0}
              {#each sortedStreamItems as item, index (item.insertionIndex)}
                {#if item.type === 'text' && item.content}
                  <ChatMessage
                    message={{
                      id: item.id,
                      role: 'assistant',
                      content: item.content,
                      timestamp: item.timestamp,
                    }}
                    streaming={index === sortedStreamItems.length - 1 && item.type === 'text'}
                    showDivider={chat.messages.length > 0 || index > 0}
                  />
                {:else if item.type === 'tool' && item.activity}
                  <div class="mt-1">
                    <ActivityCard
                      activity={item.activity}
                      pendingPermissions={chat.pendingPermissions}
                      onPermissionResponse={(allowed) => handlePermissionResponse(item.activity!.id, allowed)}
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
        isStreaming={chat.isStreaming}
      />
    </div>

    <!-- Right Sidebar - Context Panel -->
    <div
      class="shrink-0 overflow-hidden transition-[width] duration-200 ease-out"
      style="
        width: {app.rightSidebar ? '220px' : '0px'};
        border-left: {app.rightSidebar ? '1px solid hsl(var(--border))' : 'none'};
      "
    >
      <ContextPanel />
    </div>
  </div>

</div>
