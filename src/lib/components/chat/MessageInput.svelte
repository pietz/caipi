<script lang="ts">
  import { api } from '$lib/api';
  import { Shield, Pencil, AlertTriangle, ArrowUp, Brain } from 'lucide-svelte';
  import { Button, ContextIndicator, Tooltip } from '$lib/components/ui';
  import ModelSizeIcon from '$lib/components/icons/ModelSizeIcon.svelte';
  import { app, type PermissionMode } from '$lib/stores/app.svelte';
  import { chat } from '$lib/stores/chat.svelte';
  import { cn } from '$lib/utils';

  interface Props {
    onSend: (message: string) => void;
    onQueue: (message: string) => void;
    onAbort?: () => void;
    isStreaming?: boolean;
    placeholder?: string;
  }

  let { onSend, onQueue, onAbort, isStreaming = false, placeholder = 'Ask anything...' }: Props = $props();
  let value = $state('');

  const modeConfig: Record<PermissionMode, { label: string; icon: typeof Shield; danger?: boolean }> = {
    default: { label: 'Default', icon: Shield },
    acceptEdits: { label: 'Edit', icon: Pencil },
    bypassPermissions: { label: 'Allow All', icon: AlertTriangle, danger: true },
  };

  // Prefer runtime context window when provided by backend, else fallback to static config.
  const contextLimit = $derived(chat.contextWindow ?? app.backendConfig.contextLimit);
  const contextPercentage = $derived(Math.round((chat.tokenCount / contextLimit) * 100));

  // Get current thinking option label
  const thinkingLabel = $derived(
    app.thinkingOptions.find(opt => opt.value === app.thinkingLevel)?.label ?? ''
  );

  function handleModeClick() {
    // Optimistic update - backend will confirm via StateChanged event
    app.cyclePermissionMode();
    if (app.sessionId) {
      api.setPermissionMode(app.sessionId, app.permissionMode).catch(console.error);
    }
  }

  function handleModelClick() {
    // Optimistic update - backend will confirm via StateChanged event
    app.cycleModel();
    if (app.sessionId) {
      api.setModel(app.sessionId, app.model).catch(console.error);
    }
  }

  function handleThinkingClick() {
    app.cycleThinking();
    if (app.sessionId) {
      api.setThinkingLevel(app.sessionId, app.thinkingLevel).catch(console.error);
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
  const currentModel = $derived(
    app.backendConfig.models.find((m) => m.id === app.model) ?? app.backendConfig.models[0]
  );
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
          aria-label={isStreaming && !hasContent ? "Stop generation" : "Send message"}
          class="absolute right-3 top-1/2 -translate-y-1/2 z-10 w-10 h-10 p-0 rounded-lg bg-foreground text-background hover:bg-foreground hover:text-primary disabled:opacity-100 disabled:bg-foreground/50"
          disabled={!hasContent && !isStreaming}
          onclick={isStreaming && !hasContent ? onAbort : handleSubmit}
        >
          {#if isStreaming && !hasContent}
            <div class="relative flex items-center justify-center">
              <svg class="absolute w-7 h-7 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" class="opacity-20" />
                <path d="M12 2a10 10 0 0 1 10 10" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
              </svg>
              <div class="w-2.5 h-2.5 bg-current"></div>
            </div>
          {:else}
            <ArrowUp class="w-5 h-5" />
          {/if}
        </Button>
      </div>

      <!-- Footer -->
      <div class="flex items-center p-1 border-t border-border">
        <div class="flex items-center gap-2">
          <Tooltip text="Model">
            <Button
              variant="ghost"
              size="sm"
              class="justify-start gap-2 h-8 text-xs whitespace-nowrap"
              onclick={handleModelClick}
            >
              <ModelSizeIcon size={currentModel.size} backend={app.activeBackend} />
              {currentModel.name}
            </Button>
          </Tooltip>

          {#if app.thinkingOptions.length > 0}
            <Tooltip text="Thinking">
              <Button
                variant="ghost"
                size="sm"
                class="justify-start gap-2 h-8 text-xs whitespace-nowrap"
                onclick={handleThinkingClick}
              >
                <Brain size={14} />
                {thinkingLabel}
              </Button>
            </Tooltip>
          {/if}

          <Tooltip text="Permission Mode">
            <Button
              variant="ghost"
              size="sm"
              class={cn(
                'justify-start gap-2 h-8 text-xs whitespace-nowrap',
                currentMode.danger && 'text-red-500 hover:text-red-500'
              )}
              onclick={handleModeClick}
            >
              <ModeIcon size={14} />
              {currentMode.label}
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
