<script lang="ts">
  import { api } from '$lib/api';
  import { Shield, Pencil, AlertTriangle, ArrowUp, Square, Brain } from 'lucide-svelte';
  import { Button, ContextIndicator, Tooltip } from '$lib/components/ui';
  import ModelSizeIcon from '$lib/components/icons/ModelSizeIcon.svelte';
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

  const modeConfig: Record<PermissionMode, { label: string; icon: typeof Shield; danger?: boolean }> = {
    default: { label: 'Default', icon: Shield },
    acceptEdits: { label: 'Edit', icon: Pencil },
    bypassPermissions: { label: 'Allow All', icon: AlertTriangle, danger: true },
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
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }


  const hasContent = $derived(value.trim().length > 0);
  const currentMode = $derived(modeConfig[app.permissionMode]);
  const currentModel = $derived(modelConfig[app.model]);
  const ModeIcon = $derived(currentMode.icon);
</script>

<div class="border-t border-border p-4">
  <div class="max-w-3xl mx-auto">
    <div class="bg-card rounded-xl border border-border shadow-lg">
      <!-- Textarea with floating send button -->
      <div class="relative">
        <textarea
          bind:value
          onkeydown={handleKeydown}
          {placeholder}
          rows={2}
          class="w-full py-3 px-4 pr-16 bg-transparent resize-none outline-none text-sm text-foreground placeholder:text-muted-foreground overflow-y-auto"
        ></textarea>

        <!-- Floating Send/Stop Button -->
        <Button
          variant="ghost"
          class="absolute right-3 top-1/2 -translate-y-1/2 z-10 w-10 h-10 p-0 rounded-lg bg-foreground text-background hover:bg-foreground hover:text-primary disabled:opacity-100 disabled:bg-foreground/50"
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
      <div class="flex items-center p-1 border-t border-border">
        <div class="flex items-center gap-2">
          <Tooltip text="Claude Model">
            <Button
              variant="ghost"
              size="sm"
              class="w-28 justify-start gap-2 h-8 text-xs"
              onclick={handleModelClick}
            >
              <ModelSizeIcon size={currentModel.size} />
              {currentModel.label}
            </Button>
          </Tooltip>

          <Tooltip text="Permission Mode">
            <Button
              variant="ghost"
              size="sm"
              class={cn(
                'w-24 justify-start gap-2 h-8 text-xs',
                currentMode.danger && 'text-red-500 hover:text-red-500'
              )}
              onclick={handleModeClick}
            >
              <ModeIcon size={14} />
              {currentMode.label}
            </Button>
          </Tooltip>

          <Tooltip text="Extended Thinking">
            <Button
              variant={app.extendedThinking ? 'outline' : 'ghost'}
              size="icon"
              class={cn(
                'h-8 w-8',
                app.extendedThinking && 'bg-purple-500/10 border-purple-500/30 text-purple-500'
              )}
              onclick={() => app.toggleExtendedThinking()}
            >
              <Brain size={14} />
            </Button>
          </Tooltip>
        </div>

        <Tooltip text="Context Usage" class="ml-auto">
          <ContextIndicator percentage={contextPercentage} />
        </Tooltip>
      </div>
    </div>
  </div>
</div>
