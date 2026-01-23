<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { watchImmediate, type UnwatchFn } from '@tauri-apps/plugin-fs';
  import FileTreeItem from './FileTreeItem.svelte';
  import { filesStore, type FileEntry } from '$lib/stores';

  interface Props {
    rootPath: string;
  }

  let { rootPath }: Props = $props();

  let refreshTimeout: ReturnType<typeof setTimeout> | null = null;

  const tree = $derived($filesStore.tree);
  const loading = $derived($filesStore.loading);

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

  // Initial load when rootPath changes
  $effect(() => {
    if (rootPath) {
      loadDirectory(rootPath);
    }
  });

  // Watch for file system changes
  $effect(() => {
    if (!rootPath) return;

    let stopWatcher: UnwatchFn | null = null;

    watchImmediate(
      rootPath,
      () => {
        // Debounce rapid changes
        if (refreshTimeout) clearTimeout(refreshTimeout);
        refreshTimeout = setTimeout(() => loadDirectory(rootPath), 300);
      },
      { recursive: true }
    )
      .then((unwatch) => {
        stopWatcher = unwatch;
      })
      .catch((err) => {
        console.error('Failed to start file watcher:', err);
      });

    // Cleanup on rootPath change or unmount
    return () => {
      stopWatcher?.();
      if (refreshTimeout) clearTimeout(refreshTimeout);
    };
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
