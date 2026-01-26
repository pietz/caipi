<script lang="ts">
  import { getToolConfig } from './tool-configs';

  interface Props {
    toolType: string;
    index: number;
    animate?: boolean;
  }

  let { toolType, index, animate = false }: Props = $props();

  const config = $derived(getToolConfig(toolType));
  const ToolIcon = $derived(config.icon);

  // Calculate offset - always stacked at 16px intervals (can be negative for slide-out)
  const offset = $derived(index * 16);
  // Higher index = higher z-index (rightmost/newest on top)
  // Use max(1, ...) to keep positive z-index for all visible elements
  const zIndex = $derived(Math.max(1, index + 10));
  // Fade out icons that are sliding off-screen
  const isExiting = $derived(index < 0);
</script>

<div
  class="tool-stack-icon absolute flex items-center justify-center w-6 h-6 rounded-full bg-muted border border-border {config.iconColor}"
  class:tool-icon-animate={animate}
  class:tool-icon-exiting={isExiting}
  style="left: {offset}px; z-index: {zIndex};"
>
  <ToolIcon size={14} />
</div>

<style>
  .tool-stack-icon {
    transition: left 200ms ease-out, opacity 200ms ease-out;
  }

  .tool-icon-animate {
    animation: tool-icon-slide-in 350ms ease-out forwards;
  }

  .tool-icon-exiting {
    opacity: 0.5;
  }
</style>
