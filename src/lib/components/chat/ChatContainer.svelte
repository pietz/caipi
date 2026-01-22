<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { FolderOpen, Settings, Menu, ArrowLeft } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import ChatMessage from './ChatMessage.svelte';
  import ActivityCard from './ActivityCard.svelte';
  import MessageInput from './MessageInput.svelte';
  import PermissionModal from '$lib/components/permission/PermissionModal.svelte';
  import { appStore, chatStore, type Message, type ToolActivity, type PermissionRequest } from '$lib/stores';

  interface ChatEvent {
    type: string;
    content?: string;
    activity?: ToolActivity;
    id?: string;
    status?: string;
    tool?: string;
    description?: string;
    message?: string;
  }

  let messagesContainer = $state<HTMLDivElement | null>(null);
  let unlisten: (() => void) | null = null;

  let sessionId = $state<string | null>(null);
  let folderPath = $state<string | null>(null);
  let folderName = $state<string>('');

  // Subscribe to app store
  appStore.subscribe((state) => {
    sessionId = state.sessionId;
    folderPath = state.selectedFolder;
    if (folderPath) {
      folderName = folderPath.split('/').pop() || folderPath;
    }
  });

  // Local state derived from chat store
  let messages = $state<Message[]>([]);
  let activities = $state<ToolActivity[]>([]);
  let isStreaming = $state(false);
  let streamingContent = $state('');
  let pendingPermission = $state<PermissionRequest | null>(null);

  // Subscribe to chat store
  chatStore.subscribe((state) => {
    messages = state.messages;
    activities = state.activities;
    isStreaming = state.isStreaming;
    streamingContent = state.streamingContent;
    pendingPermission = state.pendingPermission;
  });

  onMount(async () => {
    // Listen for Claude events
    unlisten = await listen<ChatEvent>('claude:event', (event) => {
      handleClaudeEvent(event.payload);
    });
  });

  onDestroy(() => {
    unlisten?.();
  });

  function handleClaudeEvent(event: ChatEvent) {
    switch (event.type) {
      case 'Text':
        if (event.content) {
          chatStore.appendStreamingContent(event.content);
        }
        break;

      case 'ToolStart':
        if (event.activity) {
          chatStore.addActivity({
            ...event.activity,
            toolType: event.activity.toolType,
            status: 'running',
          });
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
          chatStore.setPermissionRequest({
            id: event.id,
            tool: event.tool,
            description: event.description,
            timestamp: Date.now() / 1000,
          });
        }
        break;

      case 'Complete':
        // Add the streamed content as an assistant message
        const content = streamingContent;
        if (content) {
          chatStore.addMessage({
            id: crypto.randomUUID(),
            role: 'assistant',
            content,
            timestamp: Date.now() / 1000,
          });
        }
        chatStore.setStreaming(false);
        chatStore.clearActivities();
        break;

      case 'Error':
        console.error('Claude error:', event.message);
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

  async function handlePermissionResponse(allowed: boolean, remember: boolean) {
    if (!sessionId || !pendingPermission) return;

    try {
      await invoke('respond_permission', {
        sessionId,
        requestId: pendingPermission.id,
        allowed,
      });
    } catch (e) {
      console.error('Failed to respond to permission:', e);
    }

    chatStore.setPermissionRequest(null);
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

  // Combine messages and activities for display
  const displayItems = $derived([...messages] as (Message | ToolActivity)[]);
</script>

<div class="flex flex-col h-full">
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 border-b border-border bg-background">
    <Button variant="ghost" size="icon" onclick={goBack}>
      <ArrowLeft class="w-5 h-5" />
    </Button>

    <div class="flex items-center gap-2 flex-1 min-w-0">
      <FolderOpen class="w-4 h-4 text-muted-foreground flex-shrink-0" />
      <span class="font-medium truncate">{folderName}</span>
    </div>

    <Button variant="ghost" size="icon">
      <Settings class="w-5 h-5" />
    </Button>
  </header>

  <!-- Messages -->
  <div
    bind:this={messagesContainer}
    class="flex-1 overflow-y-auto"
  >
    {#if messages.length === 0 && !isStreaming}
      <!-- Empty State -->
      <div class="flex flex-col items-center justify-center h-full p-8 text-center">
        <div class="text-4xl mb-4">👋</div>
        <h2 class="text-xl font-semibold mb-2">Start a conversation</h2>
        <p class="text-muted-foreground max-w-md">
          Ask Claude to help you with coding tasks, explore your codebase, or explain how things work.
        </p>
        <div class="mt-6 space-y-2">
          <p class="text-sm text-muted-foreground">Try asking:</p>
          <div class="flex flex-wrap gap-2 justify-center">
            {#each ['What does this project do?', 'Find all TODO comments', 'Explain the main function'] as prompt}
              <button
                class="px-3 py-1.5 text-sm bg-muted hover:bg-muted/80 rounded-full transition-colors"
                onclick={() => sendMessage(prompt)}
              >
                {prompt}
              </button>
            {/each}
          </div>
        </div>
      </div>
    {:else}
      <!-- Message List -->
      {#each messages as message (message.id)}
        <ChatMessage {message} />
      {/each}

      <!-- Activities (during streaming) -->
      {#if isStreaming}
        {#each activities as activity (activity.id)}
          <ActivityCard {activity} />
        {/each}

        <!-- Streaming Message -->
        {#if streamingContent}
          <ChatMessage
            message={{
              id: 'streaming',
              role: 'assistant',
              content: streamingContent,
              timestamp: Date.now() / 1000,
            }}
            streaming={true}
          />
        {/if}
      {/if}
    {/if}
  </div>

  <!-- Input -->
  <MessageInput onSend={sendMessage} disabled={isStreaming} />

  <!-- Permission Modal -->
  {#if pendingPermission}
    <PermissionModal
      request={pendingPermission}
      onResponse={handlePermissionResponse}
    />
  {/if}
</div>
