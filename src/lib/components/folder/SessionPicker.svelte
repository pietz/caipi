<script lang="ts">
  import { api, type ProjectSessions, type SessionInfo } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Folder, Loader2, X, ChevronRight, ChevronDown, Plus } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import { app } from '$lib/stores/app.svelte';
  import { chat } from '$lib/stores/chat.svelte';
  import { resetEventState } from '$lib/utils/events';
  import type { Backend } from '$lib/config/backends';
  import { SvelteSet } from 'svelte/reactivity';
  import { onMount } from 'svelte';

  interface Props {
    showClose?: boolean;
  }

  let { showClose = false }: Props = $props();

  let projects = $state<ProjectSessions[]>([]);
  let expandedFolders = $state(new SvelteSet<string>());
  let loading = $state(true);
  let validating = $state(false);
  let error = $state<string | null>(null);

  async function loadSessions() {
    try {
      loading = true;
      // Backend handles: filtering non-existent folders, sorting, limiting to 50
      projects = await api.getRecentSessions(50, app.defaultBackend);
      expandedFolders = new SvelteSet();
    } catch (e) {
      console.error('Failed to load sessions:', e);
      error = 'Failed to load sessions';
    } finally {
      loading = false;
    }
  }

  function toggleFolder(folderPath: string) {
    const newSet = new SvelteSet(expandedFolders);
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
      // Sessions are backend-specific; resume with the backend recorded on the session when present.
      const resumeBackend =
        session.backend === 'claude' || session.backend === 'codex'
          ? (session.backend as Backend)
          : undefined;
      await app.resumeSession(
        session.folderPath,
        session.sessionId,
        resumeBackend
      );
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
  onMount(() => {
    loadSessions();
  });
</script>

<div class="flex flex-col h-full pt-9 px-4 pb-8 relative" data-tauri-drag-region>
  <!-- Top right controls - matches ChatContainer titlebar positioning -->
  <div class="absolute top-1.5 right-4 flex items-center gap-1">
    <Button
      variant="ghost"
      size="icon"
      class="h-6 w-6 text-muted-foreground"
      onclick={() => void api.createWindow()}
    >
      <Plus size={14} />
    </Button>
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
    <div class="mb-6">
      <h2 class="text-sm font-semibold text-foreground mb-1">
        Recent Sessions
      </h2>
      <p class="text-xs text-muted-foreground">
        Resume a previous conversation or start a new one
      </p>
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
                      <div class="flex items-center gap-2 flex-shrink-0">
                        {#if session.backend}
                          <span class="text-[10px] px-1.5 py-0.5 rounded border border-border/60 text-muted-foreground/70">
                            {session.backend === 'claude' ? 'Claude' : session.backend === 'codex' ? 'Codex' : session.backend}
                          </span>
                        {/if}
                        <span class="text-xs text-muted-foreground/50">
                          {formatTime(session.modified)}
                        </span>
                      </div>
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
