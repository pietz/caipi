<script lang="ts" module>
  import type { FileEntry } from '$lib/stores';
</script>

<script lang="ts">
  import { FolderIcon, FileIcon, ChevronIcon } from '$lib/components/icons';
  import { filesStore } from '$lib/stores';
  import { cn } from '$lib/utils';
  import FileTreeItem from './FileTreeItem.svelte';

  interface Props {
    item: FileEntry;
    depth?: number;
  }

  let { item, depth = 0 }: Props = $props();

  let expanded = $state(false);
  let selectedPath = $state<string | null>(null);

  filesStore.subscribe((state) => {
    expanded = state.expandedPaths.has(item.path);
    selectedPath = state.selectedPath;
  });

  const isFolder = $derived(item.type === 'folder');
  const isSelected = $derived(selectedPath === item.path);

  function handleClick() {
    if (isFolder) {
      filesStore.toggleExpanded(item.path);
    } else {
      filesStore.setSelectedPath(item.path);
    }
  }
</script>

<div>
  <button
    type="button"
    onclick={handleClick}
    class={cn(
      'flex items-center gap-1 text-left transition-colors duration-100 rounded mx-1 my-px py-[3px] pr-2 text-[13px]',
      isSelected ? 'bg-selected text-accent' : 'text-file hover:bg-hover'
    )}
    style="padding-left: {8 + depth * 12}px;"
  >
    {#if isFolder}
      <span class="w-3 flex items-center text-[#666]">
        <ChevronIcon {expanded} size={12} />
      </span>
    {:else}
      <span class="w-3"></span>
    {/if}

    <span class={cn('flex items-center', isFolder ? 'text-folder' : 'text-file')}>
      {#if isFolder}
        <FolderIcon size={14} />
      {:else}
        <FileIcon size={14} />
      {/if}
    </span>

    <span class={isSelected ? 'text-accent' : 'text-selected'}>
      {item.name}
    </span>
  </button>

  {#if isFolder && expanded && item.children}
    <div>
      {#each item.children as child (child.path)}
        <FileTreeItem item={child} depth={depth + 1} />
      {/each}
    </div>
  {/if}
</div>
