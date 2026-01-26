<script lang="ts">
  import { cn } from '$lib/utils';
  import Button from './button.svelte';

  interface Props {
    percentage?: number;
    class?: string;
    onclick?: () => void;
  }

  let { percentage = 0, class: className, onclick }: Props = $props();

  const radius = 7;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = $derived(circumference - (percentage / 100) * circumference);

  // Color gradient: green (120°) -> yellow (60°) -> red (0°)
  const progressColor = $derived(`hsl(${120 - (percentage * 1.2)}, 70%, 45%)`);
</script>

<Button
  variant="ghost"
  size="sm"
  class={cn('gap-1.5 h-8 text-xs', className)}
  {onclick}
>
  <svg width="18" height="18" viewBox="0 0 18 18" class="-rotate-90">
    <!-- Background circle -->
    <circle
      cx="9"
      cy="9"
      r={radius}
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      opacity="0.2"
    />
    <!-- Progress circle -->
    <circle
      cx="9"
      cy="9"
      r={radius}
      fill="none"
      stroke={progressColor}
      stroke-width="2"
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={strokeDashoffset}
      class="transition-all duration-300"
    />
  </svg>
  <span>{percentage}%</span>
</Button>
