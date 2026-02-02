<script lang="ts">
  import { Check, Loader2, X, Ban, Clock } from 'lucide-svelte';
  import type { ToolState } from '$lib/stores';
  import { getToolConfig } from './tool-configs';

  interface Props {
    tool: ToolState;
    showPermissionButtons?: boolean;
    onPermissionResponse?: (allowed: boolean) => void;
  }

  let { tool, showPermissionButtons = false, onPermissionResponse }: Props = $props();

  const config = $derived(getToolConfig(tool.toolType));
  const ToolIcon = $derived(config.icon);
  const isAwaitingPermission = $derived(tool.status === 'awaiting_permission');
</script>

<div class="flex items-center justify-between py-0.5 px-2">
  <div class="flex items-center gap-2 min-w-0">
    <div class="w-5 h-5 flex items-center justify-center {config.iconColor}">
      <ToolIcon size={14} />
    </div>
    <span class="text-xs font-medium uppercase tracking-wide text-muted-foreground shrink-0">{config.label}</span>
    <span class="text-xs text-muted-foreground/70 truncate min-w-0 flex-1">{tool.target}</span>
  </div>

  <div class="flex items-center gap-1.5 h-6 ml-3">
    {#if tool.status === 'completed' || tool.status === 'history'}
      <Check size={14} class="text-green-500" />
    {:else if tool.status === 'error'}
      <X size={14} class="text-red-500" />
    {:else if tool.status === 'aborted'}
      <Ban size={14} class="text-muted-foreground" />
    {:else if tool.status === 'denied'}
      <Ban size={14} class="text-muted-foreground" />
    {:else if tool.status === 'pending'}
      <Clock size={14} class="text-muted-foreground animate-pulse" />
    {:else if tool.status === 'running'}
      <Loader2 size={14} class="animate-spin" />
    {:else if isAwaitingPermission && showPermissionButtons && onPermissionResponse}
      <button
        type="button"
        class="h-6 w-6 rounded-md flex items-center justify-center bg-green-500/15 hover:bg-green-500/25 text-green-500 transition-colors"
        onclick={() => onPermissionResponse(true)}
        title="Allow (Enter)"
      >
        <Check size={14} />
      </button>
      <button
        type="button"
        class="h-6 w-6 rounded-md flex items-center justify-center bg-red-500/15 hover:bg-red-500/25 text-red-500 transition-colors"
        onclick={() => onPermissionResponse(false)}
        title="Deny (Esc)"
      >
        <Ban size={14} />
      </button>
    {:else if isAwaitingPermission}
      <Clock size={14} class="text-amber-500 animate-pulse" />
    {/if}
  </div>
</div>
