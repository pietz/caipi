<script lang="ts">
  import { api, type ProjectSessions, type SessionInfo, type BackendStatus } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Folder, Loader2, X, ChevronRight, ChevronDown, Plus } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import { app, type Backend } from '$lib/stores/app.svelte';
  import { chat } from '$lib/stores/chat.svelte';
  import { resetEventState } from '$lib/utils/events';
  import { onMount } from 'svelte';

  interface Props {
    showClose?: boolean;
  }

  let { showClose = false }: Props = $props();

  let projects = $state<ProjectSessions[]>([]);
  let expandedFolders = $state<Set<string>>(new Set());
  let loading = $state(true);
  let validating = $state(false);
  let error = $state<string | null>(null);
  let backends = $state<BackendStatus[]>([]);
  let loadingBackends = $state(true);
  let selectedBackend = $state<Backend>(app.backend);

  // Backend display names
  const backendNames: Record<Backend, string> = {
    claude: 'Claude',
    codex: 'Codex',
  };

  onMount(async () => {
    // Load backend status
    try {
      backends = await api.checkBackendsStatus();
    } catch (e) {
      console.error('Failed to check backends:', e);
    } finally {
      loadingBackends = false;
    }
  });

  function isBackendAvailable(kind: Backend): boolean {
    const status = backends.find((b) => b.kind === kind);
    return !!status?.installed && !!status?.authenticated;
  }

  function selectBackend(kind: Backend) {
    if (!isBackendAvailable(kind)) return;
    selectedBackend = kind;
    // Reload sessions for new backend
    loadSessions();
  }

  async function loadSessions() {
    try {
      loading = true;
      // Backend handles: filtering non-existent folders, sorting, limiting to 100
      // Pass the selected backend to filter sessions
      projects = await api.getRecentSessions(100, selectedBackend);
      expandedFolders = new Set();
    } catch (e) {
      console.error('Failed to load sessions:', e);
      error = 'Failed to load sessions';
    } finally {
      loading = false;
    }
  }

  function toggleFolder(folderPath: string) {
    const newSet = new Set(expandedFolders);
    if (newSet.has(folderPath)) {
      newSet.delete(folderPath);
    } else {
      newSet.add(folderPath);
    }
    expandedFolders = newSet;
  }

  async function resumeSession(session: SessionInfo) {
    // If clicking on the currently active session, just return to chat
    if (app.sessionId === session.sessionId && app.folder === session.folderPath) {
      app.setScreen('chat');
      return;
    }

    validating = true;
    error = null;

    try {
      // Reset state before loading different session
      chat.reset();
      resetEventState();

      // Set the backend for this session (sessions are filtered by backend, so this matches)
      app.setBackend(selectedBackend);

      await app.resumeSession(session.folderPath, session.sessionId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to resume session';
    } finally {
      validating = false;
    }
  }

  async function selectNewFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select a project folder',
      });

      if (selected && typeof selected === 'string') {
        await startNewSession(selected);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to open folder picker';
    }
  }

  async function startNewSession(path: string) {
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

      // Reset state before starting new session
      chat.reset();
      resetEventState();

      // Set the backend for this session (might be different from default)
      app.setBackend(selectedBackend);

      // Start session
      await app.startSession(path);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to start session';
    } finally {
      validating = false;
    }
  }

  function formatTime(isoString: string): string {
    if (!isoString) return '';

    const date = new Date(isoString);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 60000) return 'Just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    if (diff < 604800000) return `${Math.floor(diff / 86400000)}d ago`;
    return date.toLocaleDateString();
  }

  function truncatePrompt(prompt: string, maxLength = 40): string {
    // Remove system tags like <ide_opened_file>
    let cleaned = prompt.replace(/<[^>]+>[^<]*<\/[^>]+>/g, '').trim();

    // If empty after cleaning, show placeholder
    if (!cleaned || cleaned === 'No prompt') {
      return 'Session started...';
    }

    // Truncate if too long
    if (cleaned.length > maxLength) {
      return cleaned.slice(0, maxLength) + '...';
    }

    return cleaned;
  }

  function goBackToChat() {
    app.setScreen('chat');
  }

  // Load sessions on mount
  $effect(() => {
    loadSessions();
  });
</script>

