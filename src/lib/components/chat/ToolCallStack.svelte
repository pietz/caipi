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

  // Stable per-tool key that also disambiguates duplicate tool ids (can happen in merged history).
  const toolKey = (tool: ToolState) => `${tool.id}:${tool.insertionIndex}`;

  // Stable key for this tool stack across streaming -> finalized remount.
  const stackKey = $derived(tools[0] ? toolKey(tools[0]) : '');

  $effect(() => {
    if (!stackKey) return;
    expanded = chat.getToolStackExpanded(stackKey);
  });

  const MAX_VISIBLE_ICONS = 5;

  const COMPLETED_STATUSES = ['completed', 'error', 'denied', 'aborted', 'history'];
  const initialRevealedKeys = $derived(
    tools.filter(t => COMPLETED_STATUSES.includes(t.status)).map(toolKey)
  );
  let revealedKeys = $state<string[]>([]);

  $effect(() => {
    for (const key of initialRevealedKeys) {
      if (!revealedKeys.includes(key)) {
        revealedKeys = [...revealedKeys, key];
      }
    }
  });

  const revealedTools = $derived(
    tools.filter(t => revealedKeys.includes(toolKey(t)))
  );

  let animatedKey = $state<string | null>(null);

  const currentTool = $derived(
    revealedTools.length > 0 ? revealedTools[revealedTools.length - 1] : null
  );
  const currentConfig = $derived(currentTool ? getToolConfig(currentTool.toolType) : null);
  const currentToolTarget = $derived(
    currentTool ? getCompactToolTarget(currentTool.toolType, currentTool.target) : ''
  );

  $effect(() => {
    const firstAwaitingIndex = tools.findIndex(t => t.status === 'awaiting_permission');

    if (firstAwaitingIndex >= 0) {
      const awaitingKey = toolKey(tools[firstAwaitingIndex]!);
      if (revealedKeys.includes(awaitingKey)) {
        return;
      }
    }

    const unrevealedTools = tools.filter(t => !revealedKeys.includes(toolKey(t)));
    if (unrevealedTools.length === 0) return;

    let keysToReveal: string[];
    if (firstAwaitingIndex >= 0) {
      keysToReveal = tools
        .slice(0, firstAwaitingIndex + 1)
        .map(toolKey)
        .filter(key => !revealedKeys.includes(key));
    } else {
      keysToReveal = unrevealedTools.map(toolKey);
    }

    if (keysToReveal.length > 0) {
      revealedKeys = [...revealedKeys, ...keysToReveal];
      animatedKey = keysToReveal[keysToReveal.length - 1] ?? null;
    }
  });

  const firstAwaitingPermission = $derived(
    tools.find(t => t.status === 'awaiting_permission')
  );

  const getVisualIndex = (toolIndex: number): number => {
    const leftmostVisibleIndex = Math.max(0, revealedTools.length - MAX_VISIBLE_ICONS);
    return toolIndex - leftmostVisibleIndex;
  };

  const renderableTools = $derived.by(() => {
    if (revealedTools.length <= MAX_VISIBLE_ICONS) {
      return revealedTools;
    }
    const startIndex = Math.max(0, revealedTools.length - MAX_VISIBLE_ICONS - 1);
    return revealedTools.slice(startIndex);
  });

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
            {#each renderableTools as tool (toolKey(tool))}
              {@const toolIndex = revealedTools.indexOf(tool)}
              {@const visualIndex = getVisualIndex(toolIndex)}
              <ToolStackIcon
                toolType={tool.toolType}
                index={visualIndex}
                animate={toolKey(tool) === animatedKey}
              />
            {/each}
          </div>
        {/if}

        <!-- Current tool label -->
        {#if currentConfig && currentTool}
          {#key toolKey(currentTool)}
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
        >
          <span class="text-xs text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
            {tools.length}
          </span>
          <ChevronDown
            size={14}
            class="text-muted-foreground transition-transform {expanded ? 'rotate-180' : ''}"
          />
        </button>
      </div>
    </div>

    {#if expanded}
      <div class="border-t border-border bg-muted/30">
        {#each tools as tool (toolKey(tool))}
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

