<script lang="ts">
  import { cn } from '$lib/utils';
  import { Copy, Check, User, Bot } from 'lucide-svelte';
  import { marked } from 'marked';
  import hljs from 'highlight.js';
  import type { Message } from '$lib/stores';

  interface Props {
    message: Message;
    streaming?: boolean;
  }

  let { message, streaming = false }: Props = $props();
  let copied = $state(false);

  // Configure marked with custom renderer for code highlighting
  const renderer = new marked.Renderer();
  renderer.code = ({ text, lang }: { text: string; lang?: string }) => {
    const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
    const highlighted = hljs.highlight(text, { language }).value;
    return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
  };

  marked.use({ renderer });

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(message.content);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }

  const isUser = $derived(message.role === 'user');
  const htmlContent = $derived(
    message.content ? marked.parse(message.content) as string : ''
  );
</script>

<div class={cn('group flex gap-4 p-4', isUser ? 'bg-muted/30' : '')}>
  <!-- Avatar -->
  <div
    class={cn(
      'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center',
      isUser ? 'bg-primary' : 'bg-secondary'
    )}
  >
    {#if isUser}
      <User class="w-4 h-4 text-primary-foreground" />
    {:else}
      <Bot class="w-4 h-4 text-secondary-foreground" />
    {/if}
  </div>

  <!-- Content -->
  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-2 mb-1">
      <span class="font-medium text-sm">
        {isUser ? 'You' : 'Claude'}
      </span>
      <span class="text-xs text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity">
        {formatTimestamp(message.timestamp)}
      </span>
    </div>

    <div
      class={cn(
        'prose prose-sm dark:prose-invert max-w-none',
        'prose-pre:bg-muted prose-pre:border prose-pre:border-border',
        'prose-code:before:content-none prose-code:after:content-none',
        streaming && 'animate-pulse'
      )}
    >
      {@html htmlContent}
      {#if streaming}
        <span class="inline-block w-2 h-4 bg-foreground animate-pulse ml-0.5"></span>
      {/if}
    </div>

    <!-- Copy Button -->
    {#if !isUser && message.content}
      <button
        onclick={copyToClipboard}
        class="mt-2 opacity-0 group-hover:opacity-100 transition-opacity p-1.5 hover:bg-muted rounded"
        title="Copy message"
      >
        {#if copied}
          <Check class="w-4 h-4 text-green-500" />
        {:else}
          <Copy class="w-4 h-4 text-muted-foreground" />
        {/if}
      </button>
    {/if}
  </div>
</div>
