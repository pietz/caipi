<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { FolderOpen, Clock, ChevronRight, Upload } from 'lucide-svelte';
  import { Button, Card, Spinner } from '$lib/components/ui';
  import { appStore } from '$lib/stores';

  interface RecentFolder {
    path: string;
    name: string;
    timestamp: number;
  }

  let recentFolders = $state<RecentFolder[]>([]);
  let loading = $state(false);
  let validating = $state(false);
  let error = $state<string | null>(null);
  let dragOver = $state(false);

  async function loadRecentFolders() {
    try {
      recentFolders = await invoke<RecentFolder[]>('get_recent_folders');
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
      const valid = await invoke<boolean>('validate_folder', { path });

      if (!valid) {
        error = 'Cannot access this folder. Please choose another.';
        return;
      }

      // Save to recent folders
      await invoke('save_recent_folder', { path });

      // Update app state
      appStore.setSelectedFolder(path);

      // Create a new session
      const sessionId = await invoke<string>('create_session', { folderPath: path });
      appStore.setSessionId(sessionId);

      // Navigate to chat
      appStore.setScreen('chat');
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to validate folder';
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

    // Get the first item
    const item = items[0];
    if (item.kind !== 'file') return;

    const entry = item.webkitGetAsEntry?.();
    if (!entry?.isDirectory) {
      error = 'Please drop a folder, not a file';
      return;
    }

    // Unfortunately, we can't get the full path from a drag-drop in a secure context
    // So we'll need to use the native dialog
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

  // Load recent folders on mount
  $effect(() => {
    loadRecentFolders();
  });
</script>

<div class="flex flex-col items-center justify-center h-full p-8">
  <div class="max-w-lg w-full space-y-6">
    <!-- Header -->
    <div class="text-center">
      <h1 class="text-2xl font-bold">Select a Project</h1>
      <p class="text-muted-foreground mt-2">
        Choose a folder to work with Claude
      </p>
    </div>

    <!-- Drop Zone -->
    <div
      role="button"
      tabindex="0"
      class="relative border-2 border-dashed rounded-lg p-12 text-center transition-colors cursor-pointer
        {dragOver ? 'border-primary bg-primary/5' : 'border-muted-foreground/25 hover:border-muted-foreground/50'}"
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
      ondrop={handleDrop}
      onclick={selectFolder}
      onkeydown={(e) => e.key === 'Enter' && selectFolder()}
    >
      {#if validating}
        <Spinner size="lg" class="mx-auto mb-4" />
        <p class="text-muted-foreground">Validating folder...</p>
      {:else}
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-muted mb-4">
          <Upload class="w-8 h-8 text-muted-foreground" />
        </div>
        <p class="text-lg font-medium">
          Drop a folder here
        </p>
        <p class="text-muted-foreground mt-1">
          or click to browse
        </p>
      {/if}
    </div>

    {#if error}
      <div class="text-destructive text-sm text-center">{error}</div>
    {/if}

    <!-- Recent Folders -->
    {#if recentFolders.length > 0}
      <div class="space-y-2">
        <h2 class="text-sm font-medium text-muted-foreground flex items-center gap-2">
          <Clock class="w-4 h-4" />
          Recent Projects
        </h2>
        <Card class="divide-y divide-border">
          {#each recentFolders as folder}
            <button
              class="w-full flex items-center gap-3 p-3 hover:bg-muted/50 transition-colors text-left"
              onclick={() => validateAndProceed(folder.path)}
              disabled={validating}
            >
              <FolderOpen class="w-5 h-5 text-muted-foreground flex-shrink-0" />
              <div class="flex-1 min-w-0">
                <div class="font-medium truncate">{folder.name}</div>
                <div class="text-xs text-muted-foreground truncate">{folder.path}</div>
              </div>
              <div class="text-xs text-muted-foreground flex-shrink-0">
                {formatTime(folder.timestamp)}
              </div>
              <ChevronRight class="w-4 h-4 text-muted-foreground flex-shrink-0" />
            </button>
          {/each}
        </Card>
      </div>
    {/if}

    <!-- Back Button -->
    <div class="text-center">
      <Button
        variant="ghost"
        onclick={() => appStore.setScreen('onboarding')}
      >
        Back to Setup
      </Button>
    </div>
  </div>
</div>
