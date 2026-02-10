<script lang="ts">
  import { api } from '$lib/api';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy, tick } from 'svelte';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { NavigationBar } from '$lib/components/navigation';
  import { CaipiIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { Settings as SettingsPanel } from '$lib/components/settings';
  import ChatMessage from './ChatMessage.svelte';
  import ToolCallStack from './ToolCallStack.svelte';
  import MessageInput from './MessageInput.svelte';
  import { HIDDEN_TOOL_TYPES } from './constants';
  import { FileExplorer, ContextPanel } from '$lib/components/sidebar';
  import { app } from '$lib/stores/app.svelte';
  import { chat, type StreamItem, type ToolState } from '$lib/stores/chat.svelte';
  import { handleChatEvent, respondToPermission, resetEventState, setOnContentChange, type ChatEvent } from '$lib/utils/events';

  // Types for grouped stream items
  type GroupedTextItem = { type: 'text'; content: string };
  type GroupedToolItem = { type: 'tool-group'; tools: ToolState[] };
  type GroupedItem = GroupedTextItem | GroupedToolItem;

  let messagesContainer = $state<HTMLDivElement | null>(null);
  let unlisten: (() => void) | null = null;
  let cleanupKeyboardShortcuts: (() => void) | null = null;

  onMount(async () => {
    // Register scroll callback for content changes (e.g., buffer flush)
    setOnContentChange(scrollToBottom);

    // Listen for backend-neutral chat events
    unlisten = await listen<ChatEvent>('chat:event', (event) => {
      handleChatEvent(event.payload, { onComplete: processQueuedMessages });
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
    setOnContentChange(null);
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
    const turnId = crypto.randomUUID();
    chat.setActiveTurnId(turnId);

    // Start streaming
    chat.setStreaming(true);

    scrollToBottom();

    try {
      await api.sendMessage(app.sessionId, message, turnId);
    } catch (e) {
      console.error('Failed to send message:', e);
      chat.setActiveTurnId(null);
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
    const turnId = crypto.randomUUID();
    chat.setActiveTurnId(turnId);

    // Keep streaming state active
    chat.setStreaming(true);

    scrollToBottom();

    try {
      await api.sendMessage(app.sessionId, nextMessage, turnId);
    } catch (e) {
      console.error('Failed to send queued message:', e);
      chat.setActiveTurnId(null);
      chat.setStreaming(false);
    }
  }

  async function scrollToBottom() {
    // Wait for Svelte to update the DOM with new content
    await tick();

    // Wait for browser layout/paint to complete
    requestAnimationFrame(() => {
      if (messagesContainer) {
        messagesContainer.scrollTo({
          top: messagesContainer.scrollHeight,
          behavior: 'instant'
        });
      }
    });
  }

  function goBack() {
    // If streaming, finalize current state but preserve messages
    if (chat.isStreaming) {
      chat.finalize();
      resetEventState();
    }
    app.setScreen('folder');
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

{#if app.settingsOpen}
  <SettingsPanel onClose={() => app.closeSettings()} />
{:else}
  <div class="flex flex-col h-full relative">
    <!-- Navigation Bar -->
    <NavigationBar title={app.folderName} onBack={goBack} />

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
              <div class="opacity-50">
                <CaipiIcon size={192} />
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
{/if}
