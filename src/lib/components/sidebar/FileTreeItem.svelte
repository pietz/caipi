<script lang="ts" module>
  import type { FileEntry } from '$lib/stores';
</script>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { FileIcon, ChevronIcon } from '$lib/components/icons';
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
  let loadingChildren = $state(false);
  let childrenLoaded = $state(false);

  filesStore.subscribe((state) => {
    expanded = state.expandedPaths.has(item.path);
    selectedPath = state.selectedPath;
  });

  const isFolder = $derived(item.type === 'folder');
  const isSelected = $derived(selectedPath === item.path);
  const hasChildren = $derived(item.children && item.children.length > 0);

  async function loadChildren() {
    if (childrenLoaded || loadingChildren) return;

    loadingChildren = true;
    try {
      const entries = await invoke<FileEntry[]>('list_directory', { path: item.path });
      filesStore.updateChildren(item.path, entries);
      childrenLoaded = true;
    } catch (e) {
      console.error('Failed to load directory:', e);
    } finally {
      loadingChildren = false;
    }
  }

  async function handleClick() {
    if (isFolder) {
      // Load children if expanding and not yet loaded
      if (!expanded && !childrenLoaded) {
        await loadChildren();
      }
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
      'w-full flex items-center gap-1.5 text-left transition-colors duration-100 rounded-sm my-px py-[3px] pr-2 text-[13px]',
      isSelected ? 'bg-selected text-accent' : 'text-file hover:bg-hover'
    )}
    style="padding-left: {8 + depth * 12}px; width: 100%;"
  >
    <!-- Icon slot: chevron for folders, file icon for files -->
    <span class={cn('w-4 h-4 flex items-center justify-center flex-shrink-0', isFolder ? 'text-muted' : 'text-file')}>
      {#if isFolder}
        <ChevronIcon {expanded} size={12} />
      {:else}
        <FileIcon size={14} />
      {/if}
    </span>

    <!-- Name with truncation -->
    <span class={cn('truncate min-w-0', isSelected ? 'text-accent' : isFolder ? 'text-folder' : 'text-selected')}>
      {item.name}
    </span>
  </button>

  {#if isFolder && expanded}
    <div>
      {#if loadingChildren}
        <div class="text-xs text-muted py-1" style="padding-left: {20 + depth * 12}px;">
          Loading...
        </div>
      {:else if hasChildren}
        {#each item.children as child (child.path)}
          <FileTreeItem item={child} depth={depth + 1} />
        {/each}
      {/if}
    </div>
  {/if}
</div>
