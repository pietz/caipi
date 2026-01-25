<script lang="ts">
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import hljs from 'highlight.js';
  import type { Message } from '$lib/stores';
  import { HIDDEN_TOOL_TYPES } from './constants';
  import ActivityCard from './ActivityCard.svelte';
  import Divider from './Divider.svelte';

  interface Props {
    message: Message;
  }

  let { message }: Props = $props();

  const visibleActivities = $derived(
    message.activities?.filter(a => !HIDDEN_TOOL_TYPES.includes(a.toolType)) ?? []
  );
  const hasActivities = $derived(visibleActivities.length > 0);

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

{#if isUser}<Divider />{/if}

<!-- Message content -->
{#if message.content}
  <div
    class="message-content text-sm leading-relaxed {isUser ? 'text-foreground/70 italic' : isError ? 'text-red-500' : 'text-foreground/90'}"
    class:error-message={isError}
  >
    {@html htmlContent}
  </div>
{/if}

<!-- Activities (for completed messages) -->
{#if hasActivities}
  <div class="mt-3">
    {#each visibleActivities as activity (activity.id)}
      <ActivityCard {activity} />
    {/each}
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
