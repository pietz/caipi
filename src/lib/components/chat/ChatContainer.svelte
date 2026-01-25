<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { PanelLeft, PanelRight, Sun, Moon, Home } from 'lucide-svelte';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import ChatMessage from './ChatMessage.svelte';
  import ActivityCard from './ActivityCard.svelte';
  import MessageInput from './MessageInput.svelte';
  import { HIDDEN_TOOL_TYPES } from './constants';
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
    [...chat.streamItems]
      .filter(item => !(item.type === 'tool' && item.activity && HIDDEN_TOOL_TYPES.includes(item.activity.toolType)))
      .sort((a, b) => a.insertionIndex - b.insertionIndex)
  );
</script>

<div class="flex flex-col h-full relative">
  <!-- Titlebar -->
  <div
    class="h-9 flex items-center justify-between px-4 border-b border-border shrink-0"
    data-tauri-drag-region
  >
    <!-- Left - Window Controls Space + Sidebar Toggle + Home -->
    <div class="flex items-center gap-1">
      <div class="w-[52px]"></div>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.toggleLeftSidebar()}
      >
        <PanelLeft size={14} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={goBack}
      >
        <Home size={14} />
      </Button>
    </div>

    <!-- Center - Project Name -->
    <div class="flex items-center">
      <span class="text-sm font-medium">{app.folderName}</span>
    </div>

    <!-- Right - Controls -->
    <div class="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={toggleTheme}
      >
        {#if currentTheme === 'dark'}
          <Sun size={14} />
        {:else}
          <Moon size={14} />
        {/if}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.toggleRightSidebar()}
      >
        <PanelRight size={14} />
      </Button>
    </div>
  </div>

  <!-- Content area with sidebars -->
  <div class="flex flex-1 min-h-0">
    <!-- Left Sidebar - File Explorer -->
    <div
      class="shrink-0 overflow-hidden transition-all duration-200 border-r border-border bg-muted/50"
      style="width: {app.leftSidebar ? '224px' : '0px'};"
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
        class="flex-1 overflow-y-auto"
      >
        {#if chat.messages.length === 0 && !chat.isStreaming}
          <!-- Empty State -->
          <div class="flex flex-col items-center justify-center h-full text-muted-foreground">
            <div class="mb-3 opacity-50">
              <CaipiIcon size={64} />
            </div>
            <p class="text-sm mb-1">
              Start a conversation
            </p>
            <p class="text-xs text-muted-foreground/70">
              Ask Claude to help with your code
            </p>
          </div>
        {:else}
          <!-- Message List -->
          <div class="max-w-3xl mx-auto px-6 py-4">
            {#each chat.messages as message (message.id)}
              <ChatMessage {message} />
            {/each}

            <!-- Stream Items (during streaming) -->
            {#if chat.isStreaming && sortedStreamItems.length > 0}
              <div>
                {#each sortedStreamItems as item, index (item.insertionIndex)}
                  {#if item.type === 'text' && item.content}
                    <div
                      class="message-content text-sm leading-relaxed text-foreground/90"
                    >
                      {@html item.content ? DOMPurify.sanitize(marked.parse(item.content) as string) : ''}
                    </div>
                  {:else if item.type === 'tool' && item.activity}
                    <div class="mt-2">
                      <ActivityCard
                        activity={item.activity}
                        pendingPermissions={chat.pendingPermissions}
                        onPermissionResponse={(allowed) => handlePermissionResponse(item.activity!.id, allowed)}
                      />
                    </div>
                  {/if}
                {/each}
              </div>
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
      class="shrink-0 overflow-hidden transition-all duration-200 border-l border-border bg-muted/50"
      style="width: {app.rightSidebar ? '224px' : '0px'};"
    >
      <ContextPanel />
    </div>
  </div>
</div>
