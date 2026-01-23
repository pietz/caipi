<script lang="ts">
  import {
    FileText,
    Pencil,
    Search,
    Terminal,
    Check,
    CircleAlert,
    Loader,
    X,
    Globe,
    Download,
    Sparkles,
    MessageCircle,
    ListTodo,
    BookOpen,
    Square,
  } from 'lucide-svelte';
  import type { ToolActivity, PermissionRequest } from '$lib/stores';

  interface Props {
    activity: ToolActivity;
    pendingPermissions?: Record<string, PermissionRequest>;
    onPermissionResponse?: (allowed: boolean) => void;
  }

  let {
    activity,
    pendingPermissions = {},
    onPermissionResponse,
  }: Props = $props();

  const isAwaitingPermission = $derived(
    pendingPermissions[activity.id] !== undefined
  );

  const toolIcons: Record<string, typeof FileText> = {
    Read: FileText,
    Write: Pencil,
    Edit: Pencil,
    Glob: Search,
    Grep: Search,
    Bash: Terminal,
    WebSearch: Globe,
    WebFetch: Download,
    Skill: Sparkles,
    Task: ListTodo,
    AskUserQuestion: MessageCircle,
    NotebookEdit: BookOpen,
  };

  const ToolIcon = $derived(toolIcons[activity.toolType] ?? Terminal);
</script>

<div
  class="flex items-center gap-2 p-2 rounded-md text-sm transition-colors"
  style:background-color={isAwaitingPermission ? 'rgba(234, 179, 8, 0.1)' : 'var(--muted)'}
  style:border="1px solid {isAwaitingPermission ? 'rgba(234, 179, 8, 0.4)' : 'var(--border-hover)'}"
>
  <!-- Tool Icon -->
  <ToolIcon class="w-4 h-4 flex-shrink-0 text-muted-foreground" />

  <!-- Target -->
  <span class="flex-1 min-w-0 truncate text-muted-foreground">
    {activity.target}
  </span>

  <!-- Status Icon -->
  <div class="w-4 h-4 flex-shrink-0">
    {#if !isAwaitingPermission && activity.status === 'running'}
      <Loader class="w-4 h-4 animate-spin text-muted-foreground" />
    {:else if !isAwaitingPermission && activity.status === 'completed'}
      <Check class="w-4 h-4 text-green-500" />
    {:else if !isAwaitingPermission && activity.status === 'error'}
      <CircleAlert class="w-4 h-4 text-destructive" />
    {:else if !isAwaitingPermission && activity.status === 'aborted'}
      <Square class="w-4 h-4 text-orange-500" />
    {/if}
  </div>

  <!-- Permission Buttons -->
  {#if isAwaitingPermission && onPermissionResponse}
    <div class="flex items-center gap-1">
      <button
        type="button"
        onclick={() => onPermissionResponse(true)}
        class="w-7 h-7 flex items-center justify-center rounded bg-green-500/20 hover:bg-green-500/30 text-green-500 transition-colors"
        title="Allow (Enter)"
      >
        <Check class="w-4 h-4" />
      </button>
      <button
        type="button"
        onclick={() => onPermissionResponse(false)}
        class="w-7 h-7 flex items-center justify-center rounded bg-red-500/20 hover:bg-red-500/30 text-red-500 transition-colors"
        title="Deny (Esc)"
      >
        <X class="w-4 h-4" />
      </button>
    </div>
  {/if}
</div>
