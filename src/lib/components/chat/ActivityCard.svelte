<script lang="ts">
  import { cn } from '$lib/utils';
  import {
    FileText,
    Pencil,
    Search,
    Terminal,
    Check,
    AlertCircle,
    Loader,
    ChevronDown,
    ChevronRight,
  } from 'lucide-svelte';
  import type { ToolActivity } from '$lib/stores';

  interface Props {
    activity: ToolActivity;
  }

  let { activity }: Props = $props();
  let expanded = $state(false);

  type IconComponent = typeof FileText;

  interface ToolConfig {
    icon: IconComponent;
    label: string;
    color: string;
  }

  const toolConfig: Record<string, ToolConfig> = {
    Read: { icon: FileText, label: 'Reading', color: 'text-blue-500' },
    Write: { icon: Pencil, label: 'Writing', color: 'text-orange-500' },
    Edit: { icon: Pencil, label: 'Editing', color: 'text-orange-500' },
    Glob: { icon: Search, label: 'Searching', color: 'text-purple-500' },
    Grep: { icon: Search, label: 'Searching', color: 'text-purple-500' },
    Bash: { icon: Terminal, label: 'Running', color: 'text-green-500' },
  };

  const config = $derived(toolConfig[activity.toolType] || {
    icon: Terminal,
    label: activity.toolType,
    color: 'text-muted-foreground',
  });
</script>

<div class="mx-4 my-2">
  <button
    class={cn(
      'w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm',
      'bg-muted/50 hover:bg-muted/70 transition-colors text-left'
    )}
    onclick={() => (expanded = !expanded)}
  >
    <!-- Tool Icon -->
    {#if config.icon === FileText}
      <FileText class={cn('w-4 h-4', config.color)} />
    {:else if config.icon === Pencil}
      <Pencil class={cn('w-4 h-4', config.color)} />
    {:else if config.icon === Search}
      <Search class={cn('w-4 h-4', config.color)} />
    {:else}
      <Terminal class={cn('w-4 h-4', config.color)} />
    {/if}

    <!-- Label -->
    <span class="flex-1 min-w-0 truncate text-muted-foreground">
      <span class={config.color}>{config.label}</span>
      <span class="ml-1">{activity.target}</span>
    </span>

    <!-- Status -->
    {#if activity.status === 'running'}
      <Loader class="w-4 h-4 animate-spin text-muted-foreground" />
    {:else if activity.status === 'completed'}
      <Check class="w-4 h-4 text-green-500" />
    {:else}
      <AlertCircle class="w-4 h-4 text-destructive" />
    {/if}

    <!-- Expand Toggle -->
    {#if activity.toolType === 'Bash'}
      {#if expanded}
        <ChevronDown class="w-4 h-4 text-muted-foreground" />
      {:else}
        <ChevronRight class="w-4 h-4 text-muted-foreground" />
      {/if}
    {/if}
  </button>

  <!-- Expanded Content -->
  {#if expanded && activity.toolType === 'Bash'}
    <div class="mt-1 ml-7 p-2 bg-muted rounded text-xs font-mono text-muted-foreground">
      {activity.target}
    </div>
  {/if}
</div>
