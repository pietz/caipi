<script lang="ts">
  import { cn } from '$lib/utils';

  interface Props {
    text: string;
    position?: 'top' | 'bottom' | 'left' | 'right';
    class?: string;
    children: import('svelte').Snippet;
  }

  let { text, position = 'top', class: className, children }: Props = $props();

  const positionClasses = {
    top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
    bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
    left: 'right-full top-1/2 -translate-y-1/2 mr-2',
    right: 'left-full top-1/2 -translate-y-1/2 ml-2',
  };

  const arrowClasses = {
    top: 'top-full left-1/2 -translate-x-1/2 border-t-foreground border-x-transparent border-b-transparent',
    bottom: 'bottom-full left-1/2 -translate-x-1/2 border-b-foreground border-x-transparent border-t-transparent',
    left: 'left-full top-1/2 -translate-y-1/2 border-l-foreground border-y-transparent border-r-transparent',
    right: 'right-full top-1/2 -translate-y-1/2 border-r-foreground border-y-transparent border-l-transparent',
  };
</script>

<div class={cn('relative inline-flex group', className)}>
  {@render children()}
  <div
    class={cn(
      'absolute z-50 px-2 py-1 text-xs rounded bg-foreground text-background whitespace-nowrap',
      'opacity-0 invisible group-hover:opacity-100 group-hover:visible',
      positionClasses[position]
    )}
    role="tooltip"
  >
    {text}
    <div
      class={cn(
        'absolute w-0 h-0 border-4',
        arrowClasses[position]
      )}
    ></div>
  </div>
</div>
