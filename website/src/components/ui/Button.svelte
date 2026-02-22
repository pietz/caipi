<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'ghost' | 'outline';
    size?: 'sm' | 'md' | 'lg';
    href?: string;
    class?: string;
    children: Snippet;
    [key: string]: unknown;
  }

  let {
    variant = 'primary',
    size = 'md',
    href,
    class: className = '',
    children,
    ...rest
  }: Props = $props();

  const baseClasses = 'inline-flex items-center justify-center font-medium rounded-lg transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:ring-offset-background disabled:opacity-50 disabled:pointer-events-none';

  const variants = {
    primary: 'bg-primary text-primary-foreground hover:bg-primary/90',
    secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
    ghost: 'hover:bg-muted text-foreground',
    outline: 'border border-border hover:bg-muted text-foreground',
  };

  const sizes = {
    sm: 'text-sm px-3 py-1.5',
    md: 'text-sm px-4 py-2',
    lg: 'text-base px-6 py-3',
  };

  const classes = `${baseClasses} ${variants[variant]} ${sizes[size]} ${className}`;
</script>

{#if href}
  <a {href} class={classes} {...rest}>
    {@render children()}
  </a>
{:else}
  <button class={classes} {...rest}>
    {@render children()}
  </button>
{/if}
