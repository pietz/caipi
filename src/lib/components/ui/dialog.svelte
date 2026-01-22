<script lang="ts">
  import { cn } from '$lib/utils';
  import { X } from 'lucide-svelte';

  interface Props {
    open?: boolean;
    onClose?: () => void;
    title?: string;
    description?: string;
    children?: import('svelte').Snippet;
    footer?: import('svelte').Snippet;
  }

  let { open = false, onClose, title, description, children, footer }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      onClose?.();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose?.();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center"
    role="dialog"
    aria-modal="true"
    aria-labelledby={title ? 'dialog-title' : undefined}
  >
    <!-- Backdrop -->
    <div
      class="fixed inset-0 bg-black/80 animate-in fade-in-0"
      onclick={handleBackdropClick}
      role="presentation"
    ></div>

    <!-- Content -->
    <div
      class={cn(
        'fixed z-50 w-full max-w-lg rounded-lg border bg-background p-6 shadow-lg',
        'animate-in fade-in-0 zoom-in-95 duration-200'
      )}
    >
      {#if title || onClose}
        <div class="flex items-center justify-between mb-4">
          {#if title}
            <h2 id="dialog-title" class="text-lg font-semibold">{title}</h2>
          {/if}
          {#if onClose}
            <button
              onclick={onClose}
              class="rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
            >
              <X class="h-4 w-4" />
              <span class="sr-only">Close</span>
            </button>
          {/if}
        </div>
      {/if}

      {#if description}
        <p class="text-sm text-muted-foreground mb-4">{description}</p>
      {/if}

      {@render children?.()}

      {#if footer}
        <div class="mt-6 flex justify-end gap-2">
          {@render footer()}
        </div>
      {/if}
    </div>
  </div>
{/if}
