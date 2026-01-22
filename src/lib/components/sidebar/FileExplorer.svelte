<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import FileTreeItem from './FileTreeItem.svelte';
  import { filesStore, type FileEntry } from '$lib/stores';

  interface Props {
    rootPath: string;
  }

  let { rootPath }: Props = $props();

  let tree = $state<FileEntry[]>([]);
  let loading = $state(false);

  filesStore.subscribe((state) => {
    tree = state.tree;
    loading = state.loading;
  });

  async function loadDirectory(path: string) {
    filesStore.setLoading(true);
    try {
      const entries = await invoke<FileEntry[]>('list_directory', { path });
      filesStore.setTree(entries);
      filesStore.setRootPath(path);
    } catch (e) {
      console.error('Failed to load directory:', e);
      filesStore.setError(e instanceof Error ? e.message : 'Failed to load directory');
    }
  }

  $effect(() => {
    if (rootPath) {
      loadDirectory(rootPath);
    }
  });
</script>

<div class="flex flex-col h-full" style="background-color: var(--sidebar);">
  <!-- Tree -->
  <div class="flex-1 overflow-auto pt-2">
    {#if loading}
      <div class="p-3 text-xs text-muted">Loading...</div>
    {:else if tree.length === 0}
      <div class="p-3 text-xs text-muted">No files</div>
    {:else}
      {#each tree as item (item.path)}
        <FileTreeItem {item} />
      {/each}
    {/if}
  </div>

  <!-- Footer -->
  <div
    class="py-2 px-3 text-xs text-dim"
    style="border-top: 1px solid var(--border);"
  >
    <span class="opacity-70">⌘⇧O</span> Quick open
  </div>
</div>
