<script lang="ts">
  import { PanelLeft, PanelRight, Settings, Menu } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import { app } from '$lib/stores/app.svelte';
  import { isMacOS } from '$lib/utils/platform';

  interface Props {
    title: string;
    onBack: () => void;
  }

  let { title, onBack }: Props = $props();

  const macOS = isMacOS();
</script>

{#if macOS}
  <!-- macOS: Integrated title bar with traffic light space -->
  <div
    class="h-9 flex items-center justify-between px-4 border-b border-border shrink-0 relative"
    data-tauri-drag-region
  >
    <!-- Left - Window Controls Space + Sidebar Toggle + Home -->
    <div class="flex items-center gap-1">
      <div class="w-16"></div>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.toggleLeftSidebar()}
      >
        <PanelLeft size={14} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={onBack}
      >
        <Menu size={14} />
      </Button>
    </div>

    <!-- Center - Project Name (absolutely centered) -->
    <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
      <span class="text-sm font-medium">{title}</span>
    </div>

    <!-- Right - Controls -->
    <div class="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.openSettings()}
      >
        <Settings size={14} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.toggleRightSidebar()}
      >
        <PanelRight size={14} />
      </Button>
    </div>
  </div>
{:else}
  <!-- Windows/Linux: Toolbar below native title bar -->
  <div
    class="h-9 flex items-center justify-between px-4 border-b border-border shrink-0 relative"
  >
    <!-- Left - Sidebar Toggle + Home -->
    <div class="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.toggleLeftSidebar()}
      >
        <PanelLeft size={14} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={onBack}
      >
        <Menu size={14} />
      </Button>
    </div>

    <!-- Center - Project Name (absolutely centered) -->
    <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
      <span class="text-sm font-medium">{title}</span>
    </div>

    <!-- Right - Controls -->
    <div class="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.openSettings()}
      >
        <Settings size={14} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={() => app.toggleRightSidebar()}
      >
        <PanelRight size={14} />
      </Button>
    </div>
  </div>
{/if}
