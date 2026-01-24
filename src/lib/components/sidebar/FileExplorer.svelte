<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { watchImmediate, type UnwatchFn } from '@tauri-apps/plugin-fs';
  import FileTreeItem from './FileTreeItem.svelte';
  import { files, type FileEntry } from '$lib/stores/files.svelte';

  interface Props {
    rootPath: string;
  }

  let { rootPath }: Props = $props();

  let refreshTimeout: ReturnType<typeof setTimeout> | null = null;

  async function loadDirectory(path: string) {
    files.setLoading(true);
    try {
      const entries = await invoke<FileEntry[]>('list_directory', { path, rootPath });
      files.setTree(entries);
      files.setRootPath(path);
    } catch (e) {
      console.error('Failed to load directory:', e);
      files.setError(e instanceof Error ? e.message : 'Failed to load directory');
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
  <!-- Header -->
  <div class="p-3 pb-0">
    <div class="text-xs font-medium text-muted uppercase tracking-[0.5px]">
      Explorer
    </div>
  </div>

  <!-- Tree -->
  <div class="flex-1 overflow-auto pt-2">
    {#if files.loading}
      <div class="p-3 text-xs text-muted">Loading...</div>
    {:else if files.tree.length === 0}
      <div class="p-3 text-xs text-muted">No files</div>
    {:else}
      {#each files.tree as item (item.path)}
        <FileTreeItem {item} {rootPath} />
      {/each}
    {/if}
  </div>

  <!-- Footer -->
  <div
    class="py-2 px-3 text-xs text-dim"
    style="border-top: 1px solid var(--border);"
  >
    <span class="opacity-70">&#8984;&#8679;O</span> Quick open
  </div>
</div>
