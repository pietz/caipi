<script lang="ts">
  import { api } from '$lib/api';
  import { Shield, Pencil, AlertTriangle, ArrowUp, Square } from 'lucide-svelte';
  import { Button, ContextIndicator, ModelCircle } from '$lib/components/ui';
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

  let { onSend, onQueue, onAbort, isStreaming = false, placeholder = 'Ask Claude anything...' }: Props = $props();
  let value = $state('');
  let textareaRef = $state<HTMLTextAreaElement | null>(null);

  const modeConfig: Record<PermissionMode, { label: string; icon: typeof Shield; danger?: boolean }> = {
    default: { label: 'Default', icon: Shield },
    acceptEdits: { label: 'Edit', icon: Pencil },
    bypassPermissions: { label: 'Danger', icon: AlertTriangle, danger: true },
  };

  const modelConfig: Record<Model, { label: string; size: 'large' | 'medium' | 'small' }> = {
    opus: { label: 'Opus 4.5', size: 'large' },
    sonnet: { label: 'Sonnet 4.5', size: 'medium' },
    haiku: { label: 'Haiku 4.5', size: 'small' },
  };

  // Calculate context percentage (200k token limit)
  const contextPercentage = $derived(Math.round((chat.tokenCount / 200000) * 100));

  function handleModeClick() {
    // Optimistic update - backend will confirm via StateChanged event
    app.cyclePermissionMode();
    if (app.sessionId) {
      api.setPermissionMode(app.sessionId, app.permissionMode);
    }
  }

  function handleModelClick() {
    // Optimistic update - backend will confirm via StateChanged event
    app.cycleModel();
    if (app.sessionId) {
      api.setModel(app.sessionId, app.model);
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

  const hasContent = $derived(value.trim().length > 0);
  const currentMode = $derived(modeConfig[app.permissionMode]);
  const currentModel = $derived(modelConfig[app.model]);
  const ModeIcon = $derived(currentMode.icon);
</script>

<div class="border-t border-border p-4">
  <div class="max-w-3xl mx-auto">
    <div class="bg-card rounded-xl border border-border overflow-hidden shadow-lg">
      <!-- Textarea with floating send button -->
      <div class="relative">
        <textarea
          bind:this={textareaRef}
          bind:value
          onkeydown={handleKeydown}
          oninput={handleInput}
          {placeholder}
          rows={2}
          class="w-full p-4 pr-16 bg-transparent resize-none outline-none text-sm text-foreground placeholder:text-muted-foreground"
        ></textarea>

        <!-- Floating Send/Stop Button -->
        <Button
          variant="ghost"
          class="absolute right-3 top-1/2 -translate-y-1/2 w-10 h-10 p-0 rounded-lg bg-foreground text-background hover:bg-foreground disabled:opacity-100 disabled:bg-foreground/50"
          disabled={!hasContent && !isStreaming}
          onclick={isStreaming && !hasContent ? onAbort : handleSubmit}
        >
          {#if isStreaming && !hasContent}
            <Square class="w-5 h-5" />
          {:else}
            <ArrowUp class="w-5 h-5" />
          {/if}
        </Button>
      </div>

      <!-- Footer -->
      <div class="flex items-center px-4 py-2 border-t border-border">
        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            class="w-28 justify-start gap-2 h-8 text-xs"
            onclick={handleModelClick}
          >
            <ModelCircle size={currentModel.size} />
            {currentModel.label}
          </Button>

          <Button
            variant={currentMode.danger ? 'destructive' : 'outline'}
            size="sm"
            class={cn(
              'w-24 justify-start gap-2 h-8 text-xs',
              currentMode.danger && 'bg-red-500/10 border-red-500/30 text-red-500 hover:bg-red-500/20 hover:text-red-500'
            )}
            onclick={handleModeClick}
          >
            <ModeIcon size={14} />
            {currentMode.label}
          </Button>

          <ContextIndicator percentage={contextPercentage} />
        </div>
      </div>
    </div>
  </div>
</div>
