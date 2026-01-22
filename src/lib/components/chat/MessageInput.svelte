<script lang="ts">
  import { cn } from '$lib/utils';
  import { Send, Loader } from 'lucide-svelte';
  import { Button, Textarea } from '$lib/components/ui';

  interface Props {
    onSend: (message: string) => void;
    disabled?: boolean;
    placeholder?: string;
  }

  let { onSend, disabled = false, placeholder = 'Message Claude...' }: Props = $props();
  let value = $state('');
  let textareaRef = $state<HTMLTextAreaElement | null>(null);

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
</script>

<form
  onsubmit={handleSubmit}
  class="flex items-end gap-2 p-4 border-t border-border bg-background"
>
  <textarea
    bind:this={textareaRef}
    bind:value
    onkeydown={handleKeydown}
    oninput={handleInput}
    {placeholder}
    {disabled}
    rows="1"
    class={cn(
      'flex-1 resize-none rounded-lg border border-input bg-background px-4 py-2.5',
      'text-sm ring-offset-background placeholder:text-muted-foreground',
      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
      'disabled:cursor-not-allowed disabled:opacity-50',
      'max-h-[200px] overflow-y-auto'
    )}
  ></textarea>

  <Button
    type="submit"
    size="icon"
    disabled={disabled || !value.trim()}
    class="flex-shrink-0 self-end"
  >
    {#if disabled}
      <Loader class="w-5 h-5 animate-spin" />
    {:else}
      <Send class="w-5 h-5" />
    {/if}
  </Button>
</form>
