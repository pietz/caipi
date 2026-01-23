<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import {
    FolderIcon,
    HomeIcon,
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
    type Message,
    type ToolActivity,
    type PermissionRequest,
    type StreamItem,
  } from '$lib/stores';

  interface ChatEvent {
    type: string;
    content?: string;
    activity?: ToolActivity;
    id?: string;
    status?: string;
    tool?: string;
    toolUseId?: string;  // The tool_use_id for matching to activity
    description?: string;
    message?: string;
    authType?: string;
    planContent?: string;
  }

  let messagesContainer = $state<HTMLDivElement | null>(null);
  let unlisten: (() => void) | null = null;

  let sessionId = $state<string | null>(null);
  let folderPath = $state<string | null>(null);
  let folderName = $state<string>('');
  let leftSidebarOpen = $state(false);
  let rightSidebarOpen = $state(false);
  let authType = $state<string | null>(null);
  let currentTheme = $state<'light' | 'dark'>('dark');

  // Subscribe to resolved theme
  resolvedTheme.subscribe((theme) => {
    currentTheme = theme;
  });

  // Subscribe to app store
  appStore.subscribe((state) => {
    sessionId = state.sessionId;
    folderPath = state.selectedFolder;
    leftSidebarOpen = state.leftSidebarOpen;
    rightSidebarOpen = state.rightSidebarOpen;
    authType = state.authType;
    if (folderPath) {
      folderName = folderPath.split('/').pop() || folderPath;
    }
  });

  // Local state derived from chat store
  let messages = $state<Message[]>([]);
  let activities = $state<ToolActivity[]>([]);
  let streamItems = $state<StreamItem[]>([]);
  let isStreaming = $state(false);
  let streamingContent = $state('');
  let pendingPermissions = $state<Record<string, PermissionRequest>>({});

  // Subscribe to chat store
  chatStore.subscribe((state) => {
    messages = state.messages;
    activities = state.activities;
    streamItems = state.streamItems;
    isStreaming = state.isStreaming;
    streamingContent = state.streamingContent;
    pendingPermissions = state.pendingPermissions;
  });

  onMount(async () => {
    // Listen for Claude events
    unlisten = await listen<ChatEvent>('claude:event', (event) => {
      handleClaudeEvent(event.payload);
    });

    // Global keyboard listener for permission shortcuts
    window.addEventListener('keydown', handleGlobalKeydown);
  });

  onDestroy(() => {
    unlisten?.();
    window.removeEventListener('keydown', handleGlobalKeydown);
  });

  function handleGlobalKeydown(e: KeyboardEvent) {
    // Handle permission shortcuts - approve/deny first pending permission in UI order
    const permissionKeys = Object.keys(pendingPermissions);
    if (permissionKeys.length === 0) return;

    if (e.key === 'Enter' || e.key === 'Escape') {
      const activeElement = document.activeElement as HTMLTextAreaElement | null;
      const isTextareaWithContent = activeElement?.tagName === 'TEXTAREA' && activeElement.value.trim().length > 0;

      if (isTextareaWithContent) return;

      e.preventDefault();

      // Find the first pending permission by activity order in streamItems
      const sortedItems = [...streamItems].sort((a, b) => a.insertionIndex - b.insertionIndex);
      const firstPendingActivity = sortedItems.find(
        item => item.type === 'tool' && item.activity && pendingPermissions[item.activity.id]
      );

      if (firstPendingActivity?.activity) {
        const permission = pendingPermissions[firstPendingActivity.activity.id];
        if (permission) {
          if (e.key === 'Enter') {
            handlePermissionResponseForRequest(permission, true);  // Allow
          } else if (e.key === 'Escape') {
            handlePermissionResponseForRequest(permission, false); // Deny
          }
        }
      }
    }
  }

  function handleClaudeEvent(event: ChatEvent) {
    switch (event.type) {
      case 'Text':
        if (event.content) {
          chatStore.appendStreamingContent(event.content);
        }
        break;

      case 'ToolStart':
        if (event.activity) {
          const newActivity = {
            ...event.activity,
            toolType: event.activity.toolType,
            status: 'running' as const,
          };
          chatStore.addActivity(newActivity);

          // If there's a pending permission request with matching tool type but no activityId,
          // link it to this activity (handles case where permission request arrives before ToolStart)
          // Find by request id (since we don't have activityId yet)
          for (const [key, permission] of Object.entries(pendingPermissions)) {
            if (permission.activityId === null && permission.tool === newActivity.toolType) {
              // Remove the old entry and add with the new activityId
              chatStore.removePermissionRequest(key);
              chatStore.addPermissionRequest({
                ...permission,
                activityId: newActivity.id,
              });
              break; // Only update one permission per ToolStart
            }
          }
        }
        break;

      case 'ToolEnd':
        if (event.id && event.status) {
          chatStore.updateActivityStatus(
            event.id,
            event.status as ToolActivity['status']
          );
        }
        break;

      case 'PermissionRequest':
        if (event.id && event.tool && event.description) {
          // Use toolUseId for exact matching when available (handles parallel tools)
          // Fall back to finding by tool type for backwards compatibility
          let matchingActivityId: string | null = null;

          if (event.toolUseId) {
            // Exact match by tool_use_id
            const exactMatch = activities.find((a) => a.id === event.toolUseId);
            matchingActivityId = exactMatch?.id || null;
          }

          if (!matchingActivityId) {
            // Fallback: find by tool type that doesn't already have a pending permission
            const matchingActivity = activities.find(
              (a) => a.status === 'running' && a.toolType === event.tool && !pendingPermissions[a.id]
            );
            matchingActivityId = matchingActivity?.id || null;
          }

          chatStore.addPermissionRequest({
            id: event.id,
            activityId: matchingActivityId,
            tool: event.tool,
            description: event.description,
            timestamp: Date.now() / 1000,
          });
        }
        break;

      case 'Complete':
        // Convert streamItems to a message with embedded activities
        chatStore.finalizeStream();
        // Process any queued messages
        processQueuedMessages();
        break;

      case 'AbortComplete':
        // Backend has confirmed abort - finalize frontend state
        chatStore.finalizeStream();
        chatStore.setStreaming(false);
        chatStore.clearMessageQueue();
        chatStore.clearPermissionRequests();
        break;

      case 'SessionInit':
        if (event.authType) {
          appStore.setAuthType(event.authType);
        }
        break;

      case 'Error':
        console.error('Claude error:', event.message);
        // Add error as a visible message in the chat
        chatStore.addMessage({
          id: crypto.randomUUID(),
          role: 'error',
          content: event.message || 'An unknown error occurred',
          timestamp: Date.now() / 1000,
        });
        chatStore.setStreaming(false);
        break;
    }

    scrollToBottom();
  }

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

  async function handlePermissionResponseForRequest(permission: PermissionRequest, allowed: boolean) {
    if (!sessionId) return;

    try {
      await invoke('respond_permission', {
        sessionId,
        requestId: permission.id,
        allowed,
      });
    } catch (e) {
      console.error('Failed to respond to permission:', e);
    }

    // Remove this specific permission request
    const key = permission.activityId || permission.id;
    chatStore.removePermissionRequest(key);
  }

  // Convenience wrapper for ActivityCard callbacks
  function handlePermissionResponse(activityId: string, allowed: boolean) {
    const permission = pendingPermissions[activityId];
    if (permission) {
      handlePermissionResponseForRequest(permission, allowed);
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

      <!-- Home button -->
      <button
        type="button"
        onclick={goBack}
        class="p-1 rounded transition-all duration-100 text-muted hover:bg-hover hover:text-secondary"
        title="Back to projects"
      >
        <HomeIcon size={16} />
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
        class="flex-1 overflow-y-auto p-4"
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
