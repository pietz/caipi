<script lang="ts">
  import { SendIcon, StopIcon } from '$lib/components/icons';
  import { Button } from '$lib/components/ui';
  import { chatStore } from '$lib/stores';
  import { cn } from '$lib/utils';

  interface Props {
    onSend: (message: string) => void;
    onAbort?: () => void;
    disabled?: boolean;
    placeholder?: string;
  }

  let { onSend, onAbort, disabled = false, placeholder = 'Ask Claude something...' }: Props = $props();
  let value = $state('');
  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  let focused = $state(false);

  let tokenCount = $state(0);
  let sessionDuration = $state(0);

  chatStore.subscribe((state) => {
    tokenCount = state.tokenCount;
    sessionDuration = state.sessionDuration;
  });

  function handleSubmit(e?: Event) {
    e?.preventDefault();
    if (!value.trim() || disabled) return;

    onSend(value.trim());
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

<div class="py-3 px-4 border-t border-border bg-header">
  <!-- Input wrapper -->
  <div
    class={cn(
      'flex items-center gap-2 rounded-lg py-2.5 px-3 transition-colors duration-150 bg-input border',
      focused ? 'border-[var(--ring)]' : 'border-input'
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
      disabled={disabled}
      rows={1}
      class="flex-1 bg-transparent border-none outline-none resize-none text-sm text-primary leading-[1.4] p-0 m-0 align-middle max-h-[200px] overflow-y-auto disabled:cursor-not-allowed disabled:opacity-50"
    ></textarea>

    {#if disabled}
      <Button
        variant="destructive"
        size="sm"
        onclick={onAbort}
        class="shrink-0 px-2 py-1.5"
        title="Stop generation"
      >
        <StopIcon size={14} />
      </Button>
    {:else}
      <Button
        variant={hasContent ? 'default' : 'secondary'}
        size="sm"
        onclick={handleSubmit}
        disabled={!hasContent}
        class="shrink-0 px-2 py-1.5"
      >
        <SendIcon size={14} />
      </Button>
    {/if}
  </div>

  <!-- Footer row with hints and stats -->
  <div class="flex justify-between items-center mt-2 text-xs text-darkest">
    <span>⇧↵ new line · ⌘↵ send</span>
    <div class="flex gap-4">
      <span>{formatTokens(tokenCount)} / 200k tokens</span>
      <span>{formatDuration(sessionDuration)}</span>
    </div>
  </div>
</div>
