<script lang="ts">
  import { api, type ProjectSessions, type SessionInfo } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Folder, Loader2, Sun, Moon, X, ChevronRight, ChevronDown, Plus } from 'lucide-svelte';
  import { Button } from '$lib/components/ui';
  import { themeStore, resolvedTheme } from '$lib/stores/theme';
  import { app } from '$lib/stores/app.svelte';

  interface Props {
    showClose?: boolean;
  }

  let { showClose = false }: Props = $props();

  let projects = $state<ProjectSessions[]>([]);
  let expandedFolders = $state<Set<string>>(new Set());
  let loading = $state(true);
  let validating = $state(false);
  let error = $state<string | null>(null);

  const currentTheme = $derived($resolvedTheme);

  function toggleTheme() {
    themeStore.setPreference(currentTheme === 'dark' ? 'light' : 'dark');
  }

  async function loadSessions() {
    try {
      loading = true;
      const allProjects = await api.getAllSessions();

      // Flatten all sessions with their project info, take top 50, regroup
      const allSessions = allProjects.flatMap(p =>
        p.sessions.map(s => ({ ...s, project: p }))
      );

      // Sort by modified (most recent first) and take top 100
      allSessions.sort((a, b) => b.modified.localeCompare(a.modified));
      const top100 = allSessions.slice(0, 100);

      // Regroup by project, preserving the order of first appearance
      const projectMap = new Map<string, ProjectSessions>();
      for (const session of top100) {
        const key = session.project.folderPath;
        if (!projectMap.has(key)) {
          projectMap.set(key, {
            folderPath: session.project.folderPath,
            folderName: session.project.folderName,
            sessions: [],
            latestModified: session.modified,
          });
        }
        projectMap.get(key)!.sessions.push(session);
      }

      projects = Array.from(projectMap.values());

      // Accordions closed by default
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
    validating = true;
    error = null;

    try {
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
    <Button
      variant="ghost"
      size="icon"
      class="h-6 w-6 text-muted-foreground"
      onclick={toggleTheme}
    >
      {#if currentTheme === 'dark'}
        <Sun size={14} />
      {:else}
        <Moon size={14} />
      {/if}
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
    {:else}
      <span class="text-xs text-muted-foreground/50 ml-2">v0.1.0</span>
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
                <button
                  type="button"
                  class="flex items-center justify-between w-full py-2.5 px-3 hover:bg-muted/50 transition-colors text-left rounded-md"
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
                  </div>
                  <span class="text-xs text-muted-foreground/50">
                    {project.sessions.length} session{project.sessions.length !== 1 ? 's' : ''}
                  </span>
                </button>

                <!-- Sessions list (collapsible) -->
                {#if isExpanded}
                  <div class="ml-6 border-l border-border/50">
                    {#each project.sessions as session}
                      <button
                        type="button"
                        class="flex items-center justify-between w-full py-2 px-3 hover:bg-muted/50 transition-colors text-left"
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
          class="flex items-center gap-2 w-full py-2.5 px-3 rounded-md hover:bg-muted/50 transition-colors text-left"
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
