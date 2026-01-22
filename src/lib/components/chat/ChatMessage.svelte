<script lang="ts">
  import { marked } from 'marked';
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
  const htmlContent = $derived(
    message.content ? marked.parse(message.content) as string : ''
  );
</script>

<div>
  <!-- Divider between messages -->
  {#if showDivider}
    <div
      class="h-px my-4"
      style="background-color: rgba(255, 255, 255, 0.04);"
    ></div>
  {/if}

  <div class="flex flex-col gap-1.5">
    <!-- Role label -->
    <div
      class="text-xs font-medium text-muted uppercase tracking-[0.5px]"
    >
      {isUser ? 'You' : 'Claude'}
    </div>

    <!-- Message content -->
    {#if message.content}
      <div
        class="text-sm leading-[1.6] whitespace-pre-wrap"
        style="color: {isUser ? 'var(--text-secondary)' : 'var(--text-primary)'};"
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