<div class="flex flex-col h-full pt-9 px-4 pb-8 relative" data-tauri-drag-region>
  <!-- Top right controls - matches ChatContainer titlebar positioning -->
  <div class="absolute top-1.5 right-4 flex items-center gap-1">
    {#if showClose}
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-muted-foreground"
        onclick={goBackToChat}
      >
        <X size={14} />
      </Button>
    {/if}
  </div>

  <div class="w-full max-w-lg mx-auto flex flex-col h-full">
    <!-- Header -->
    <div class="mb-4">
      <h2 class="text-sm font-semibold text-foreground mb-1">
        Recent Sessions
      </h2>
      <p class="text-xs text-muted-foreground">
        Resume a previous conversation or start a new one
      </p>
    </div>

    <!-- Backend Selector -->
    <div class="mb-4">
      <div class="flex gap-1 p-1 bg-muted rounded-lg">
        {#each ['claude', 'codex'] as kind}
          {@const isAvailable = isBackendAvailable(kind as Backend)}
          {@const isSelected = selectedBackend === kind}
          <button
            type="button"
            class="flex-1 flex items-center justify-center gap-2 px-3 py-1.5 text-xs rounded-md transition-colors {isSelected
              ? 'bg-background shadow-sm'
              : isAvailable
                ? 'hover:bg-background/50'
                : 'opacity-50 cursor-not-allowed'}"
            onclick={() => selectBackend(kind as Backend)}
            disabled={!isAvailable || loadingBackends}
          >
            {#if loadingBackends}
              <Loader2 size={12} class="animate-spin" />
            {/if}
            {backendNames[kind as Backend]}
            {#if !loadingBackends && !isAvailable}
              <span class="text-muted-foreground/50 text-[10px]">(unavailable)</span>
            {/if}
          </button>
        {/each}
      </div>
    </div>

    {#if error}
      <div class="text-xs text-red-500 text-center mb-4">{error}</div>
    {/if}

    {#if loading}
      <div class="flex items-center justify-center flex-1">
        <Loader2 size={24} class="animate-spin text-muted-foreground" />
      </div>
    {:else}
      <!-- Sessions list -->
      <div class="flex-1 overflow-y-auto -mx-2 px-2">
        {#if projects.length === 0}
          <div class="text-center text-muted-foreground text-sm py-8">
            No previous sessions found
          </div>
        {:else}
          <div class="flex flex-col gap-1">
            {#each projects as project}
              {@const isExpanded = expandedFolders.has(project.folderPath)}
              <div class="rounded-lg overflow-hidden">
                <!-- Folder header -->
                <div class="flex items-center w-full">
                  <button
                    type="button"
                    class="flex items-center flex-1 py-2.5 px-3 hover:bg-muted transition-colors text-left rounded-l-md"
                    onclick={() => toggleFolder(project.folderPath)}
                    disabled={validating}
                  >
                    <div class="flex items-center gap-2">
                      <span class="text-muted-foreground">
                        {#if isExpanded}
                          <ChevronDown size={16} />
                        {:else}
                          <ChevronRight size={16} />
                        {/if}
                      </span>
                      <span class="text-muted-foreground">
                        <Folder size={16} />
                      </span>
                      <span class="text-sm font-medium text-foreground">{project.folderName}</span>
                      <span class="text-xs text-muted-foreground/50">
                        ({project.sessions.length})
                      </span>
                    </div>
                  </button>
                  <button
                    type="button"
                    class="py-2.5 px-3 hover:bg-muted transition-colors rounded-r-md"
                    onclick={() => startNewSession(project.folderPath)}
                    disabled={validating}
                    title="New session"
                  >
                    <Plus size={16} class="text-muted-foreground" />
                  </button>
                </div>

                <!-- Sessions list (collapsible) -->
                {#if isExpanded}
                  <div class="ml-6 border-l border-border/50">
                    {#each project.sessions as session}
                      <button
                        type="button"
                        class="flex items-center justify-between w-full py-2 px-3 hover:bg-muted transition-colors text-left"
                        onclick={() => resumeSession(session)}
                        disabled={validating}
                      >
                        <div class="flex-1 min-w-0 pr-3">
                          <div class="text-sm text-foreground truncate">
                            {truncatePrompt(session.firstPrompt)}
                          </div>
                        </div>
                        <span class="text-xs text-muted-foreground/50 flex-shrink-0">
                          {formatTime(session.modified)}
                        </span>
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- New Session section -->
      <div class="mt-6 pt-4 border-t border-border">
        <button
          type="button"
          class="flex items-center gap-2 w-full py-2.5 px-3 rounded-md hover:bg-muted transition-colors text-left"
          onclick={selectNewFolder}
          disabled={validating}
        >
          {#if validating}
            <Loader2 size={16} class="animate-spin text-muted-foreground" />
            <span class="text-sm text-muted-foreground">Starting session...</span>
          {:else}
            <Plus size={16} class="text-muted-foreground" />
            <span class="text-sm text-foreground">New Session</span>
            <span class="text-xs text-muted-foreground/50 ml-auto">Select folder...</span>
          {/if}
        </button>
      </div>
    {/if}
  </div>
</div>
