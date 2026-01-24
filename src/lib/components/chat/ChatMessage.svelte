<script lang="ts">
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import hljs from 'highlight.js';
  import type { Message } from '$lib/stores';
  import ActivityCard from './ActivityCard.svelte';

  interface Props {
    message: Message;
    streaming?: boolean;
    showDivider?: boolean;
  }

  let { message, streaming = false, showDivider = false }: Props = $props();

  const hasActivities = $derived(message.activities && message.activities.length > 0);

  // Configure marked with custom renderer for code highlighting
  const renderer = new marked.Renderer();
  renderer.code = ({ text, lang }: { text: string; lang?: string }) => {
    const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
    const highlighted = hljs.highlight(text, { language }).value;
    return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
  };

  marked.use({ renderer });

  const isUser = $derived(message.role === 'user');
  const isError = $derived(message.role === 'error');
  const htmlContent = $derived(
    message.content ? DOMPurify.sanitize(marked.parse(message.content) as string) : ''
  );
</script>

<div>
  <!-- Divider between messages -->
  {#if showDivider}
    <div class="h-px my-2" style:background-color="rgba(255, 255, 255, 0.04)"></div>
  {/if}

  <div class="flex flex-col gap-1">
    <!-- Role label -->
    <div
      class="text-xs font-medium uppercase tracking-wide"
      style:color={isError ? '#ef4444' : 'var(--text-muted)'}
    >
      {isUser ? 'You' : isError ? 'Error' : 'Claude'}
    </div>

    <!-- Message content -->
    {#if message.content}
      <div
        class="message-content text-sm leading-relaxed"
        class:error-message={isError}
        style:color={isError ? '#ef4444' : isUser ? 'var(--text-secondary)' : 'var(--text-primary)'}
      >
        {#if streaming}
          {@html htmlContent}
          <span class="inline-block w-0.5 h-4 bg-foreground animate-pulse ml-0.5"></span>
        {:else}
          {@html htmlContent}
        {/if}
      </div>
    {/if}

    <!-- Activities (for completed messages) -->
    {#if hasActivities}
      {#each message.activities as activity (activity.id)}
        <ActivityCard {activity} />
      {/each}
    {/if}
  </div>
</div>

<style>
  .error-message {
    background-color: rgba(239, 68, 68, 0.1);
    padding: 8px 12px;
    border-radius: 6px;
    border: 1px solid rgba(239, 68, 68, 0.2);
  }

  /* Tighten markdown spacing */
  :global(.message-content p) {
    margin: 0.5em 0;
  }

  :global(.message-content p:first-child) {
    margin-top: 0;
  }

  :global(.message-content p:last-child) {
    margin-bottom: 0;
  }

  :global(.message-content ul),
  :global(.message-content ol) {
    margin: 0.5em 0;
    padding-left: 1.5em;
  }

  :global(.message-content li) {
    margin: 0.25em 0;
  }

  :global(.message-content pre) {
    margin: 0.5em 0;
    border-radius: 6px;
    overflow-x: auto;
  }

  :global(.message-content code:not(pre code)) {
    background: hsl(var(--muted));
    padding: 0.15em 0.4em;
    border-radius: 4px;
    font-size: 0.9em;
  }

  :global(.message-content blockquote) {
    margin: 0.5em 0;
    padding-left: 1em;
    border-left: 3px solid hsl(var(--border));
    color: var(--text-secondary);
  }

  :global(.message-content h1),
  :global(.message-content h2),
  :global(.message-content h3),
  :global(.message-content h4) {
    margin: 0.75em 0 0.5em 0;
    font-weight: 600;
  }

  :global(.message-content h1:first-child),
  :global(.message-content h2:first-child),
  :global(.message-content h3:first-child),
  :global(.message-content h4:first-child) {
    margin-top: 0;
  }
</style>
