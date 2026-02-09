// Shared tool configurations for tool display components
import {
  Eye,
  Pencil,
  Search,
  Terminal,
  Globe,
  Download,
  Sparkles,
  MessageCircle,
  ListTodo,
  BookOpen,
  Brain,
} from 'lucide-svelte';
import type { ComponentType } from 'svelte';

export type ToolConfig = {
  icon: ComponentType;
  // Full class (colored background) - kept for future use
  className: string;
  // Text color only for stack icons
  iconColor: string;
  label: string;
};

export const toolConfigs: Record<string, ToolConfig> = {
  // Read operations - blue
  Read: {
    icon: Eye,
    className: 'bg-blue-500/10 border-blue-500/20 text-blue-500',
    iconColor: 'text-blue-500',
    label: 'view'
  },
  Glob: {
    icon: Search,
    className: 'bg-blue-500/10 border-blue-500/20 text-blue-500',
    iconColor: 'text-blue-500',
    label: 'glob'
  },
  Grep: {
    icon: Search,
    className: 'bg-blue-500/10 border-blue-500/20 text-blue-500',
    iconColor: 'text-blue-500',
    label: 'grep'
  },
  WebFetch: {
    icon: Download,
    className: 'bg-blue-500/10 border-blue-500/20 text-blue-500',
    iconColor: 'text-blue-500',
    label: 'fetch'
  },

  // Write operations - amber
  Write: {
    icon: Pencil,
    className: 'bg-amber-500/10 border-amber-500/20 text-amber-500',
    iconColor: 'text-amber-500',
    label: 'create'
  },
  Edit: {
    icon: Pencil,
    className: 'bg-amber-500/10 border-amber-500/20 text-amber-500',
    iconColor: 'text-amber-500',
    label: 'edit'
  },
  NotebookEdit: {
    icon: BookOpen,
    className: 'bg-amber-500/10 border-amber-500/20 text-amber-500',
    iconColor: 'text-amber-500',
    label: 'notebook'
  },

  // Terminal operations - purple
  Bash: {
    icon: Terminal,
    className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
    iconColor: 'text-purple-500',
    label: 'bash'
  },

  // Search/web operations - emerald
  WebSearch: {
    icon: Globe,
    className: 'bg-emerald-500/10 border-emerald-500/20 text-emerald-500',
    iconColor: 'text-emerald-500',
    label: 'search'
  },

  // Other operations
  Skill: {
    icon: Sparkles,
    className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
    iconColor: 'text-purple-500',
    label: 'skill'
  },
  Task: {
    icon: ListTodo,
    className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
    iconColor: 'text-purple-500',
    label: 'task'
  },
  AskUserQuestion: {
    icon: MessageCircle,
    className: 'bg-blue-500/10 border-blue-500/20 text-blue-500',
    iconColor: 'text-blue-500',
    label: 'ask'
  },

  // Codex CLI tool names
  command_execution: {
    icon: Terminal,
    className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
    iconColor: 'text-purple-500',
    label: 'bash'
  },
  web_search: {
    icon: Globe,
    className: 'bg-emerald-500/10 border-emerald-500/20 text-emerald-500',
    iconColor: 'text-emerald-500',
    label: 'search'
  },
  file_change: {
    icon: Pencil,
    className: 'bg-amber-500/10 border-amber-500/20 text-amber-500',
    iconColor: 'text-amber-500',
    label: 'patch'
  },

  // Thinking
  Thinking: {
    icon: Brain,
    className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
    iconColor: 'text-purple-500',
    label: 'thinking'
  },
};

export const defaultConfig: ToolConfig = {
  icon: Terminal,
  className: 'bg-purple-500/10 border-purple-500/20 text-purple-500',
  iconColor: 'text-purple-500',
  label: 'tool'
};

export function getToolConfig(toolType: string): ToolConfig {
  return toolConfigs[toolType] ?? toolConfigs[toolType.toLowerCase()] ?? defaultConfig;
}
