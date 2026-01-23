<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { get } from 'svelte/store';
  import { open } from '@tauri-apps/plugin-dialog';
  import { FolderIcon, SpinnerIcon, SunIcon, MoonIcon, CloseIcon } from '$lib/components/icons';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import { appStore } from '$lib/stores';

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
  let loading = $state(false);
  let validating = $state(false);
  let error = $state<string | null>(null);
  let dragOver = $state(false);
  let dropZoneHover = $state(false);
  let hoveredFolder = $state<string | null>(null);
  let currentTheme = $state<'light' | 'dark'>('dark');

  // Subscribe to resolved theme
  resolvedTheme.subscribe((theme) => {
    currentTheme = theme;
  });

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

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

      // Get current settings
      const { permissionMode, model } = get(appStore);

      // Create a new session with permission mode and model
      const sessionId = await invoke<string>('create_session', {
        folderPath: path,
        permissionMode,
        model
      });
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
    appStore.setScreen('chat');
  }

  // Load recent folders on mount
  $effect(() => {
    loadRecentFolders();
  });
</script>

<div class="flex flex-col items-center justify-center h-full pt-12 px-8 pb-8 relative" data-tauri-drag-region>
  <!-- Top right controls -->
  <div class="absolute top-3 right-4 flex items-center gap-2">
    <button
      type="button"
      onclick={toggleTheme}
      class="p-1 rounded transition-all duration-100 text-dim hover:bg-hover hover:text-secondary"
      title={currentTheme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
    >
      {#if currentTheme === 'dark'}
        <SunIcon size={16} />
      {:else}
        <MoonIcon size={16} />
      {/if}
    </button>
    {#if showClose}
      <button
        type="button"
        onclick={goBackToChat}
        class="p-1 rounded transition-all duration-100 text-dim hover:bg-hover hover:text-secondary"
        title="Back to chat"
      >
        <CloseIcon size={16} />
      </button>
    {:else}
      <span class="text-xs text-darkest">v0.1.0</span>
    {/if}
  </div>

  <div class="w-full max-w-lg">
  <!-- Header -->
  <div class="mb-6">
    <h2 class="text-sm font-semibold text-primary mb-1">
      Open a Project
    </h2>
    <p class="text-xs text-muted">
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
    class="w-full rounded-lg p-8 flex flex-col items-center cursor-pointer mb-6 transition-all duration-150"
    style="
      border: 1px dashed {dragOver ? 'var(--accent-blue)' : dropZoneHover ? 'var(--text-dim)' : 'var(--border-hover)'};
      background-color: {dragOver ? 'rgba(59, 130, 246, 0.05)' : dropZoneHover ? 'var(--hover)' : 'transparent'};
    "
    disabled={validating}
  >
    {#if validating}
      <div class="text-dim mb-2">
        <SpinnerIcon size={32} />
      </div>
      <p class="text-xs text-secondary">Validating folder...</p>
    {:else}
      <div class="text-dim mb-2">
        <FolderIcon size={32} />
      </div>
      <p class="text-xs text-secondary mb-1">
        Drop a folder here or click to browse
      </p>
      <p class="text-xs text-dim">
        <span class="opacity-70">⌘O</span> to open folder
      </p>
    {/if}
  </button>

  {#if error}
    <div class="text-xs text-red-500 text-center mb-4">{error}</div>
  {/if}

  <!-- Recent Projects -->
  {#if recentFolders.length > 0}
    <div>
      <div class="text-xs font-medium text-dim uppercase tracking-[0.5px] mb-2">
        Recent Projects
      </div>
      <div class="flex flex-col gap-0.5">
        {#each recentFolders as folder}
          <button
            type="button"
            class="flex items-center justify-between py-2.5 px-3 rounded-md cursor-pointer transition-colors duration-100 text-left w-full"
            style="background-color: {hoveredFolder === folder.path ? 'var(--hover)' : 'transparent'};"
            onmouseenter={() => hoveredFolder = folder.path}
            onmouseleave={() => hoveredFolder = null}
            onclick={() => validateAndProceed(folder.path)}
            disabled={validating}
          >
            <div class="flex items-center gap-2.5">
              <span class="text-folder">
                <FolderIcon size={16} />
              </span>
              <div>
                <div class="text-sm text-primary">{folder.name}</div>
                <div class="text-xs text-dim">{folder.path}</div>
              </div>
            </div>
            <span class="text-xs text-dim">{formatTime(folder.timestamp)}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
  </div>
</div>
