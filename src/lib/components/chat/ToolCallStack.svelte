<script lang="ts">
  import { ChevronDown, Check, Ban } from 'lucide-svelte';
  import type { ToolState } from '$lib/stores';
  import { chat } from '$lib/stores/chat.svelte';
  import { getToolConfig } from './tool-configs';
  import { getCompactToolTarget } from './tool-target-format';
  import ToolStackIcon from './ToolStackIcon.svelte';
  import ToolExpandedRow from './ToolExpandedRow.svelte';

  interface Props {
    tools: ToolState[];
    onPermissionResponse?: (toolId: string, allowed: boolean) => void;
  }

  let { tools, onPermissionResponse }: Props = $props();

  let expanded = $state(false);
  let pendingPermission = $state(false);

  // Stable key for this tool stack across streaming -> finalized remount.
  // The first tool id is stable for a contiguous tool group.
  const stackKey = $derived(tools[0]?.id ?? '');

  // Restore persisted expanded state on mount/remount.
  $effect(() => {
    if (!stackKey) return;
    expanded = chat.getToolStackExpanded(stackKey);
  });

  // Maximum number of visible icons in the stack
  const MAX_VISIBLE_ICONS = 5;

  // Track which tool IDs have been revealed (by ID to prevent re-animation on remount)
  // Initialize with completed tools (they don't need animation)
  const COMPLETED_STATUSES = ['completed', 'error', 'denied', 'aborted', 'history'];
  const initialRevealedIds = $derived(
    tools.filter(t => COMPLETED_STATUSES.includes(t.status)).map(t => t.id)
  );
  let revealedIds = $state<string[]>([]);

  // Sync revealedIds when tools prop changes (e.g., new completed tools from history)
  $effect(() => {
    for (const id of initialRevealedIds) {
      if (!revealedIds.includes(id)) {
        revealedIds = [...revealedIds, id];
      }
    }
  });

  // Tools that have been revealed (preserving order from tools array)
  const revealedTools = $derived(
    tools.filter(t => revealedIds.includes(t.id))
  );

  // Animate only when *this component* reveals a new tool (avoid remount flicker on completion).
  let animatedId = $state<string | null>(null);

  // Currently displayed tool (last revealed)
  const currentTool = $derived(
    revealedTools.length > 0 ? revealedTools[revealedTools.length - 1] : null
  );
  const currentConfig = $derived(currentTool ? getToolConfig(currentTool.toolType) : null);
  const currentToolTarget = $derived(
    currentTool ? getCompactToolTarget(currentTool.toolType, currentTool.target) : ''
  );

  // Simple reveal logic:
  // - Always reveal tools up to (and including) the first one needing permission
  // - If a tool needs permission, pause revealing further until permission is granted
  $effect(() => {
    // Find the first tool awaiting permission (whether revealed or not)
    const firstAwaiting = tools.find(t => t.status === 'awaiting_permission');

    // If the first awaiting is already revealed, don't reveal more until it's handled
    if (firstAwaiting && revealedIds.includes(firstAwaiting.id)) {
      return;
    }

    // Find all unrevealed tools
    const unrevealedTools = tools.filter(t => !revealedIds.includes(t.id));
    if (unrevealedTools.length === 0) return;

    let idsToReveal: string[];
    if (firstAwaiting) {
      // Reveal all tools up to and including the first one needing permission
      const targetIndex = tools.findIndex(t => t.id === firstAwaiting.id);
      idsToReveal = tools
        .slice(0, targetIndex + 1)
        .map(t => t.id)
        .filter(id => !revealedIds.includes(id));
    } else {
      // No permission needed - reveal all unrevealed tools
      idsToReveal = unrevealedTools.map(t => t.id);
    }

    if (idsToReveal.length > 0) {
      revealedIds = [...revealedIds, ...idsToReveal];
      animatedId = idsToReveal[idsToReveal.length - 1] ?? null;
    }
  });

  // First tool awaiting permission (for expanded view buttons)
  // Uses find() since tools array is already in insertion order
  const firstAwaitingPermission = $derived(
    tools.find(t => t.status === 'awaiting_permission')
  );

  // Note: don't derive animation from revealedIds directly, or completed-message remounts will re-animate.

  // Calculate the visual index for each tool (can be negative for off-screen tools)
  // This allows sliding out animation instead of instant disappear
  const getVisualIndex = (toolIndex: number): number => {
    const leftmostVisibleIndex = Math.max(0, revealedTools.length - MAX_VISIBLE_ICONS);
    return toolIndex - leftmostVisibleIndex;
  };

  // Include one extra tool on the left for slide-out animation
  const renderableTools = $derived.by(() => {
    if (revealedTools.length <= MAX_VISIBLE_ICONS) {
      return revealedTools;
    }
    // Include one extra tool that's sliding out
    const startIndex = Math.max(0, revealedTools.length - MAX_VISIBLE_ICONS - 1);
    return revealedTools.slice(startIndex);
  });

  // Calculate container width for stacked icons (always 16px offset, max MAX_VISIBLE_ICONS)
  const visibleCount = $derived(Math.min(revealedTools.length, MAX_VISIBLE_ICONS));
  const stackWidth = $derived(
    visibleCount > 0 ? (visibleCount - 1) * 16 + 24 : 0
  );

  function toggleExpanded() {
    expanded = !expanded;
    if (stackKey) {
      chat.setToolStackExpanded(stackKey, expanded);
    }
  }

  function handlePermissionResponse(toolId: string, allowed: boolean) {
    if (pendingPermission) return;
    pendingPermission = true;
    onPermissionResponse?.(toolId, allowed);
    // Reset after a short delay to allow the UI to update
    setTimeout(() => pendingPermission = false, 500);
  }
