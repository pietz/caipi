<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { renderMarkdown } from '$lib/utils/markdown';
  import type { Message } from '$lib/stores';
  import { HIDDEN_TOOL_TYPES } from './constants';
  import ToolCallStack from './ToolCallStack.svelte';
  import Divider from './Divider.svelte';

  interface Props {
    message: Message;
  }

  let { message }: Props = $props();

  const visibleTools = $derived(
    message.tools?.filter(t => !HIDDEN_TOOL_TYPES.includes(t.toolType)) ?? []
  );
  const hasTools = $derived(visibleTools.length > 0);

  function handleClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
    const anchor = target.closest('a');
    if (anchor && anchor.href) {
      event.preventDefault();
      openUrl(anchor.href);
    }
  }

  const isUser = $derived(message.role === 'user');
  const isError = $derived(message.role === 'error');
  const htmlContent = $derived(
    message.content ? renderMarkdown(message.content) : ''
  );
</script>

{#if isUser}<Divider />{/if}

<!-- Message content -->
{#if message.content}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="message-content text-sm leading-relaxed {isUser ? 'text-foreground/70 italic' : isError ? 'text-red-500' : 'text-foreground/90'}"
    class:error-message={isError}
    onclick={handleClick}
  >
    {@html htmlContent}
  </div>
{/if}

<!-- Tools (for completed messages) -->
{#if hasTools}
  <div class="mt-3">
    <ToolCallStack tools={visibleTools} />
  </div>
{/if}

{#if isUser}<Divider />{/if}

<style>
  .error-message {
    background-color: rgba(239, 68, 68, 0.1);
    padding: 8px 12px;
    border-radius: 6px;
    border: 1px solid rgba(239, 68, 68, 0.2);
  }
</style>
