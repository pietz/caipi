<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { SendIcon, StopIcon, ShieldIcon, EditIcon, AlertTriangleIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { app, type PermissionMode, type Model } from '$lib/stores/app.svelte';
  import { chat } from '$lib/stores/chat.svelte';
  import { cn } from '$lib/utils';

  interface Props {
    onSend: (message: string) => void;
    onQueue: (message: string) => void;
    onAbort?: () => void;
    isStreaming?: boolean;
    placeholder?: string;
  }

  let { onSend, onQueue, onAbort, isStreaming = false, placeholder = 'Ask Claude something...' }: Props = $props();
  let value = $state('');
  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  let focused = $state(false);

  const modeConfig: Record<PermissionMode, { label: string; color: string }> = {
    default: { label: 'Default', color: 'text-blue-400' },
    acceptEdits: { label: 'Edit', color: 'text-purple-400' },
    bypassPermissions: { label: 'Danger', color: 'text-red-400' },
  };

  const modelConfig: Record<Model, { label: string }> = {
    opus: { label: 'Opus 4.5' },
    sonnet: { label: 'Sonnet 4.5' },
    haiku: { label: 'Haiku 4.5' },
  };

  function handleModeClick() {
    // Optimistic update - backend will confirm via StateChanged event
    app.cyclePermissionMode();
    if (app.sessionId) {
      invoke('set_permission_mode', { sessionId: app.sessionId, mode: app.permissionMode });
    }
  }

  function handleModelClick() {
    // Optimistic update - backend will confirm via StateChanged event
    app.cycleModel();
    if (app.sessionId) {
      invoke('set_model', { sessionId: app.sessionId, model: app.model });
    }
  }

  function handleSubmit(e?: Event) {
    e?.preventDefault();
    if (!value.trim()) return;

    const msg = value.trim();

    if (isStreaming) {
      onQueue(msg);  // Queue during streaming
    } else {
      onSend(msg);   // Send directly
    }
    value = '';

    // Reset textarea height
    if (textareaRef) {
      textareaRef.style.height = 'auto';
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }

  function handleInput(e: Event) {
    const target = e.target as HTMLTextAreaElement;
    // Auto-resize textarea
    target.style.height = 'auto';
    target.style.height = Math.min(target.scrollHeight, 200) + 'px';
  }

  function formatDuration(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}m ${secs}s`;
  }

  function formatTokens(count: number): string {
    if (count >= 1000) {
      return `${(count / 1000).toFixed(1)}k`;
    }
    return count.toString();
  }

  const hasContent = $derived(value.trim().length > 0);
</script>

<div class="p-2 border-t border-border bg-header">
  <!-- Input wrapper -->
  <div
    class={cn(
      'flex items-center gap-2 rounded-lg p-2 transition-colors duration-150 bg-background border',
      focused ? 'border-ring' : 'border-input'
    )}
  >
    <textarea
      bind:this={textareaRef}
      bind:value
      onkeydown={handleKeydown}
      oninput={handleInput}
      onfocus={() => focused = true}
      onblur={() => focused = false}
      {placeholder}
      rows={1}
      class="flex-1 bg-transparent border-none outline-none resize-none text-sm text-foreground leading-normal p-0 m-0 align-middle max-h-[200px] overflow-y-auto"
    ></textarea>

    {#if isStreaming && !hasContent}
      <!-- Streaming with no content: show stop button -->
      <Button
        variant="destructive"
        size="sm"
        onclick={onAbort}
        class="shrink-0 p-2"
        title="Stop generation"
      >
        <StopIcon size={14} />
      </Button>
    {:else}
      <!-- Not streaming, or streaming with content: show send button -->
      <Button
        variant={hasContent ? 'default' : 'secondary'}
        size="sm"
        onclick={handleSubmit}
        disabled={!hasContent}
        class="shrink-0 p-2"
        title={isStreaming ? 'Queue message' : 'Send message'}
      >
        <SendIcon size={14} />
      </Button>
    {/if}
  </div>

  <!-- Footer row with mode/model selectors and stats -->
  <div class="flex justify-between items-center mt-2 text-xs text-darkest">
    <div class="flex items-center gap-1">
      <button
        type="button"
        onclick={handleModeClick}
        class={cn(
          'flex items-center gap-1.5 px-2 py-1 rounded transition-colors duration-100 hover:bg-hover',
          modeConfig[app.permissionMode].color
        )}
        title="Click to cycle permission mode"
      >
        {#if app.permissionMode === 'bypassPermissions'}
          <AlertTriangleIcon size={12} />
        {:else if app.permissionMode === 'acceptEdits'}
          <EditIcon size={12} />
        {:else}
          <ShieldIcon size={12} />
        {/if}
        <span>{modeConfig[app.permissionMode].label}</span>
      </button>
      <button
        type="button"
        onclick={handleModelClick}
        class="flex items-center gap-1.5 px-2 py-1 rounded transition-colors duration-100 hover:bg-hover text-muted-foreground hover:text-foreground"
        title="Click to cycle model"
      >
        <span class="w-[10px] h-[10px] flex items-center justify-center">
          <span
            class="rounded-full bg-current"
            style="width: {app.model === 'opus' ? 10 : app.model === 'sonnet' ? 7 : 5}px; height: {app.model === 'opus' ? 10 : app.model === 'sonnet' ? 7 : 5}px;"
          ></span>
        </span>
        <span>{modelConfig[app.model].label}</span>
      </button>
    </div>
    <div class="flex gap-4">
      <span>{formatTokens(chat.tokenCount)} / 200k tokens</span>
      <span>{formatDuration(chat.sessionDuration)}</span>
    </div>
  </div>
</div>
