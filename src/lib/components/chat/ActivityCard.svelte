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
    X,
  } from 'lucide-svelte';
  import type { ToolActivity, PermissionRequest } from '$lib/stores';

  interface Props {
    activity: ToolActivity;
    pendingPermission?: PermissionRequest | null;
    onPermissionResponse?: (allowed: boolean) => void;
  }

  let { activity, pendingPermission = null, onPermissionResponse }: Props = $props();
  let expanded = $state(false);

  // Check if this activity is awaiting permission
  // Match by tool type - the permission request comes for the currently running tool
  const isAwaitingPermission = $derived(
    pendingPermission !== null &&
    activity.status === 'running' &&
    activity.toolType === pendingPermission.tool
  );

  type IconComponent = typeof FileText;

  interface ToolConfig {
    icon: IconComponent;
    color: string;
  }

  const toolConfig: Record<string, ToolConfig> = {
    Read: { icon: FileText, color: 'text-blue-500' },
    Write: { icon: Pencil, color: 'text-orange-500' },
    Edit: { icon: Pencil, color: 'text-orange-500' },
    Glob: { icon: Search, color: 'text-purple-500' },
    Grep: { icon: Search, color: 'text-purple-500' },
    Bash: { icon: Terminal, color: 'text-green-500' },
  };

  const config = $derived(toolConfig[activity.toolType] || {
    icon: Terminal,
    color: 'text-muted-foreground',
  });

  // Determine the label based on status
  const label = $derived(
    isAwaitingPermission
      ? 'Waiting'
      : activity.status === 'completed'
        ? 'Ran'
        : 'Running'
  );
</script>

<div class="my-2">
  <div
    class="flex items-center gap-2 rounded-md text-sm transition-colors"
    style="
      background-color: {isAwaitingPermission ? 'rgba(234, 179, 8, 0.1)' : 'var(--muted)'};
      border: 1px solid {isAwaitingPermission ? 'rgba(234, 179, 8, 0.4)' : 'var(--border-hover)'};
    "
  >
    <button
      class={cn(
        'flex-1 flex items-center gap-2 py-2 pl-2 pr-3 text-left rounded-md',
        !isAwaitingPermission && 'hover:bg-muted/70'
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
        <span class={config.color}>{label}</span>
        <span class="ml-1">{activity.target}</span>
      </span>

      <!-- Status (only show when not awaiting permission) -->
      {#if !isAwaitingPermission}
        {#if activity.status === 'running'}
          <Loader class="w-4 h-4 animate-spin text-muted-foreground" />
        {:else if activity.status === 'completed'}
          <Check class="w-4 h-4 text-green-500" />
        {:else if activity.status === 'error'}
          <AlertCircle class="w-4 h-4 text-destructive" />
        {/if}
      {/if}

      <!-- Expand Toggle -->
      {#if activity.toolType === 'Bash' && !isAwaitingPermission}
        {#if expanded}
          <ChevronDown class="w-4 h-4 text-muted-foreground" />
        {:else}
          <ChevronRight class="w-4 h-4 text-muted-foreground" />
        {/if}
      {/if}
    </button>

    <!-- Permission buttons (inline) -->
    {#if isAwaitingPermission && onPermissionResponse}
      <div class="flex items-center gap-1 pr-2">
        <button
          type="button"
          onclick={() => onPermissionResponse(true)}
          class="w-7 h-7 flex items-center justify-center rounded bg-green-500/20 hover:bg-green-500/30 text-green-500 transition-colors"
          title="Allow"
        >
          <Check class="w-4 h-4" />
        </button>
        <button
          type="button"
          onclick={() => onPermissionResponse(false)}
          class="w-7 h-7 flex items-center justify-center rounded bg-red-500/20 hover:bg-red-500/30 text-red-500 transition-colors"
          title="Deny"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    {/if}
  </div>

  <!-- Expanded Content -->
  {#if expanded && activity.toolType === 'Bash' && !isAwaitingPermission}
    <div class="mt-1 ml-7 p-2 bg-muted rounded text-xs font-mono text-muted-foreground">
      {activity.target}
    </div>
  {/if}
</div>
