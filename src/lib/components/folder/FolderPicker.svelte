<script lang="ts">
  import { api, type RecentFolder as ApiRecentFolder } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Folder, Loader2, X } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import { app } from '$lib/stores/app.svelte';

  interface Props {
    showClose?: boolean;
  }

  let { showClose = false }: Props = $props();

  interface RecentFolder {
    path: string;
    name: string;
    timestamp: number;
  }

  let recentFolders = $state<RecentFolder[]>([]);
  let validating = $state(false);
  let error = $state<string | null>(null);
  let dragOver = $state(false);
  let dropZoneHover = $state(false);
  let hoveredFolder = $state<string | null>(null);

  async function loadRecentFolders() {
    try {
      const folders = await api.getRecentFolders();
      // Map API response to local format
      recentFolders = folders.map(f => ({
        path: f.path,
        name: f.path.split('/').pop() || f.path,
        timestamp: new Date(f.lastUsed).getTime() / 1000,
      }));
    } catch (e) {
      console.error('Failed to load recent folders:', e);
    }
  }

  async function selectFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select a project folder',
      });

      if (selected && typeof selected === 'string') {
        await validateAndProceed(selected);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to open folder picker';
    }
  }

  async function validateAndProceed(path: string) {
    validating = true;
    error = null;

    try {
      const valid = await api.validateFolder(path);

      if (!valid) {
        error = 'Cannot access this folder. Please choose another.';
        return;
      }

      // Save to recent folders
      await api.saveRecentFolder(path);

      // Start session
      await app.startSession(path);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to start session';
    } finally {
      validating = false;
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    dragOver = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;

    const items = e.dataTransfer?.items;
    if (!items || items.length === 0) return;

    const item = items[0];
    if (item.kind !== 'file') return;

    const entry = item.webkitGetAsEntry?.();
    if (!entry?.isDirectory) {
      error = 'Please drop a folder, not a file';
      return;
    }

    error = 'Drag and drop is not fully supported. Please use the browse button.';
  }

  function formatTime(timestamp: number): string {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 60000) return 'Just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    if (diff < 604800000) return `${Math.floor(diff / 86400000)}d ago`;
    return date.toLocaleDateString();
  }

  function goBackToChat() {
    app.setScreen('chat');
  }

  // Load recent folders on mount
  $effect(() => {
    loadRecentFolders();
  });
</script>

<div class="flex flex-col items-center justify-center h-full pt-12 px-8 pb-8 relative" data-tauri-drag-region>
  <!-- Top right controls -->
  <div class="absolute top-3 right-4 flex items-center gap-1">
    {#if showClose}
      <Button
        variant="ghost"
        size="icon"
        class="h-8 w-8 text-muted-foreground"
        onclick={goBackToChat}
      >
        <X size={16} />
      </Button>
    {/if}
  </div>

  <div class="w-full max-w-lg">
    <!-- Header -->
    <div class="mb-6">
      <h2 class="text-sm font-semibold text-foreground mb-1">
        Open a Project
      </h2>
      <p class="text-xs text-muted-foreground">
        Select a folder to start working with Claude
      </p>
    </div>

    <!-- Drop Zone -->
    <button
      type="button"
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
      ondrop={handleDrop}
      onclick={selectFolder}
      onmouseenter={() => dropZoneHover = true}
      onmouseleave={() => dropZoneHover = false}
      class="w-full rounded-lg p-8 flex flex-col items-center cursor-pointer mb-6 transition-all duration-150 border border-dashed {dragOver ? 'border-primary bg-primary/5' : dropZoneHover ? 'border-muted-foreground bg-muted/50' : 'border-border'}"
      disabled={validating}
    >
      {#if validating}
        <div class="text-muted-foreground mb-2">
          <Loader2 size={32} class="animate-spin" />
        </div>
        <p class="text-xs text-muted-foreground">Validating folder...</p>
      {:else}
        <div class="text-muted-foreground mb-2">
          <Folder size={32} />
        </div>
        <p class="text-xs text-muted-foreground mb-1">
          Drop a folder here or click to browse
        </p>
        <p class="text-xs text-muted-foreground/50">
          <span class="opacity-70">&#8984;O</span> to open folder
        </p>
      {/if}
    </button>

    {#if error}
      <div class="text-xs text-red-500 text-center mb-4">{error}</div>
    {/if}

    <!-- Recent Projects -->
    {#if recentFolders.length > 0}
      <div>
        <div class="text-xs uppercase tracking-widest font-semibold mb-3 text-muted-foreground/50">
          Recent Projects
        </div>
        <div class="flex flex-col gap-0.5">
          {#each recentFolders as folder}
            <button
              type="button"
              class="flex items-center justify-between py-2.5 px-3 rounded-md cursor-pointer transition-colors text-left w-full hover:bg-muted/50"
              onclick={() => validateAndProceed(folder.path)}
              disabled={validating}
            >
              <div class="flex items-center gap-2.5">
                <span class="text-muted-foreground">
                  <Folder size={16} />
                </span>
                <div>
                  <div class="text-sm text-foreground">{folder.name}</div>
                  <div class="text-xs text-muted-foreground/70">{folder.path}</div>
                </div>
              </div>
              <span class="text-xs text-muted-foreground/50">{formatTime(folder.timestamp)}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
