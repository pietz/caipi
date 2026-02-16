<script lang="ts" module>
  import type { FileEntry } from '$lib/stores/files.svelte';
</script>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { ChevronDown, ChevronRight, Folder, File } from 'lucide-svelte';
  import { files } from '$lib/stores/files.svelte';
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

  async function handleDoubleClick() {
    console.log('Double click on:', item.path, 'isFolder:', isFolder);
    if (!isFolder) {
      try {
        console.log('Calling openPath with:', item.path);
        await openPath(item.path);
        console.log('openPath succeeded');
      } catch (e) {
        console.error('Failed to open file:', e);
      }
    }
  }
</script>

<div>
  <button
    type="button"
    onclick={handleClick}
    ondblclick={handleDoubleClick}
    class="grid grid-cols-[auto_auto_minmax(0,1fr)] items-center gap-1 py-1 px-2 rounded hover:bg-muted/50 transition-colors w-full text-left"
    style="padding-left: {depth * 12 + 8}px;"
  >
    {#if isFolder}
      <span class="text-muted-foreground w-3 flex-shrink-0">
        {#if expanded}
          <ChevronDown size={12} />
        {:else}
          <ChevronRight size={12} />
        {/if}
      </span>
    {:else}
      <span class="w-3"></span>
    {/if}
    <span class="text-muted-foreground flex-shrink-0">
      {#if isFolder}
        <Folder size={14} />
      {:else}
        <File size={14} />
      {/if}
    </span>
    <span class="text-xs text-foreground/80 min-w-0 truncate whitespace-nowrap overflow-hidden">
      {item.name}
    </span>
  </button>

  {#if isFolder && expanded}
    <div>
      {#if loadingChildren}
        <div class="text-xs text-muted-foreground py-1" style="padding-left: {depth * 12 + 20}px;">
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
