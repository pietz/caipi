<script lang="ts">
  import { cn } from '$lib/utils';
  import {
    FileText,
    Pencil,
    Search,
    Terminal,
    Check,
    CircleAlert,
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

  const isAwaitingPermission = $derived(
    pendingPermission !== null &&
    activity.status === 'running' &&
    activity.toolType === pendingPermission.tool
  );

  const isBash = $derived(activity.toolType === 'Bash');

  const toolIcons: Record<string, { icon: typeof FileText; color: string }> = {
    Read: { icon: FileText, color: 'text-blue-500' },
    Write: { icon: Pencil, color: 'text-orange-500' },
    Edit: { icon: Pencil, color: 'text-orange-500' },
    Glob: { icon: Search, color: 'text-purple-500' },
    Grep: { icon: Search, color: 'text-purple-500' },
    Bash: { icon: Terminal, color: 'text-green-500' },
  };

  const toolConfig = $derived(toolIcons[activity.toolType] ?? { icon: Terminal, color: 'text-muted-foreground' });
  const ToolIcon = $derived(toolConfig.icon);
</script>

<div class="mt-1">
  <div
    class="flex items-center gap-2 rounded-md text-sm transition-colors"
    style:background-color={isAwaitingPermission ? 'rgba(234, 179, 8, 0.1)' : 'var(--muted)'}
    style:border="1px solid {isAwaitingPermission ? 'rgba(234, 179, 8, 0.4)' : 'var(--border-hover)'}"
  >
    <button
      class={cn(
        'flex-1 flex items-center gap-2 p-2 text-left rounded-md',
        !isAwaitingPermission && 'hover:bg-muted/70'
      )}
      onclick={() => (expanded = !expanded)}
    >
      <!-- Tool Icon -->
      <ToolIcon class={cn('w-4 h-4 flex-shrink-0', toolConfig.color)} />

      <!-- Target -->
      <span class="flex-1 min-w-0 truncate text-muted-foreground">
        {activity.target}
      </span>

      <!-- Status Icon (fixed size container prevents layout shift) -->
      <div class="w-4 h-4 flex-shrink-0">
        {#if !isAwaitingPermission && activity.status === 'running'}
          <Loader class="w-4 h-4 animate-spin text-muted-foreground" />
        {:else if !isAwaitingPermission && activity.status === 'completed'}
          <Check class="w-4 h-4 text-green-500" />
        {:else if !isAwaitingPermission && activity.status === 'error'}
          <CircleAlert class="w-4 h-4 text-destructive" />
        {/if}
      </div>

      <!-- Expand Toggle (fixed size container, only visible for Bash) -->
      {#if isBash}
        <div class="w-4 h-4 flex-shrink-0">
          {#if !isAwaitingPermission}
            {#if expanded}
              <ChevronDown class="w-4 h-4 text-muted-foreground" />
            {:else}
              <ChevronRight class="w-4 h-4 text-muted-foreground" />
            {/if}
          {/if}
        </div>
      {/if}
    </button>

    <!-- Permission Buttons -->
    {#if isAwaitingPermission && onPermissionResponse}
      <div class="flex items-center gap-1 pr-2">
        <button
          type="button"
          onclick={() => onPermissionResponse(true)}
          class="w-7 h-7 flex items-center justify-center rounded bg-green-500/20 hover:bg-green-500/30 text-green-500 transition-colors"
          title="Allow (Enter)"
        >
          <Check class="w-4 h-4" />
        </button>
        <button
          type="button"
          onclick={() => onPermissionResponse(false)}
          class="w-7 h-7 flex items-center justify-center rounded bg-red-500/20 hover:bg-red-500/30 text-red-500 transition-colors"
          title="Deny (Esc)"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    {/if}
  </div>

  <!-- Expanded Content (Bash only) -->
  {#if isBash && expanded && !isAwaitingPermission}
    <div class="mt-1 ml-7 p-2 bg-muted rounded text-xs font-mono text-muted-foreground">
      {activity.target}
    </div>
  {/if}
</div>