</script>

<div class="tool-call-stack my-2">
  <div class="rounded-lg border border-border bg-muted/50 overflow-hidden">
    <!-- Header/collapsed view -->
    <div class="flex items-center justify-between px-3 h-10">
      <!-- Left side: clickable area to expand -->
      <button
        type="button"
        class="flex items-center gap-2 min-w-0 flex-1 h-full hover:opacity-80 transition-opacity"
        onclick={toggleExpanded}
      >
        <!-- Stacked icons with overflow hidden for slide-out effect -->
        {#if stackWidth > 0}
          <div class="relative h-6 overflow-hidden shrink-0" style="width: {stackWidth}px;">
            {#each renderableTools as tool (tool.id)}
              {@const toolIndex = revealedTools.indexOf(tool)}
              {@const visualIndex = getVisualIndex(toolIndex)}
              <ToolStackIcon
                toolType={tool.toolType}
                index={visualIndex}
                animate={tool.id === animatedId}
              />
            {/each}
          </div>
        {/if}

        <!-- Current tool label -->
        {#if currentConfig && currentTool}
          {#key currentTool.id}
            <span class="text-xs font-medium uppercase tracking-wide text-muted-foreground tool-label-animate shrink-0">
              {currentConfig.label}
            </span>
            <span class="text-xs text-muted-foreground/70 truncate tool-label-animate min-w-0 flex-1 text-left mr-3">
              {currentToolTarget}
            </span>
          {/key}
        {/if}
      </button>

      <!-- Right side: permission buttons or count/chevron -->
      <div class="flex items-center gap-2">
        {#if firstAwaitingPermission}
          <!-- Inline permission buttons for first tool awaiting permission -->
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

        <!-- Count badge -->
        <button
          type="button"
          class="flex items-center gap-2 h-full hover:opacity-80 transition-opacity"
          onclick={toggleExpanded}
        >
          <span class="text-xs text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
            {tools.length}
          </span>
          <!-- Chevron -->
          <ChevronDown
            size={14}
            class="text-muted-foreground transition-transform {expanded ? 'rotate-180' : ''}"
          />
        </button>
      </div>
    </div>

    <!-- Expanded view - seamlessly connected with horizontal separator -->
    {#if expanded}
      <div class="border-t border-border bg-muted/30">
        {#each tools as tool (tool.id)}
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

<style>
  .tool-label-animate {
    animation: tool-label-fade-in 200ms ease-out forwards;
  }
</style>
