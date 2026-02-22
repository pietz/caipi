<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { watchImmediate, type UnwatchFn } from '@tauri-apps/plugin-fs';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { Bug } from 'lucide-svelte';
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

    let cancelled = false;
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
        if (cancelled) {
          // Already unmounted, stop immediately
          unwatch();
        } else {
          stopWatcher = unwatch;
        }
      })
      .catch((err) => {
        console.error('Failed to start file watcher:', err);
      });

    // Cleanup on rootPath change or unmount
    return () => {
      cancelled = true;
      stopWatcher?.();
      if (refreshTimeout) clearTimeout(refreshTimeout);
    };
  });
</script>

<div class="flex flex-col h-full">
  <!-- Header -->
  <div class="p-3 pb-0">
    <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
      Files
    </div>
  </div>

  <!-- Tree -->
  <div class="flex-1 overflow-auto">
    {#if files.loading}
      <div class="p-3 text-xs text-muted-foreground">Loading...</div>
    {:else if files.tree.length === 0}
      <div class="p-3 text-xs text-muted-foreground">No files</div>
    {:else}
      {#each files.tree as item (item.path)}
        <FileTreeItem {item} {rootPath} />
      {/each}
    {/if}
  </div>

  <!-- Footer -->
  <button
    class="flex items-center gap-2 px-3 py-2 text-xs text-muted-foreground/50 hover:text-muted-foreground transition-colors cursor-pointer border-t border-border"
    onclick={() => openUrl('https://github.com/pietz/caipi/issues')}
  >
    <Bug size={12} />
    <span>Report an issue</span>
  </button>
</div>
