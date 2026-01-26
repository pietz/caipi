<script lang="ts">
  import {
    Eye,
    Pencil,
    Search,
    Terminal,
    Check,
    Loader2,
    X,
    Globe,
    Download,
    Sparkles,
    MessageCircle,
    ListTodo,
    BookOpen,
    Ban,
    AlertCircle,
    Clock,
    Brain,
  } from 'lucide-svelte';
  import type { ToolState } from '$lib/stores';

  interface Props {
    tool: ToolState;
    onPermissionResponse?: (allowed: boolean) => void;
  }

  let {
    tool,
    onPermissionResponse,
  }: Props = $props();

  const isAwaitingPermission = $derived(tool.status === 'awaiting_permission');

  // Tool configurations with icons and colors
  type ToolConfig = {
    icon: typeof Eye;
    className: string;
    label: string;
  };

  const toolConfigs: Record<string, ToolConfig> = {
    // Read operations - blue
    Read: { icon: Eye, className: 'bg-blue-500/10 border-blue-500/20 text-blue-500', label: 'view' },
    Glob: { icon: Search, className: 'bg-blue-500/10 border-blue-500/20 text-blue-500', label: 'glob' },
    Grep: { icon: Search, className: 'bg-blue-500/10 border-blue-500/20 text-blue-500', label: 'grep' },
    WebFetch: { icon: Download, className: 'bg-blue-500/10 border-blue-500/20 text-blue-500', label: 'fetch' },

    // Write operations - amber
    Write: { icon: Pencil, className: 'bg-amber-500/10 border-amber-500/20 text-amber-500', label: 'create' },
    Edit: { icon: Pencil, className: 'bg-amber-500/10 border-amber-500/20 text-amber-500', label: 'edit' },
    NotebookEdit: { icon: BookOpen, className: 'bg-amber-500/10 border-amber-500/20 text-amber-500', label: 'notebook' },

    // Terminal operations - purple
    Bash: { icon: Terminal, className: 'bg-purple-500/10 border-purple-500/20 text-purple-500', label: 'bash' },

    // Search/web operations - emerald
    WebSearch: { icon: Globe, className: 'bg-emerald-500/10 border-emerald-500/20 text-emerald-500', label: 'search' },

    // Other operations
    Skill: { icon: Sparkles, className: 'bg-purple-500/10 border-purple-500/20 text-purple-500', label: 'skill' },
    Task: { icon: ListTodo, className: 'bg-purple-500/10 border-purple-500/20 text-purple-500', label: 'task' },
    AskUserQuestion: { icon: MessageCircle, className: 'bg-blue-500/10 border-blue-500/20 text-blue-500', label: 'ask' },

    // Thinking
    Thinking: { icon: Brain, className: 'bg-purple-500/10 border-purple-500/20 text-purple-500', label: 'thinking' },
  };

  const defaultConfig: ToolConfig = {
    icon: Terminal,
    className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
    label: 'tool'
  };

  const config = $derived(toolConfigs[tool.toolType] ?? defaultConfig);
  const ToolIcon = $derived(config.icon);
</script>

<div class="flex items-center justify-between rounded-lg border px-3 h-10 my-2 {config.className}">
  <div class="flex items-center gap-2 min-w-0">
    <ToolIcon size={14} />
    <span class="text-xs font-medium uppercase tracking-wide opacity-70">{config.label}</span>
    <span class="text-xs text-muted-foreground truncate">{tool.target}</span>
  </div>

  <div class="flex items-center gap-1.5 h-6">
    {#if tool.status === 'completed'}
      <Check size={14} class="text-green-500" />
    {:else if tool.status === 'error'}
      <AlertCircle size={14} class="text-red-500" />
    {:else if tool.status === 'aborted'}
      <Ban size={14} class="text-muted-foreground" />
    {:else if tool.status === 'denied'}
      <X size={14} class="text-red-500" />
    {:else if tool.status === 'pending'}
      <Clock size={14} class="text-muted-foreground animate-pulse" />
    {:else if tool.status === 'running'}
      <Loader2 size={14} class="animate-spin" />
    {:else if isAwaitingPermission && onPermissionResponse}
      <button
        type="button"
        class="h-6 w-6 rounded-md flex items-center justify-center bg-green-500/15 hover:bg-green-500/25 text-green-500 transition-colors"
        onclick={() => onPermissionResponse(true)}
        title="Allow (Enter)"
      >
        <Check size={14} />
      </button>
      <button
        type="button"
        class="h-6 w-6 rounded-md flex items-center justify-center bg-red-500/15 hover:bg-red-500/25 text-red-500 transition-colors"
        onclick={() => onPermissionResponse(false)}
        title="Deny (Esc)"
      >
        <X size={14} />
      </button>
    {/if}
  </div>
</div>
