<script lang="ts" module>
  import type { FileEntry } from '$lib/stores/files.svelte';
</script>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { FileIcon, ChevronIcon } from '$lib/components/icons';
  import { files } from '$lib/stores/files.svelte';
  import { cn } from '$lib/utils';
  import FileTreeItem from './FileTreeItem.svelte';

  interface Props {
    item: FileEntry;
    rootPath: string;
    depth?: number;
  }

  let { item, rootPath, depth = 0 }: Props = $props();

  let loadingChildren = $state(false);
  let childrenLoaded = $state(false);

  const isFolder = $derived(item.type === 'folder');
  const isSelected = $derived(files.selected === item.path);
  const hasChildren = $derived(item.children && item.children.length > 0);
  const expanded = $derived(files.expanded.has(item.path));

  async function loadChildren() {
    if (childrenLoaded || loadingChildren) return;

    loadingChildren = true;
    try {
      const entries = await invoke<FileEntry[]>('list_directory', { path: item.path, rootPath });
      files.updateChildren(item.path, entries);
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
      files.toggleExpanded(item.path);
    } else {
      files.setSelected(item.path);
    }
  }
</script>

<div>
  <button
    type="button"
    onclick={handleClick}
    class={cn(
      'w-full flex items-center gap-1.5 text-left transition-colors duration-100 rounded-sm my-px py-[3px] pr-2 text-[13px]',
      isSelected ? 'bg-selected text-primary' : 'text-file hover:bg-hover'
    )}
    style="padding-left: {8 + depth * 12}px; width: 100%;"
  >
    <!-- Icon slot: chevron for folders, file icon for files -->
    <span class={cn('w-4 h-4 flex items-center justify-center flex-shrink-0', isFolder ? 'text-muted-foreground' : 'text-file')}>
      {#if isFolder}
        <ChevronIcon {expanded} size={12} />
      {:else}
        <FileIcon size={14} />
      {/if}
    </span>

    <!-- Name with truncation -->
    <span class={cn('truncate min-w-0', isSelected ? 'text-primary' : isFolder ? 'text-folder' : 'text-selected')}>
      {item.name}
    </span>
  </button>

  {#if isFolder && expanded}
    <div>
      {#if loadingChildren}
        <div class="text-xs text-muted-foreground py-1" style="padding-left: {20 + depth * 12}px;">
          Loading...
        </div>
      {:else if hasChildren}
        {#each item.children as child (child.path)}
          <FileTreeItem item={child} {rootPath} depth={depth + 1} />
        {/each}
      {/if}
    </div>
  {/if}
</div>
