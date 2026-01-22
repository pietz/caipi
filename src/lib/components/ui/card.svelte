<script lang="ts">
  import { cn } from '$lib/utils';
  import type { HTMLAttributes } from 'svelte/elements';

  interface Props extends HTMLAttributes<HTMLDivElement> {}

  let { class: className, children, ...restProps }: Props = $props();
</script>

<div
  class={cn('rounded-lg border bg-card text-card-foreground shadow-sm', className)}
  {...restProps}
>
  {@render children?.()}
</div>
