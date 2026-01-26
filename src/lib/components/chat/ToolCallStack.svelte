<script lang="ts">
  import { ChevronDown, Check, X } from 'lucide-svelte';
  import type { ToolState } from '$lib/stores';
  import { getToolConfig } from './tool-configs';
  import ToolStackIcon from './ToolStackIcon.svelte';
  import ToolExpandedRow from './ToolExpandedRow.svelte';

  interface Props {
    tools: ToolState[];
    onPermissionResponse?: (toolId: string, allowed: boolean) => void;
  }

  let { tools, onPermissionResponse }: Props = $props();

  let expanded = $state(false);

  // Maximum number of visible icons in the stack
  const MAX_VISIBLE_ICONS = 5;

  // Track which tool IDs have been revealed (by ID to prevent re-animation on remount)
  // Initialize with completed tools (they don't need animation)
  const COMPLETED_STATUSES = ['completed', 'error', 'denied', 'aborted'];
  let revealedIds = $state<string[]>(
    tools.filter(t => COMPLETED_STATUSES.includes(t.status)).map(t => t.id)
  );

  // Tools that have been revealed (preserving order from tools array)
  const revealedTools = $derived(
    tools.filter(t => revealedIds.includes(t.id))
  );

  // Currently displayed tool (last revealed)
  const currentTool = $derived(
    revealedTools.length > 0 ? revealedTools[revealedTools.length - 1] : null
  );
  const currentConfig = $derived(currentTool ? getToolConfig(currentTool.toolType) : null);

  // Check if current tool needs permission (for inline buttons)
  const currentNeedsPermission = $derived(
    currentTool?.status === 'awaiting_permission'
  );

  // Simple reveal logic:
  // - Reveal all unrevealed tools up to (and including) the first one needing permission
  // - If current tool needs permission, pause (don't reveal more)
  // - When permission is granted, this effect re-runs and reveals more
  $effect(() => {
    // If current tool needs permission, pause revealing
    if (currentTool?.status === 'awaiting_permission') {
      return;
    }

    // Find all unrevealed tools
    const unrevealedTools = tools.filter(t => !revealedIds.includes(t.id));
    if (unrevealedTools.length === 0) return;

    // Find the first unrevealed tool that needs permission (if any)
    const firstNeedingPermission = unrevealedTools.find(
      t => t.status === 'awaiting_permission'
    );

    let idsToReveal: string[];
    if (firstNeedingPermission) {
      // Reveal all tools up to and including the first one needing permission
      const targetIndex = tools.findIndex(t => t.id === firstNeedingPermission.id);
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
    }
  });

  // First tool awaiting permission (for expanded view buttons)
  // Uses find() since tools array is already in insertion order
  const firstAwaitingPermission = $derived(
    tools.find(t => t.status === 'awaiting_permission')
  );

  // The ID of the most recently revealed tool (for animation)
  const lastRevealedId = $derived(
    revealedIds.length > 0 ? revealedIds[revealedIds.length - 1] : null
  );

  // Calculate the visual index for each tool (can be negative for off-screen tools)
  // This allows sliding out animation instead of instant disappear
  const getVisualIndex = (toolIndex: number): number => {
    const leftmostVisibleIndex = Math.max(0, revealedTools.length - MAX_VISIBLE_ICONS);
    return toolIndex - leftmostVisibleIndex;
  };

  // Include one extra tool on the left for slide-out animation
  const renderableTools = $derived(() => {
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

  function handlePermissionResponse(toolId: string, allowed: boolean) {
    onPermissionResponse?.(toolId, allowed);
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
        onclick={() => expanded = !expanded}
      >
        <!-- Stacked icons with overflow hidden for slide-out effect -->
        {#if stackWidth > 0}
          <div class="relative h-6 overflow-hidden shrink-0" style="width: {stackWidth}px;">
            {#each renderableTools() as tool (tool.id)}
              {@const toolIndex = revealedTools.indexOf(tool)}
              {@const visualIndex = getVisualIndex(toolIndex)}
              <ToolStackIcon
                toolType={tool.toolType}
                index={visualIndex}
                animate={tool.id === lastRevealedId}
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
            <span class="text-xs text-muted-foreground/70 truncate tool-label-animate min-w-0">
              {currentTool.target}
            </span>
          {/key}
        {/if}
      </button>

      <!-- Right side: permission buttons or count/chevron -->
      <div class="flex items-center gap-2">
        {#if currentNeedsPermission && currentTool}
          <!-- Inline permission buttons -->
          <button
            type="button"
            class="h-6 w-6 rounded-md flex items-center justify-center bg-green-500/15 hover:bg-green-500/25 text-green-500 transition-colors"
            onclick={() => handlePermissionResponse(currentTool.id, true)}
            title="Allow (Enter)"
          >
            <Check size={14} />
          </button>
          <button
            type="button"
            class="h-6 w-6 rounded-md flex items-center justify-center bg-red-500/15 hover:bg-red-500/25 text-red-500 transition-colors"
            onclick={() => handlePermissionResponse(currentTool.id, false)}
            title="Deny (Esc)"
          >
            <X size={14} />
          </button>
        {/if}

        <!-- Count badge -->
        <button
          type="button"
          class="flex items-center gap-2 h-full hover:opacity-80 transition-opacity"
          onclick={() => expanded = !expanded}
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
        {#each tools.slice().reverse() as tool (tool.id)}
          <ToolExpandedRow
            {tool}
            showPermissionButtons={firstAwaitingPermission?.id === tool.id && tool.id !== currentTool?.id}
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
