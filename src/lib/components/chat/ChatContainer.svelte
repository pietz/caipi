<script lang="ts">
  import { api } from '$lib/api';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { PanelLeft, PanelRight, Sun, Moon, Menu } from 'lucide-svelte';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import ChatMessage from './ChatMessage.svelte';
  import ToolCallStack from './ToolCallStack.svelte';
  import MessageInput from './MessageInput.svelte';
  import { HIDDEN_TOOL_TYPES } from './constants';
  import { FileExplorer, ContextPanel } from '$lib/components/sidebar';
  import { app } from '$lib/stores/app.svelte';
  import { chat, type StreamItem, type ToolState } from '$lib/stores/chat.svelte';
  import { handleClaudeEvent, respondToPermission, resetEventState, type ChatEvent } from '$lib/utils/events';

  // Types for grouped stream items
  type GroupedTextItem = { type: 'text'; content: string };
  type GroupedToolItem = { type: 'tool-group'; tools: ToolState[] };
  type GroupedItem = GroupedTextItem | GroupedToolItem;

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

  // Scroll to bottom when history is loaded (messages jump from 0 to many)
  let prevMessageCount = $state(0);
  $effect(() => {
    const count = chat.messages.length;
    // History loaded: went from 0 to multiple messages at once
    if (prevMessageCount === 0 && count > 1) {
      scrollToBottom();
    }
    prevMessageCount = count;
  });

  onDestroy(() => {
    unlisten?.();
    cleanupKeyboardShortcuts?.();
    resetEventState();
  });

  function setupKeyboardShortcuts(): () => void {
    function handleKeydown(e: KeyboardEvent) {
      const toolsAwaitingPermission = chat.getToolsAwaitingPermission();
      if (toolsAwaitingPermission.length === 0) return;

      if (e.key === 'Enter' || e.key === 'Escape') {
        const activeElement = document.activeElement as HTMLTextAreaElement | null;
        const isTextareaWithContent =
          activeElement?.tagName === 'TEXTAREA' && activeElement.value.trim().length > 0;

        if (isTextareaWithContent) return;

        e.preventDefault();

        // Get the first tool awaiting permission (already sorted by insertion order)
        const firstTool = toolsAwaitingPermission[0];
        if (firstTool && app.sessionId) {
          const allowed = e.key === 'Enter';
          respondToPermission(app.sessionId, firstTool, allowed);
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
      await api.sendMessage(app.sessionId, message);
    } catch (e) {
      console.error('Failed to send message:', e);
      chat.setStreaming(false);
    }
  }

  function queueMessage(message: string) {
    // Add to queue - message will be added to UI when processed
    chat.enqueueMessage(message);
    scrollToBottom();
  }

  async function processQueuedMessages() {
    const nextMessage = chat.dequeueMessage();
    if (!nextMessage || !app.sessionId) return;

    // Add user message now, right before sending
    chat.addUserMessage(nextMessage);

    // Keep streaming state active
    chat.setStreaming(true);

    scrollToBottom();

    try {
      await api.sendMessage(app.sessionId, nextMessage);
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
    chat.clearPendingPermissions();

    try {
      await api.abortSession(app.sessionId);
    } catch (e) {
      console.error('Failed to abort session:', e);
      chat.finalize();
      chat.setStreaming(false);
    }
  }

  function handlePermissionResponse(toolId: string, allowed: boolean) {
    if (!app.sessionId) return;
    const tool = chat.getTool(toolId);
    if (tool) {
      respondToPermission(app.sessionId, tool, allowed);
    }
  }

  // Derived values for template
  const sortedStreamItems = $derived(
    [...chat.streamItems]
      .filter(item => {
        if (item.type === 'tool' && item.toolId) {
          const tool = chat.getTool(item.toolId);
          return tool && !HIDDEN_TOOL_TYPES.includes(tool.toolType);
        }
        return true;
      })
      .sort((a, b) => a.insertionIndex - b.insertionIndex)
  );

  // Group consecutive tools together
  const groupedStreamItems = $derived((): GroupedItem[] => {
    const groups: GroupedItem[] = [];
    let currentToolGroup: ToolState[] = [];

    for (const item of sortedStreamItems) {
      if (item.type === 'tool' && item.toolId) {
        const tool = chat.getTool(item.toolId);
        if (tool) {
          currentToolGroup.push(tool);
        }
      } else if (item.type === 'text' && item.content) {
        // Text breaks tool groups
        if (currentToolGroup.length > 0) {
          groups.push({ type: 'tool-group', tools: [...currentToolGroup] });
          currentToolGroup = [];
        }
        groups.push({ type: 'text', content: item.content });
      }
    }

    // Don't forget remaining tools
    if (currentToolGroup.length > 0) {
      groups.push({ type: 'tool-group', tools: currentToolGroup });
    }

    return groups;
  });
</script>

<div class="flex flex-col h-full relative">
  <!-- Titlebar -->
  <div
    class="h-9 flex items-center justify-between px-4 border-b border-border shrink-0 relative"
    data-tauri-drag-region
  >
    <!-- Left - Window Controls Space + Sidebar Toggle + Home -->
    <div class="flex items-center gap-1">
      <div class="w-16"></div>
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
        <Menu size={14} />
      </Button>
    </div>

    <!-- Center - Project Name (absolutely centered) -->
    <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
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
      class="shrink-0 overflow-hidden transition-all duration-200 border-r border-border bg-muted/50 {app.leftSidebar ? 'w-48' : 'w-0'}"
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
          <div class="flex items-center justify-center h-full">
            <div class="opacity-20">
              <CaipiIcon size={128} />
            </div>
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
                {#each groupedStreamItems() as group, index (index)}
                  {#if group.type === 'text'}
                    <div
                      class="message-content text-sm leading-relaxed text-foreground/90"
                    >
                      {@html group.content ? DOMPurify.sanitize(marked.parse(group.content) as string) : ''}
                    </div>
                  {:else if group.type === 'tool-group'}
                    <ToolCallStack
                      tools={group.tools}
                      onPermissionResponse={(toolId, allowed) => handlePermissionResponse(toolId, allowed)}
                    />
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
      class="shrink-0 overflow-hidden transition-all duration-200 border-l border-border bg-muted/50 {app.rightSidebar ? 'w-48' : 'w-0'}"
    >
      <ContextPanel />
    </div>
  </div>
</div>
