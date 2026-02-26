<script lang="ts">
  import { Bot, ChevronDown, Check, Ban, Loader2 } from 'lucide-svelte';
  import { chat } from '$lib/stores/chat.svelte';
  import type { SubagentGroup } from './subagent-grouping';
  import ToolExpandedRow from './ToolExpandedRow.svelte';

  interface Props {
    group: SubagentGroup;
    onPermissionResponse?: (toolId: string, allowed: boolean) => void;
  }

  let { group, onPermissionResponse }: Props = $props();

  let expanded = $state(false);
  let pendingPermission = $state(false);

  const storageKey = $derived(group.id);

  $effect(() => {
    expanded = chat.getSubagentGroupExpanded(storageKey);
  });

  const firstAwaitingPermission = $derived(
    group.tools.find(tool => tool.status === 'awaiting_permission')
  );

  function toggleExpanded() {
    expanded = !expanded;
    chat.setSubagentGroupExpanded(storageKey, expanded);
  }

  function handlePermissionResponse(toolId: string, allowed: boolean) {
    if (pendingPermission) return;
    pendingPermission = true;
    onPermissionResponse?.(toolId, allowed);
    setTimeout(() => pendingPermission = false, 500);
  }
</script>

<div class="subagent-group my-2">
  <div class="rounded-lg border border-border bg-muted/40 overflow-hidden">
    <div class="h-10 px-3 flex items-center justify-between">
      <button
        type="button"
        class="h-full flex items-center gap-2 min-w-0 flex-1 hover:opacity-80 transition-opacity text-left"
        onclick={toggleExpanded}
      >
        <div class="w-6 h-6 rounded-full bg-muted border border-border flex items-center justify-center text-blue-500 shrink-0">
          <Bot size={14} />
        </div>
        <span class="text-xs font-medium uppercase tracking-wide text-muted-foreground shrink-0">
          agent
        </span>
        <span class="text-xs text-muted-foreground/70 truncate min-w-0">{group.title}</span>
      </button>

      <div class="flex items-center gap-2">
        {#if group.status === 'running'}
          <Loader2 size={14} class="animate-spin text-blue-500" />
        {/if}

        {#if firstAwaitingPermission}
          <button
            type="button"
            class="h-6 w-6 rounded-md flex items-center justify-center bg-green-500/15 hover:bg-green-500/25 text-green-500 transition-colors"
            onclick={() => handlePermissionResponse(firstAwaitingPermission.id, true)}
            title="Allow (Enter)"
          >
            <Check size={14} />
          </button>
          <button
            type="button"
            class="h-6 w-6 rounded-md flex items-center justify-center bg-red-500/15 hover:bg-red-500/25 text-red-500 transition-colors"
            onclick={() => handlePermissionResponse(firstAwaitingPermission.id, false)}
            title="Deny (Esc)"
          >
            <Ban size={14} />
          </button>
        {/if}

        <button
          type="button"
          class="flex items-center gap-2 h-full hover:opacity-80 transition-opacity"
          onclick={toggleExpanded}
          title={expanded ? 'Collapse' : 'Expand'}
        >
          <span class="text-xs text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
            {group.tools.length}
          </span>
          <ChevronDown
            size={14}
            class="text-muted-foreground transition-transform {expanded ? 'rotate-180' : ''}"
          />
        </button>
      </div>
    </div>

    {#if expanded}
      <div class="border-t border-border bg-muted/20">
        {#each group.tools as tool (tool.id)}
          <ToolExpandedRow
            {tool}
            showPermissionButtons={tool.status === 'awaiting_permission' && tool.id !== firstAwaitingPermission?.id}
            onPermissionResponse={(allowed) => handlePermissionResponse(tool.id, allowed)}
          />
        {/each}
      </div>
    {/if}
  </div>
</div>
